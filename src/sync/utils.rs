use anyhow::{Context, Result};
use log::warn;
use reqwest::{Client, Request, Response};
use serde_json::Value;
use std::time::Duration;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::time;
use tower::buffer::Buffer;
use tower::limit::{ConcurrencyLimit, RateLimit};
use tower::{Service, ServiceExt};

pub async fn download_tar(filename: &str, response: reqwest::Response) -> Result<()> {
    let mut file = match File::create(filename).await {
        Err(why) => panic!("couldn't create {}", why),
        Ok(file) => file,
    };
    let content = response.bytes().await?;
    file.write_all(&content).await?;
    Ok(())
}

async fn get_with_retry(url: &str) -> Result<reqwest::Response> {
    let response = match reqwest::get(url).await {
        Ok(mut resp) => {
            let status_to_retry = ["429", "502", "503", "504", "520"];
            let mut retry_time = 20;
            while status_to_retry.contains(&resp.status().as_str()) {
                warn!(
                    "\nStatus {} - Retrying in {} seconds...\n",
                    resp.status().as_str(),
                    retry_time
                );
                if retry_time > 300 {
                    break;
                }
                time::sleep(Duration::from_secs(retry_time)).await;
                resp = reqwest::get(url)
                    .await
                    .with_context(|| format!("Failed to get {url}"))?;
                retry_time += 20;
            }
            resp
        }
        Err(e) => {
            warn!("\nERROR - {e} - Retrying...\n");
            time::sleep(Duration::from_secs(120)).await;
            reqwest::get(url)
                .await
                .with_context(|| format!("Failed to get {url}"))?
        }
    };
    Ok(response)
}

pub async fn get_json(url: &str) -> Result<Value> {
    let response = get_with_retry(url).await?;
    let values = response
        .json::<Value>()
        .await
        .context(format!("Failed to parse JSON from {url}"))?;
    Ok(values)
}

pub fn build_service(client: Client) -> Buffer<ConcurrencyLimit<RateLimit<Client>>, Request> {
    let buffer = dotenv::var("GROOT_BUFFER")
        .unwrap_or("100".to_string())
        .as_str()
        .parse::<usize>()
        .unwrap();
    let limit = dotenv::var("GROOT_CONCURRENCY_LIMIT")
        .unwrap_or("10".to_string())
        .as_str()
        .parse::<usize>()
        .unwrap();
    let total_req = dotenv::var("GROOT_TOTAL_REQUESTS_PER_SECOND")
        .unwrap_or("5".to_string())
        .parse::<u64>()
        .unwrap();
    tower::ServiceBuilder::new()
        .buffer(buffer)
        .concurrency_limit(limit)
        .rate_limit(total_req, Duration::from_secs(1))
        .service(client.clone())
}

pub async fn request(
    url: String,
    mut service: Buffer<ConcurrencyLimit<RateLimit<Client>>, Request>,
) -> (
    Buffer<ConcurrencyLimit<RateLimit<Client>>, Request>,
    Response,
) {
    let client = reqwest::Client::new();
    let http_request = client.get(url).build().unwrap();
    let mut is_ready = service.ready().await.is_ok();
    while !is_ready {
        is_ready = service.ready().await.is_ok();
    }
    let response = service.call(http_request).await.unwrap();
    (service, response)
}
