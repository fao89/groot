use anyhow::{Context, Result};
use serde_json::Value;
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

pub async fn get_with_retry(url: &str) -> Result<reqwest::Response> {
    let response = match reqwest::get(url).await {
        Ok(mut resp) => {
            let status_to_retry = ["429", "502", "503", "504", "520"];
            let mut retry_time = 10;
            while status_to_retry.contains(&resp.status().as_str()) {
                eprintln!(
                    "\nStatus {} - Retrying in {} seconds...\n",
                    resp.status().as_str(),
                    retry_time
                );
                if retry_time > 100 {
                    break;
                }
                time::sleep(Duration::from_secs(retry_time)).await;
                resp = reqwest::get(url)
                    .await
                    .with_context(|| format!("Failed to get {}", url))?;
                retry_time += 20;
            }
            resp
        }
        Err(e) => {
            eprintln!("\nERROR - {} - Retrying...\n", e);
            time::sleep(Duration::from_secs(120)).await;
            reqwest::get(url)
                .await
                .with_context(|| format!("Failed to get {}", url))?
        }
    };
    Ok(response)
}

pub async fn get_json(url: &str) -> Result<Value> {
    let response = get_with_retry(url).await?;
    let values = response
        .json::<Value>()
        .await
        .context(format!("Failed to parse JSON from {}", url))?;
    Ok(values)
}
