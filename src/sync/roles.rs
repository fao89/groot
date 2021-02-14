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

pub async fn sync_roles(response: &Value) -> Result<()> {
    for data in response.as_object().unwrap()["results"].as_array().unwrap() {
        let content_path = format!(
            "roles/{}/{}/",
            data["summary_fields"]["namespace"]["name"]
                .as_str()
                .unwrap(),
            data["name"].as_str().unwrap(),
        );
        async_std::fs::create_dir_all(&content_path).await?;
        download_json(
            format!("{}metadata.json", content_path).as_str(),
            data.to_string(),
        )
        .await
        .unwrap();
        for version in data["summary_fields"]["versions"].as_array().unwrap() {
            let version_path = format!(
                "roles/{}/{}/{}/",
                data["summary_fields"]["namespace"]["name"]
                    .as_str()
                    .unwrap(),
                data["name"].as_str().unwrap(),
                version["name"].as_str().unwrap(),
            );
            async_std::fs::create_dir_all(&version_path).await?;
            download_json(
                format!("{}metadata.json", version_path).as_str(),
                data.to_string(),
            )
            .await
            .unwrap();
            let github_url = format!(
                "https://github.com/{}/{}/archive/{}.tar.gz",
                data["github_user"].as_str().unwrap(),
                data["github_repo"].as_str().unwrap(),
                version["name"].as_str().unwrap()
            );
            let download_url = Url::parse(github_url.as_str())?;
            let response = reqwest::get(download_url.as_str()).await?;
            let filename = download_url.path_segments().unwrap().last().unwrap();
            download_tar(format!("{}{}", version_path, filename).as_str(), response)
                .await
                .unwrap()
        }
    }
    Ok(())
}
