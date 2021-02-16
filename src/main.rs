mod cli;
mod sync;
use anyhow::{Context, Result};
use cli::Command;
use cli::Config;
use serde_json::Value;
use structopt::StructOpt;
use sync::{get_with_retry, sync_collections, sync_roles};
use url::Url;

#[tokio::main]
async fn main() -> Result<()> {
    let conf = Config::from_args();
    let Command::Sync(sync_params) = conf.command;
    let content_type = sync_params.content.as_str();
    let root = Url::parse(sync_params.url.as_str()).context("Failed to parse URL")?;
    let mut target = match content_type {
        "roles" => root
            .join("api/v1/roles/?page_size=100")
            .context("Failed to join api/v1/roles")?,
        "collections" => root
            .join("api/v2/collections/?page_size=20")
            .context("Failed to join api/v2/collections")?,
        _ => panic!("Invalid content type!"),
    };
    loop {
        let response = get_with_retry(target.as_str()).await.unwrap();
        let results = response
            .json::<Value>()
            .await
            .context(format!("Failed to parse JSON from {}", target))?;
        match content_type {
            "roles" => sync_roles(&results).await.unwrap(),
            "collections" => sync_collections(&results).await.unwrap(),
            _ => panic!("Invalid content type!"),
        };
        if results.as_object().unwrap()["next"].as_str().is_none() {
            println!("Sync is complete!");
            break;
        }
        target = root
            .join(results.as_object().unwrap()["next_link"].as_str().unwrap())
            .context("Failed to join next_link")?
    }
    Ok(())
}
