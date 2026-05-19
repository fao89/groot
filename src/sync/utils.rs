use anyhow::{Context, Result};
use log::warn;
use reqwest::{Client, Request, Response};
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context as TaskContext, Poll};
use std::time::Duration;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::time;
use tower::util::BoxService;
use tower::{Service, ServiceBuilder, ServiceExt};

// Type alias for our rate-limited service
pub type RateLimitedHttpService =
    BoxService<Request, Response, Box<dyn std::error::Error + Send + Sync>>;

// Wrapper around reqwest::Client to implement the Service trait
#[derive(Clone)]
pub struct HttpService {
    client: Client,
}

impl HttpService {
    fn new(client: Client) -> Self {
        Self { client }
    }
}

impl Service<Request> for HttpService {
    type Response = Response;
    type Error = reqwest::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut TaskContext<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let client = self.client.clone();
        Box::pin(async move { client.execute(req).await })
    }
}

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

pub fn build_service(client: Client) -> RateLimitedHttpService {
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

    let http_service = HttpService::new(client);

    ServiceBuilder::new()
        .rate_limit(total_req, Duration::from_secs(1))
        .concurrency_limit(limit)
        .buffer(buffer)
        .service(http_service)
        .boxed()
}

pub async fn request(
    url: String,
    mut service: RateLimitedHttpService,
) -> (RateLimitedHttpService, Response) {
    let client = Client::new();
    let request = client.get(&url).build().unwrap();

    // Wait for the service to be ready
    let ready_service = service.ready().await.unwrap();

    // Make the request
    let response = ready_service.call(request).await.unwrap();

    (service, response)
}
