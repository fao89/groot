mod cli;
use async_std::fs::File;
use async_std::path::Path;
use async_std::prelude::*;
use cli::Command;
use cli::Config;
use error_chain::error_chain;
use serde_json::Value;
use structopt::StructOpt;
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
    let conf = Config::from_args();
    let Command::Sync(sync_params) = conf.command;
    let content_type = sync_params.content.as_str();
    let root = Url::parse(sync_params.url.as_str())?;
    let mut target = match content_type {
        "roles" => root.join("api/v1/roles/?page_size=100")?,
        "collections" => root.join("api/v2/collections/?page_size=100")?,
        _ => panic!("Invalid content type!"),
    };
    loop {
        let response = reqwest::get(target.as_str()).await?;
        let results = response.json::<Value>().await?;
        match content_type {
            "roles" => sync_roles(&results).await?,
            "collections" => sync_collections(&results, &root).await?,
            _ => panic!("Invalid content type!"),
        };
        if results.as_object().unwrap()["next"].as_str().is_none() {
            println!("Sync is complete!");
            break;
        }
        target = root.join(results.as_object().unwrap()["next"].as_str().unwrap())?
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

async fn sync_roles(response: &Value) -> Result<()> {
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
        .await?;
        let version_path = format!(
            "roles/{}/{}/{}/",
            data["summary_fields"]["namespace"]["name"]
                .as_str()
                .unwrap(),
            data["name"].as_str().unwrap(),
            data["github_branch"].as_str().unwrap(),
        );
        async_std::fs::create_dir_all(&version_path).await?;
        download_json(
            format!("{}metadata.json", version_path).as_str(),
            data.to_string(),
        )
        .await?;
        let github_url = format!(
            "https://github.com/{}/{}/archive/{}.tar.gz",
            data["github_user"].as_str().unwrap(),
            data["github_repo"].as_str().unwrap(),
            data["github_branch"].as_str().unwrap()
        );
        let download_url = Url::parse(github_url.as_str())?;
        let response = reqwest::get(download_url.as_str()).await?;
        let filename = download_url.path_segments().unwrap().last().unwrap();
        download_tar(format!("{}{}", version_path, filename).as_str(), response).await?
    }
    Ok(())
}

async fn sync_collections(response: &Value, root: &Url) -> Result<()> {
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
        .await?;
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
                .await?;
                let download_url = Url::parse(json_response["download_url"].as_str().unwrap())?;
                let response = reqwest::get(download_url.as_str()).await?;
                let filename = download_url.path_segments().unwrap().last().unwrap();
                download_tar(format!("{}{}", version_path, filename).as_str(), response).await?
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
