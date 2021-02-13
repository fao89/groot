use async_std::fs::File;
use async_std::path::Path;
use async_std::prelude::*;
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

#[tokio::main]
async fn main() -> Result<()> {
    let content_type = "collections"; // TODO: get it from CLI
    let root = Url::parse("https://galaxy.ansible.com/api/")?; // TODO: get it from CLI
    let target = match content_type {
        "roles" => root.join("v1/roles")?,
        "collections" => root.join("v2/collections")?,
        _ => panic!("Invalid content type!"),
    };
    let response = reqwest::get(target.as_str()).await?;
    let results = response.json::<Value>().await?;
    for data in results.as_object().unwrap()["results"].as_array().unwrap() {
        let content_path = format!(
            "{}/{}/{}/",
            content_type,
            data["namespace"]["name"].as_str().unwrap(),
            data["name"].as_str().unwrap(),
        );
        async_std::fs::create_dir_all(&content_path).await?;
        download_json(
            format!("{}metadata.json", content_path).as_str(),
            data.to_string(),
        )
        .await?;
        let response = reqwest::get(data["latest_version"]["href"].as_str().unwrap()).await?;
        let json_response = response.json::<Value>().await?;
        let version_path = format!(
            "{}/{}/{}/{}/",
            content_type,
            json_response["namespace"]["name"].as_str().unwrap(),
            json_response["collection"]["name"].as_str().unwrap(),
            json_response["version"].as_str().unwrap(),
        );
        async_std::fs::create_dir_all(&version_path).await?;
        download_json(
            format!("{}metadata.json", version_path).as_str(),
            data.to_string(),
        )
        .await?;
        let download_url = Url::parse(json_response["download_url"].as_str().unwrap())?;
        let response = reqwest::get(download_url.as_str()).await?;
        let filename = download_url.path_segments().unwrap().last().unwrap();
        download_tar(format!("{}{}", version_path, filename).as_str(), response).await?
    }

    Ok(())
}

async fn download_tar(filename: &str, response: reqwest::Response) -> Result<()> {
    println!("Downloading {} ...", filename);

    let path = Path::new(filename);

    let mut file = match File::create(&path).await {
        Err(why) => panic!("couldn't create {}", why),
        Ok(file) => file,
    };
    let content = response.bytes().await?;
    file.write_all(&content).await?;
    Ok(())
}

async fn download_json(filename: &str, content: String) -> Result<()> {
    let path = Path::new(filename);

    let mut file = match File::create(&path).await {
        Err(why) => panic!("couldn't create {}", why),
        Ok(file) => file,
    };
    file.write_all(&content.as_bytes()).await?;
    Ok(())
}
