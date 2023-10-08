use super::{
    a2b_base64, build_service, fetch_collection, get_json, process_collection_data,
    sync_collections, sync_roles,
};
use crate::models;
use actix_web::{http::header::HeaderMap, web};
use anyhow::{Context, Result};
use diesel::pg::upsert::excluded;
use diesel::prelude::*;
use diesel::{
    r2d2::{ConnectionManager, Pool},
    PgConnection,
};
use futures::future::try_join_all;
use log::info;
use r2d2_redis::redis::Commands;
use r2d2_redis::RedisConnectionManager;
use serde_json::json;
use std::time::Duration;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use url::Url;
use yaml_rust::Yaml;
use yaml_rust::YamlLoader;

pub async fn process_requirements(
    root: Url,
    chunk: Vec<u8>,
    pool: web::Data<Pool<ConnectionManager<PgConnection>>>,
) -> Result<()> {
    let contents = std::str::from_utf8(chunk.as_ref()).unwrap();
    let docs = YamlLoader::load_from_str(contents).unwrap();
    let doc = &docs[0];
    for content in "collections roles".split(' ') {
        let url_path = match content {
            "roles" => "api/v1/roles/?namespace=",
            "collections" => "api/v3/collections/",
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
                info!("Syncing roles");
                let to_fetch: Vec<_> = responses.iter().map(sync_roles).collect();
                try_join_all(to_fetch).await?;
            } else {
                info!("Syncing collections");
                let to_fetch: Vec<_> = responses.iter().map(fetch_collection).collect();
                let data = try_join_all(to_fetch).await?;
                process_collection_data(pool.clone(), data, true).await?
            };
        }
    }
    Ok(())
}

pub async fn mirror_content(
    root: Url,
    content_type: &str,
    pool: web::Data<Pool<ConnectionManager<PgConnection>>>,
) -> Result<()> {
    let mut target = if content_type == "roles" {
        root.join("api/v1/roles/?page_size=100")
            .context("Failed to join api/v1/roles")?
    } else if content_type == "collections" {
        root.join("api/v3/plugin/ansible/search/collection-versions/?is_deprecated=false&repository_label=!hide_from_search&offset=0&limit=100")
            .context("Failed to join api/v3/collections")?
    } else {
        panic!("Invalid content type!")
    };
    let client = reqwest::Client::new();
    let service = build_service(client.clone());
    loop {
        let results = get_json(target.as_str()).await.unwrap();
        if content_type == "roles" {
            info!("Syncing roles");
            sync_roles(&results).await?;
            if results.as_object().unwrap()["next"].as_str().is_none() {
                info!("Sync is complete!");
                break;
            }
            target = root
                .join(results.as_object().unwrap()["next_link"].as_str().unwrap())
                .context("Failed to join next_link")?
        } else if content_type == "collections" {
            info!("Syncing collections");
            sync_collections(pool.clone(), &results, client.clone(), service.clone()).await?;
            if results.as_object().unwrap()["links"]["next"]
                .as_str()
                .is_none()
            {
                info!("Sync is complete!");
                break;
            }
            target = root
                .join(
                    results.as_object().unwrap()["links"]["next"]
                        .as_str()
                        .unwrap(),
                )
                .context("Failed to join next_link")?
        } else {
            panic!("Invalid content type!")
        };
    }
    Ok(())
}

pub async fn import_task(
    task_uuid: &str,
    filename: &str,
    headers: &HeaderMap,
    data: Vec<u8>,
    dpool: web::Data<Pool<ConnectionManager<PgConnection>>>,
    rpool: web::Data<Pool<RedisConnectionManager>>,
) -> Result<()> {
    let mut rconn = rpool
        .get_timeout(Duration::from_secs(1))
        .expect("couldn't get redis connection from pool");
    rconn
        .set::<&str, &str, bool>(task_uuid, "running")
        .expect("Error setting key");

    let parts = filename.split('-').collect::<Vec<&str>>();
    let (namespace, name, version) = (parts[0], parts[1], parts[2].replace(".tar.gz", ""));

    use crate::schema::*;
    let mut dbconn = dpool.get().expect("couldn't get db connection from pool");
    let col = models::CollectionNew { namespace, name };
    let collection_id: Vec<i32> = diesel::insert_into(collections::table)
        .values(&col)
        .on_conflict((collections::columns::namespace, collections::columns::name))
        .do_update()
        .set((
            collections::columns::namespace.eq(excluded(collections::columns::namespace)),
            collections::columns::name.eq(excluded(collections::columns::name)),
        ))
        .returning(collections::columns::id)
        .get_results(&mut dbconn)
        .unwrap();

    let content_length: usize = match headers.get("content-length") {
        Some(header_value) => header_value.to_str().unwrap_or("0").parse().unwrap(),
        None => "0".parse().unwrap(),
    };
    let config = crate::config::Config::from_env().unwrap();
    let href = format!(
        "http://{}:{}/api/v2/collections/{}/{}/",
        config.server.host, config.server.port, namespace, name
    );
    let artifact = json!({"filename": filename, "size": content_length, "href": href});
    let metadata = json!({"name": name, "version": version, "namespace": namespace, "groot": true});
    let cversion = models::CollectionVersionNew {
        collection_id: collection_id.first().unwrap(),
        artifact: &artifact,
        version: &version,
        metadata: &metadata,
    };
    diesel::insert_into(collection_versions::table)
        .values(&cversion)
        .on_conflict((
            collection_versions::columns::collection_id,
            collection_versions::columns::version,
        ))
        .do_nothing()
        .execute(&mut dbconn)
        .unwrap();

    let file_path = format!(
        "content/collections/{}/{}/versions/{}/",
        namespace, name, version
    );
    tokio::fs::create_dir_all(&file_path)
        .await
        .with_context(|| format!("Failed to create dir {file_path}"))?;
    info!("Uploading {}", filename);
    let mut file = File::create(format!("{}/{}", file_path, filename))
        .await
        .unwrap();

    let encoded = match headers.get("content-transfer-encoding") {
        None => "",
        Some(encoded) => encoded.to_str().unwrap(),
    };
    if encoded == "base64" {
        let decoded = a2b_base64(data, false).unwrap();
        file.write_all(&decoded).await.unwrap();
    } else {
        file.write_all(&data).await.unwrap();
    }

    rconn
        .set::<&str, &str, bool>(task_uuid, "completed")
        .expect("Error setting key");

    Ok(())
}
