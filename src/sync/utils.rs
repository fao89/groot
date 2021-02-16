use anyhow::{Context, Result};
use async_std::fs::File;
use async_std::path::Path;
use async_std::prelude::*;
use async_std::task;
use std::time::Duration;

pub async fn download_tar(filename: &str, response: reqwest::Response) -> Result<()> {
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

pub async fn download_json(filename: &str, content: String) -> Result<()> {
    let path = Path::new(filename);

    let mut file = match File::create(&path).await {
        Err(why) => panic!("couldn't create {}", why),
        Ok(file) => file,
    };
    file.write_all(&content.as_bytes()).await?;
    Ok(())
}

pub async fn get_with_retry(url: &str) -> Result<reqwest::Response> {
    let mut response = reqwest::get(url)
        .await
        .with_context(|| format!("Failed to get {}", url))?;
    if !response.status().is_success() {
        task::sleep(Duration::from_secs(60)).await;
        println!("\nStatus {} - Retrying...\n", response.status());
        response = reqwest::get(url)
            .await
            .with_context(|| format!("Failed to get {}", url))?;
    }
    Ok(response)
}
