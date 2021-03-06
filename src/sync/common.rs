use super::{fetch_collection, get_json, sync_collections, sync_roles};
use anyhow::{Context, Result};
use futures::future::try_join_all;
use std::io::prelude::*;
use url::Url;
use yaml_rust::Yaml;
use yaml_rust::YamlLoader;

pub async fn process_requirements(root: &Url, requirements: String) -> Result<()> {
    let mut req = std::fs::File::open(requirements).context("Failed to open requirements.yml")?;
    let mut contents = String::new();
    req.read_to_string(&mut contents)
        .context("Failed to read requirements.yml")?;
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
                    let collection_name = match col.as_str() {
                        Some(value) => value,
                        None => col
                            .as_hash()
                            .unwrap()
                            .get(&Yaml::from_str("name"))
                            .unwrap()
                            .as_str()
                            .unwrap(),
                    };
                    let path = format!("{}{}", url_path, collection_name.replace(".", url_sep));
                    if col.as_hash().is_some()
                        && col
                            .as_hash()
                            .unwrap()
                            .get(&Yaml::from_str("source"))
                            .is_some()
                    {
                        let source_url = Url::parse(
                            col.as_hash()
                                .unwrap()
                                .get(&Yaml::from_str("source"))
                                .unwrap()
                                .as_str()
                                .unwrap(),
                        );
                        source_url.unwrap().join(path.as_str()).unwrap()
                    } else {
                        root.join(path.as_str()).unwrap()
                    }
                })
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

pub async fn mirror_content(root: Url, content_type: &str) -> Result<()> {
    let mut target = if content_type == "roles" {
        root.join("api/v1/roles/?page_size=100")
            .context("Failed to join api/v1/roles")?
    } else if content_type == "collections" {
        root.join("api/v2/collections/?page_size=20")
            .context("Failed to join api/v2/collections")?
    } else {
        panic!("Invalid content type!")
    };
    loop {
        let results = get_json(target.as_str()).await.unwrap();
        if content_type == "roles" {
            sync_roles(&results).await?
        } else if content_type == "collections" {
            sync_collections(&results).await?
        } else {
            panic!("Invalid content type!")
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
