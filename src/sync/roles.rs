use super::{download_tar, get_json};
use anyhow::{Context, Result};
use futures::future::try_join_all;
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;
use url::Url;

pub async fn sync_roles(response: &Value) -> Result<()> {
    let results = response.as_object().unwrap()["results"].as_array().unwrap();
    let role_futures: Vec<_> = results.iter().map(fetch_role).collect();
    try_join_all(role_futures)
        .await
        .context("Failed to join roles futures")?;
    Ok(())
}

async fn fetch_role(data: &Value) -> Result<()> {
    let content_path = format!(
        "content/roles/{}/{}/",
        data["summary_fields"]["namespace"]["name"]
            .as_str()
            .unwrap(),
        data["name"].as_str().unwrap(),
    );
    tokio::fs::create_dir_all(&content_path)
        .await
        .with_context(|| format!("Failed to create dir {content_path}"))?;
    fetch_versions(data)
        .await
        .with_context(|| format!("Failed to fetch role versions from {}", data["commit_url"]))?;
    let dependencies: Vec<String> = data["summary_fields"]["dependencies"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|x| {
            std::fs::metadata(format!(
                "content/roles/{}",
                x.as_str().unwrap().replace('.', "/")
            ))
            .is_err()
        })
        .map(|d| {
            let dep_path = format!("content/roles/{}", d.as_str().unwrap().replace('.', "/"));
            std::fs::create_dir_all(dep_path).unwrap();
            format!(
                "https://galaxy.ansible.com/api/v1/roles/?namespace__name={}",
                d.as_str().unwrap().replace('.', "&name=")
            )
        })
        .collect();
    if !dependencies.is_empty() {
        fetch_dependencies(dependencies).await;
    }
    Ok(())
}
async fn fetch_versions(data: &Value) -> Result<()> {
    let versions = data["summary_fields"]["versions"].as_array().unwrap();
    let version_futures: Vec<_> = versions
        .iter()
        .map(|version| fetch_role_version(data, &version["name"]))
        .collect();
    try_join_all(version_futures)
        .await
        .context("Failed to join role versions futures")?;
    fetch_role_version(data, &data["github_branch"]).await?;
    Ok(())
}
async fn fetch_role_version(data: &Value, version: &Value) -> Result<()> {
    let version_path = format!(
        "content/roles/{}/{}/versions/{}/",
        data["summary_fields"]["namespace"]["name"]
            .as_str()
            .unwrap(),
        data["name"].as_str().unwrap(),
        version.as_str().unwrap(),
    );
    tokio::fs::create_dir_all(&version_path)
        .await
        .with_context(|| format!("Failed to create dir {version_path}"))?;
    let github_url = format!(
        "https://github.com/{}/{}/archive/{}.tar.gz",
        data["github_user"].as_str().unwrap(),
        data["github_repo"].as_str().unwrap(),
        version.as_str().unwrap()
    );
    let download_url = Url::parse(github_url.as_str())
        .with_context(|| format!("Failed to parse url {github_url}"))?;
    let response = reqwest::get(download_url.as_str())
        .await
        .with_context(|| format!("Failed to download {download_url}"))?;
    let filename = download_url.path_segments().unwrap().last().unwrap();
    download_tar(format!("{version_path}{filename}").as_str(), response)
        .await
        .unwrap();
    Ok(())
}

fn fetch_dependencies(dependencies: Vec<String>) -> Pin<Box<dyn Future<Output = ()>>> {
    Box::pin(async move {
        let deps: Vec<_> = dependencies.iter().map(|x| get_json(x)).collect();
        let deps_json = try_join_all(deps).await.unwrap();
        let to_fetch: Vec<_> = deps_json.iter().map(sync_roles).collect();
        try_join_all(to_fetch).await.unwrap();
    })
}
