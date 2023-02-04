use super::{download_tar, get_json, get_with_retry};
use crate::db_utils::get_pool;
use crate::models;
use anyhow::{Context, Result};
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, PooledConnection};
use futures::future::try_join_all;
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;
use url::Url;

type CurrentPool =
    diesel::r2d2::PooledConnection<diesel::r2d2::ConnectionManager<diesel::PgConnection>>;

fn get_connection() -> PooledConnection<ConnectionManager<PgConnection>> {
    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
    let pool = get_pool(&db_url);
    pool.get().expect("couldn't get db connection from pool")
}

pub async fn sync_collections(response: &Value) -> Result<()> {
    let results = response.as_object().unwrap()["results"].as_array().unwrap();

    use crate::schema::collections::dsl::*;
    let mut conn = get_connection();

    let to_save: Vec<_> = results
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
    let collection_futures: Vec<_> = results.iter().map(fetch_collection).collect();
    try_join_all(collection_futures)
        .await
        .context("Failed to join collection futures")?;
    Ok(())
}

pub async fn fetch_collection(data: &Value) -> Result<()> {
    let content_path = format!(
        "collections/{}/{}/",
        data["namespace"]["name"].as_str().unwrap(),
        data["name"].as_str().unwrap(),
    );
    let mut conn = get_connection();
    tokio::fs::create_dir_all(&content_path)
        .await
        .with_context(|| format!("Failed to create dir {content_path}"))?;

    use crate::schema::collections::dsl::*;
    let collection = collections
        .filter(
            namespace
                .eq(data["namespace"]["name"].as_str().unwrap())
                .and(name.eq(data["name"].as_str().unwrap())),
        )
        .first::<models::Collection>(&mut conn)
        .optional()?
        .unwrap();
    fetch_versions(&data["versions_url"], collection, &mut conn)
        .await
        .with_context(|| {
            format!(
                "Failed to fetch collection versions from {}",
                data["versions_url"]
            )
        })?;
    Ok(())
}

async fn fetch_versions(
    url: &Value,
    collection: models::Collection,
    conn: &mut CurrentPool,
) -> Result<()> {
    let mut versions_url = format!("{}?page_size=20", url.as_str().unwrap());
    loop {
        let json_response = get_json(versions_url.as_str()).await?;
        let results = json_response.as_object().unwrap()["results"]
            .as_array()
            .unwrap();

        // Downloading
        let collection_version_futures: Vec<_> =
            results.iter().map(fetch_collection_version).collect();
        let downloaded = try_join_all(collection_version_futures)
            .await
            .context("Failed to join collection versions futures")?;
        // DB
        use crate::schema::collection_versions::dsl::*;
        let to_save: Vec<_> = downloaded
            .iter()
            .map(|data| models::CollectionVersionNew {
                collection_id: &collection.id,
                artifact: &data["artifact"],
                version: data["version"].as_str().unwrap(),
                metadata: &data["metadata"],
            })
            .collect();
        println!("====== Saving collection versions ======");
        diesel::insert_into(collection_versions)
            .values(&to_save)
            .on_conflict((collection_id, version))
            .do_nothing()
            .execute(conn)
            .unwrap();

        if json_response.as_object().unwrap()["next"]
            .as_str()
            .is_none()
        {
            break;
        }
        versions_url = json_response.as_object().unwrap()["next"]
            .as_str()
            .unwrap()
            .to_string();
    }
    Ok(())
}

async fn fetch_collection_version(data: &Value) -> Result<Value> {
    let json_response = get_json(data["href"].as_str().unwrap()).await?;
    let namespace = json_response["namespace"]["name"].as_str().unwrap();
    let version_path = format!(
        "collections/{}/{}/versions/{}/",
        namespace,
        json_response["collection"]["name"].as_str().unwrap(),
        json_response["version"].as_str().unwrap(),
    );
    tokio::fs::create_dir_all(&version_path)
        .await
        .with_context(|| format!("Failed to create dir {version_path}"))?;

    let download_url = Url::parse(json_response["download_url"].as_str().unwrap())
        .with_context(|| format!("Failed to parse URL {}", json_response["download_url"]))?;
    let response = get_with_retry(download_url.as_str()).await?;
    let filename = download_url.path_segments().unwrap().last().unwrap();
    download_tar(format!("{version_path}{filename}").as_str(), response)
        .await
        .with_context(|| format!("Failed to download {download_url}"))?;
    let root = json_response["collection"]["href"]
        .as_str()
        .unwrap()
        .split(namespace)
        .next()
        .unwrap();
    let dependencies: Vec<String> = json_response["metadata"]["dependencies"]
        .as_object()
        .unwrap()
        .keys()
        .filter(|x| std::fs::metadata(format!("collections/{}", x.replace('.', "/"))).is_err())
        .map(|d| {
            let dep_path = format!("collections/{}", d.replace('.', "/"));
            std::fs::create_dir_all(dep_path).unwrap();
            format!("{}{}/", root, d.replace('.', "/"))
        })
        .collect();
    if !dependencies.is_empty() {
        fetch_dependencies(dependencies).await;
    }
    Ok(json_response)
}
fn fetch_dependencies(dependencies: Vec<String>) -> Pin<Box<dyn Future<Output = ()>>> {
    Box::pin(async move {
        let deps: Vec<_> = dependencies.iter().map(|x| get_json(x)).collect();
        let deps_json = try_join_all(deps).await.unwrap();
        use crate::schema::collections::dsl::*;
        let mut conn = get_connection();

        let to_save: Vec<_> = deps_json
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
        let to_fetch: Vec<_> = deps_json.iter().map(fetch_collection).collect();
        try_join_all(to_fetch).await.unwrap();
    })
}
