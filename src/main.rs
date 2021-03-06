mod api;
mod cli;
mod sync;
use crate::api::serve_content;
use anyhow::{Context, Result};
use cli::{Command, Config};
use std::process;
use structopt::StructOpt;
use sync::{mirror_content, process_requirements};
use url::Url;

#[tokio::main]
async fn main() -> Result<()> {
    let conf = Config::from_args();
    if conf.serve {
        println!("Serving content at: http://127.0.0.1:3030/");
        serve_content().await?;
    }
    if conf.command.is_none() {
        eprintln!("Please refer to: --help");
        process::exit(1);
    }
    let Command::Sync(sync_params) = conf.command.unwrap();
    let content_type = sync_params.content.as_str();
    let root = Url::parse(sync_params.url.as_str()).context("Failed to parse URL")?;
    if sync_params.requirement.is_empty() && sync_params.content.is_empty() {
        eprintln!("Please specify a content type or requirements.yml");
        process::exit(1);
    } else if !sync_params.requirement.is_empty() {
        process_requirements(&root, sync_params.requirement).await?;
    } else {
        mirror_content(root, content_type).await?;
    }
    Ok(())
}
