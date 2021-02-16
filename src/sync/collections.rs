use super::{download_json, download_tar, get_with_retry};
use anyhow::{Context, Result};
use futures::future::try_join_all;
use serde_json::Value;
use url::Url;

pub async fn sync_collections(response: &Value) -> Result<()> {
    let results = response.as_object().unwrap()["results"].as_array().unwrap();
    let collection_futures: Vec<_> = results.iter().map(|data| fetch_collection(&data)).collect();
    try_join_all(collection_futures)
        .await
        .context("Failed to join collection futures")?;
    Ok(())
}

async fn fetch_collection(data: &Value) -> Result<()> {
    let content_path = format!(
        "collections/{}/{}/",
        data["namespace"]["name"].as_str().unwrap(),
        data["name"].as_str().unwrap(),
    );
    async_std::fs::create_dir_all(&content_path)
        .await
        .with_context(|| format!("Failed to create dir {}", content_path))?;
    download_json(
        format!("{}metadata.json", content_path).as_str(),
        data.to_string(),
    )
    .await
    .context("Failed to download collection metadata.json")?;
    fetch_versions(&data["versions_url"])
        .await
        .with_context(|| {
            format!(
                "Failed to fetch collection versions from {}",
                data["versions_url"]
            )
        })?;
    Ok(())
}

async fn fetch_versions(url: &Value) -> Result<()> {
    let mut versions_url = format!("{}?page_size=20", url.as_str().unwrap());
    loop {
        let response = get_with_retry(versions_url.as_str()).await?;
        let json_response = response
            .json::<Value>()
            .await
            .with_context(|| format!("Failed to parse JSON from {}", versions_url))?;
        let results = json_response.as_object().unwrap()["results"]
            .as_array()
            .unwrap();
        let collection_version_futures: Vec<_> = results
            .iter()
            .map(|data| fetch_collection_version(&data))
            .collect();
        try_join_all(collection_version_futures)
            .await
            .context("Failed to join collection versions futures")?;
        if json_response.as_object().unwrap()["next"]
            .as_str()
            .is_none()
        {
            break;
        }
        versions_url = json_response.as_object().unwrap()["next"]
            .as_str()
            .unwrap()
            .to_string();
    }
    Ok(())
}

async fn fetch_collection_version(data: &Value) -> Result<()> {
    let response = get_with_retry(data["href"].as_str().unwrap()).await?;
    let json_response = response
        .json::<Value>()
        .await
        .with_context(|| format!("Failed to parse JSON from {}", data["href"]))?;
    let version_path = format!(
        "collections/{}/{}/{}/",
        json_response["namespace"]["name"].as_str().unwrap(),
        json_response["collection"]["name"].as_str().unwrap(),
        json_response["version"].as_str().unwrap(),
    );
    async_std::fs::create_dir_all(&version_path)
        .await
        .with_context(|| format!("Failed to create dir {}", version_path))?;
    download_json(
        format!("{}metadata.json", version_path).as_str(),
        json_response.to_string(),
    )
    .await
    .context("Failed to save collection version metadata.json")?;
    let download_url = Url::parse(json_response["download_url"].as_str().unwrap())
        .with_context(|| format!("Failed to parse URL {}", json_response["download_url"]))?;
    let response = get_with_retry(download_url.as_str()).await?;
    let filename = download_url.path_segments().unwrap().last().unwrap();
    download_tar(format!("{}{}", version_path, filename).as_str(), response)
        .await
        .with_context(|| format!("Failed to download {}", download_url))?;
    Ok(())
}
