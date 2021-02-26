mod cli;
mod sync;
use anyhow::{Context, Result};
use cli::Command;
use cli::Config;
use futures::future::try_join_all;
use std::io::prelude::*;
use structopt::StructOpt;
use sync::{fetch_collection, get_json, sync_collections, sync_roles};
use url::Url;
use yaml_rust::YamlLoader;

#[tokio::main]
async fn main() -> Result<()> {
    let conf = Config::from_args();
    let Command::Sync(sync_params) = conf.command;
    let content_type = sync_params.content.as_str();
    let root = Url::parse(sync_params.url.as_str()).context("Failed to parse URL")?;
    if sync_params.requirement.is_empty() && sync_params.content.is_empty() {
        panic!("Please specify a content type or requirements.yml")
    } else if !sync_params.requirement.is_empty() {
        process_requirements(&root, sync_params.requirement).await?;
    } else {
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
            let results = get_json(target.as_str()).await.unwrap();
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
    }
    Ok(())
}

async fn process_requirements(root: &Url, requirements: String) -> Result<()> {
    let mut req = std::fs::File::open(requirements)?;
    let mut contents = String::new();
    req.read_to_string(&mut contents)?;
    let docs = YamlLoader::load_from_str(&contents).unwrap();
    let doc = &docs[0];
    for content in "collections roles".split(' ') {
        let url_path = match content {
            "roles" => "api/v1/roles/?namespace__name=",
            "collections" => "api/v2/collections/",
            _ => panic!("Invalid content type!"),
        };
        let url_sep = match content {
            "roles" => "&name=",
            "collections" => "/",
            _ => panic!("Invalid content type!"),
        };
        if doc[content].is_array() {
            let collection_paths: Vec<_> = doc[content]
                .as_vec()
                .unwrap()
                .iter()
                .map(|col| {
                    format!(
                        "{}{}",
                        url_path,
                        col.as_str().unwrap().replace(".", url_sep)
                    )
                })
                .map(|path| root.join(path.as_str()).unwrap())
                .collect();
            let collection_futures: Vec<_> = collection_paths
                .iter()
                .map(|data| get_json(&data.as_str()))
                .collect();
            let responses: Vec<_> = try_join_all(collection_futures).await?;
            if content == "roles" {
                let to_fetch: Vec<_> = responses.iter().map(|value| sync_roles(value)).collect();
                try_join_all(to_fetch).await?;
            } else {
                let to_fetch: Vec<_> = responses
                    .iter()
                    .map(|value| fetch_collection(value))
                    .collect();
                try_join_all(to_fetch).await?;
            };
        }
    }
    Ok(())
}
