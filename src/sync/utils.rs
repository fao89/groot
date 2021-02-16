use anyhow::{Context, Result};
use std::time::Duration;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::time;

pub async fn download_tar(filename: &str, response: reqwest::Response) -> Result<()> {
    println!("Downloading {} ...", filename);

    let mut file = match File::create(filename).await {
        Err(why) => panic!("couldn't create {}", why),
        Ok(file) => file,
    };
    let content = response.bytes().await?;
    file.write_all(&content).await?;
    Ok(())
}

pub async fn download_json(filename: &str, content: String) -> Result<()> {
    let mut file = match File::create(filename).await {
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
        time::sleep(Duration::from_secs(60)).await;
        println!("\nStatus {} - Retrying...\n", response.status());
        response = reqwest::get(url)
            .await
            .with_context(|| format!("Failed to get {}", url))?;
    }
    Ok(response)
}
