use super::{download_json, download_tar};
use anyhow::{Context, Result};
use futures::future::try_join_all;
use serde_json::Value;
use url::Url;

pub async fn sync_roles(response: &Value) -> Result<()> {
    let results = response.as_object().unwrap()["results"].as_array().unwrap();
    let role_futures: Vec<_> = results.iter().map(|data| fetch_role(&data)).collect();
    try_join_all(role_futures)
        .await
        .context("Failed to join roles futures")?;
    Ok(())
}

async fn fetch_role(data: &Value) -> Result<()> {
    let content_path = format!(
        "roles/{}/{}/",
        data["summary_fields"]["namespace"]["name"]
            .as_str()
            .unwrap(),
        data["name"].as_str().unwrap(),
    );
    tokio::fs::create_dir_all(&content_path)
        .await
        .with_context(|| format!("Failed to create dir {}", content_path))?;
    download_json(
        format!("{}metadata.json", content_path).as_str(),
        data.to_string(),
    )
    .await
    .unwrap();
    fetch_versions(&data).await.with_context(|| {
        format!(
            "Failed to fetch collection versions from {}",
            data["versions_url"]
        )
    })?;
    Ok(())
}
async fn fetch_versions(data: &Value) -> Result<()> {
    let versions = data["summary_fields"]["versions"].as_array().unwrap();
    let version_futures: Vec<_> = versions
        .iter()
        .map(|version| fetch_role_version(data, &version))
        .collect();
    try_join_all(version_futures)
        .await
        .context("Failed to join role versions futures")?;
    Ok(())
}
async fn fetch_role_version(data: &Value, version: &Value) -> Result<()> {
    let version_path = format!(
        "roles/{}/{}/{}/",
        data["summary_fields"]["namespace"]["name"]
            .as_str()
            .unwrap(),
        data["name"].as_str().unwrap(),
        version["name"].as_str().unwrap(),
    );
    tokio::fs::create_dir_all(&version_path)
        .await
        .with_context(|| format!("Failed to create dir {}", version_path))?;
    download_json(
        format!("{}metadata.json", version_path).as_str(),
        version.to_string(),
    )
    .await
    .unwrap();
    let github_url = format!(
        "https://github.com/{}/{}/archive/{}.tar.gz",
        data["github_user"].as_str().unwrap(),
        data["github_repo"].as_str().unwrap(),
        version["name"].as_str().unwrap()
    );
    let download_url = Url::parse(github_url.as_str())
        .with_context(|| format!("Failed to parse url {}", github_url))?;
    let response = reqwest::get(download_url.as_str())
        .await
        .with_context(|| format!("Failed to download {}", download_url))?;
    let filename = download_url.path_segments().unwrap().last().unwrap();
    download_tar(format!("{}{}", version_path, filename).as_str(), response)
        .await
        .unwrap();
    Ok(())
}
