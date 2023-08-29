use super::{a2b_base64, fetch_collection, get_json, sync_collections, sync_roles};
use crate::db_utils::get_db_connection;
use crate::models;
use actix_multipart::Field;
use anyhow::{Context, Result};
use diesel::prelude::*;
use diesel::r2d2::PooledConnection;
use futures::future::try_join_all;
use futures::TryStreamExt;
use r2d2_redis::redis::Commands;
use r2d2_redis::RedisConnectionManager;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use url::Url;
use yaml_rust::Yaml;
use yaml_rust::YamlLoader;

pub async fn process_requirements(root: &Url, chunk: &actix_web::web::Bytes) -> Result<()> {
    let contents = std::str::from_utf8(chunk).unwrap();
    let docs = YamlLoader::load_from_str(contents).unwrap();
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
            let content_paths: Vec<_> = doc[content]
                .as_vec()
                .unwrap()
                .iter()
                .map(|item| {
                    let content_name = match item.as_str() {
                        Some(value) => value,
                        None => item
                            .as_hash()
                            .unwrap()
                            .get(&Yaml::from_str("name"))
                            .unwrap()
                            .as_str()
                            .unwrap(),
                    };
                    let path = format!("{}{}", url_path, content_name.replace('.', url_sep));
                    if item.as_hash().is_some()
                        && item
                            .as_hash()
                            .unwrap()
                            .get(&Yaml::from_str("source"))
                            .is_some()
                    {
                        let source_url = Url::parse(
                            item.as_hash()
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
            let content_futures: Vec<_> = content_paths
                .iter()
                .map(|data| get_json(data.as_str()))
                .collect();
            let responses: Vec<_> = try_join_all(content_futures).await?;
            if content == "roles" {
                let to_fetch: Vec<_> = responses.iter().map(sync_roles).collect();
                try_join_all(to_fetch).await?;
            } else {
                use crate::schema::collections::dsl::*;
                let mut conn = get_db_connection();

                let to_save: Vec<_> = responses
                    .iter()
                    .map(|data| models::CollectionNew {
                        namespace: data["namespace"]["name"].as_str().unwrap(),
                        name: data["name"].as_str().unwrap(),
                    })
                    .collect();
                println!("====== Saving collections ======");
                diesel::insert_into(collections)
                    .values(&to_save)
                    .on_conflict((namespace, name))
                    .do_nothing()
                    .execute(&mut conn)
                    .unwrap();
                // Downloading
                let to_fetch: Vec<_> = responses.iter().map(fetch_collection).collect();
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

pub async fn import_task(
    task_uuid: &str,
    mut conn: PooledConnection<RedisConnectionManager>,
    mut field: Field,
) -> Result<()> {
    let filename = field.content_disposition().get_filename().unwrap();
    let parts = filename.split('-').collect::<Vec<&str>>();
    let (namespace, name, version) = (parts[0], parts[1], parts[2].replace(".tar.gz", ""));
    let file_path = format!(
        "content/collections/{}/{}/versions/{}/",
        namespace, name, version
    );
    tokio::fs::create_dir_all(&file_path)
        .await
        .with_context(|| format!("Failed to create dir {file_path}"))?;

    let mut file = File::create(format!("{}/{}", file_path, filename))
        .await
        .unwrap();

    let mut data = Vec::new();

    while let Ok(Some(chunk)) = field.try_next().await {
        data.extend_from_slice(&chunk);
    }
    let encoded = match field.headers().get("content-transfer-encoding") {
        None => "",
        Some(encoded) => encoded.to_str().unwrap(),
    };
    if encoded == "base64" {
        let decoded = a2b_base64(data, false).unwrap();
        file.write_all(&decoded).await.unwrap();
    } else {
        file.write_all(&data).await.unwrap();
    }

    conn.set::<&str, &str, bool>(task_uuid, "completed")
        .expect("Error setting key");

    Ok(())
}
