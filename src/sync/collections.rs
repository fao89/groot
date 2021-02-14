use super::{download_json, download_tar};
use error_chain::error_chain;
use serde_json::Value;
use url::Url;

error_chain! {
     foreign_links {
         Io(async_std::io::Error);
         HttpRequest(reqwest::Error);
         ParseUrl(url::ParseError);
     }
}

pub async fn sync_collections(response: &Value, root: &Url) -> Result<()> {
    for data in response.as_object().unwrap()["results"].as_array().unwrap() {
        let content_path = format!(
            "collections/{}/{}/",
            data["namespace"]["name"].as_str().unwrap(),
            data["name"].as_str().unwrap(),
        );
        async_std::fs::create_dir_all(&content_path).await?;
        download_json(
            format!("{}metadata.json", content_path).as_str(),
            data.to_string(),
        )
        .await
        .unwrap();
        let mut versions_url = format!("{}?page_size=100", data["versions_url"].as_str().unwrap());
        loop {
            let response = reqwest::get(versions_url.as_str()).await?;
            let results = response.json::<Value>().await?;
            for version in results.as_object().unwrap()["results"].as_array().unwrap() {
                let response = reqwest::get(version["href"].as_str().unwrap()).await?;
                let json_response = response.json::<Value>().await?;
                let version_path = format!(
                    "collections/{}/{}/{}/",
                    json_response["namespace"]["name"].as_str().unwrap(),
                    json_response["collection"]["name"].as_str().unwrap(),
                    json_response["version"].as_str().unwrap(),
                );
                async_std::fs::create_dir_all(&version_path).await?;
                download_json(
                    format!("{}metadata.json", version_path).as_str(),
                    data.to_string(),
                )
                .await
                .unwrap();
                let download_url = Url::parse(json_response["download_url"].as_str().unwrap())?;
                let response = reqwest::get(download_url.as_str()).await?;
                let filename = download_url.path_segments().unwrap().last().unwrap();
                download_tar(format!("{}{}", version_path, filename).as_str(), response)
                    .await
                    .unwrap()
            }
            if results.as_object().unwrap()["next"].as_str().is_none() {
                break;
            }
            versions_url = root
                .join(results.as_object().unwrap()["next"].as_str().unwrap())?
                .to_string();
        }
    }
    Ok(())
}
