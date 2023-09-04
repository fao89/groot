use super::{download_tar, get_json, get_with_retry};
use crate::models;
use crate::schema::collection_versions;
use actix_web::web;
use anyhow::{Context, Result};
use diesel::pg::upsert::excluded;
use diesel::prelude::*;
use diesel::{
    r2d2::{ConnectionManager, Pool},
    PgConnection,
};
use futures::future::try_join_all;
use serde_json::Value;
use std::collections::HashMap;
use url::Url;

#[derive(Debug, Clone)]
pub struct CollectionData {
    pub namespace: String,
    pub name: String,
    pub download_url: String,
    pub artifact: Value,
    pub version: String,
    pub metadata: Value,
}

pub async fn sync_collections(
    pool: web::Data<Pool<ConnectionManager<PgConnection>>>,
    response: &Value,
) -> Result<()> {
    let results = response.as_object().unwrap()["results"].as_array().unwrap();

    let collection_futures: Vec<_> = results.iter().map(fetch_collection).collect();
    let data = try_join_all(collection_futures)
        .await
        .context("Failed to join collection futures")?;
    process_collection_data(pool, data, false).await?;
    Ok(())
}

pub async fn fetch_collection(data: &Value) -> Result<Vec<CollectionData>> {
    fetch_versions(&data["versions_url"])
        .await
        .with_context(|| {
            format!(
                "Failed to fetch collection versions from {}",
                data["versions_url"]
            )
        })
}

async fn fetch_versions(url: &Value) -> Result<Vec<CollectionData>> {
    let mut versions: Vec<CollectionData> = Vec::new();
    let mut versions_url = format!("{}?page_size=100", url.as_str().unwrap());
    loop {
        let json_response = get_json(versions_url.as_str()).await?;
        let results = json_response.as_object().unwrap()["results"]
            .as_array()
            .unwrap();

        // Downloading
        let collection_version_futures: Vec<_> =
            results.iter().map(fetch_collection_version).collect();
        let cversions = try_join_all(collection_version_futures)
            .await
            .context("Failed to join collection versions futures")?;
        versions.extend_from_slice(&cversions);

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
    Ok(versions)
}

async fn fetch_collection_version(data: &Value) -> Result<CollectionData> {
    let json_response = get_json(data["href"].as_str().unwrap()).await?;
    let data = CollectionData {
        namespace: json_response["namespace"]["name"]
            .as_str()
            .unwrap()
            .to_string(),
        name: json_response["collection"]["name"]
            .as_str()
            .unwrap()
            .to_string(),
        download_url: json_response["download_url"].as_str().unwrap().to_string(),
        artifact: json_response["artifact"].clone(),
        version: json_response["version"].as_str().unwrap().to_string(),
        metadata: json_response["metadata"].clone(),
    };

    Ok(data)
}

pub async fn download_version(data: &CollectionData) -> Result<()> {
    let version_path = format!(
        "content/collections/{}/{}/versions/{}/",
        data.namespace.as_str(),
        data.name.as_str(),
        data.version.as_str(),
    );
    tokio::fs::create_dir_all(&version_path)
        .await
        .with_context(|| format!("Failed to create dir {version_path}"))?;
    let download_url = Url::parse(data.download_url.as_str())
        .with_context(|| format!("Failed to parse URL {}", data.download_url))?;
    let response = get_with_retry(download_url.as_str()).await?;
    let filename = download_url.path_segments().unwrap().last().unwrap();
    download_tar(format!("{version_path}{filename}").as_str(), response)
        .await
        .with_context(|| format!("Failed to download {download_url}"))?;
    Ok(())
}

pub async fn process_collection_data(
    pool: web::Data<Pool<ConnectionManager<PgConnection>>>,
    data: Vec<Vec<CollectionData>>,
    fetch_dependencies: bool,
) -> Result<()> {
    let mut to_process = data;
    loop {
        let mut versions: Vec<CollectionData> = Vec::new();
        let mut to_save: Vec<models::CollectionNew> = Vec::new();
        for colls in to_process.iter() {
            versions.extend_from_slice(colls);
            let col = colls.first().unwrap();
            to_save.push(models::CollectionNew {
                namespace: col.namespace.as_str(),
                name: col.name.as_str(),
            })
        }
        use crate::schema::collections::dsl::*;
        let mut conn = pool.get().expect("couldn't get db connection from pool");

        let cdata: Vec<(i32, String, String)> = diesel::insert_into(collections)
            .values(&to_save)
            .on_conflict((namespace, name))
            .do_update()
            .set((namespace.eq(excluded(namespace)), name.eq(excluded(name))))
            .returning((id, namespace, name))
            .get_results(&mut conn)
            .unwrap();
        let mut mmap: HashMap<String, i32> = HashMap::new();
        for v in cdata.iter() {
            mmap.insert(format!("{}.{}", v.1.as_str(), v.2.as_str()), v.0);
        }

        let data_futures: Vec<_> = versions.iter().map(download_version).collect();
        try_join_all(data_futures)
            .await
            .context("Failed to join collection data futures")?;
        let mut to_save: Vec<models::CollectionVersionNew> = Vec::new();
        for vs in versions.iter() {
            to_save.push(models::CollectionVersionNew {
                collection_id: &mmap
                    [format!("{}.{}", vs.namespace.as_str(), vs.name.as_str()).as_str()],
                artifact: &vs.artifact,
                version: vs.version.as_str(),
                metadata: &vs.metadata,
            })
        }
        diesel::insert_into(collection_versions::table)
            .values(&to_save)
            .on_conflict((
                collection_versions::columns::version,
                collection_versions::columns::collection_id,
            ))
            .do_update()
            .set((
                collection_versions::columns::collection_id
                    .eq(excluded(collection_versions::columns::collection_id)),
                collection_versions::columns::version
                    .eq(excluded(collection_versions::columns::version)),
            ))
            .execute(&mut conn)
            .unwrap();
        if fetch_dependencies {
            let galaxy_url =
                dotenv::var("GALAXY_URL").unwrap_or("https://galaxy.ansible.com/".to_string());
            let collections_endpoint = format!("{}api/v2/collections/", galaxy_url);
            let dependencies: Vec<Vec<String>> = versions
                .iter()
                .filter(|c| !c.metadata["dependencies"].as_object().unwrap().is_empty())
                .map(|c| {
                    c.metadata["dependencies"]
                        .as_object()
                        .unwrap()
                        .keys()
                        .filter(|x| {
                            std::fs::metadata(format!(
                                "content/collections/{}",
                                x.replace('.', "/")
                            ))
                            .is_err()
                        })
                        .map(|d| {
                            let dep_path = format!("content/collections/{}", d.replace('.', "/"));
                            std::fs::create_dir_all(dep_path).unwrap();
                            format!("{}{}/", collections_endpoint, d.replace('.', "/"))
                        })
                        .collect()
                })
                .collect();

            let mut deps: HashMap<String, bool> = HashMap::new();
            for urls in dependencies {
                for url in urls {
                    deps.insert(url, true);
                }
            }
            if deps.keys().len() > 0 {
                let dependencies: Vec<_> = deps.keys().map(|url| get_json(url)).collect();
                let deps_json = try_join_all(dependencies).await.unwrap();
                let to_fetch: Vec<_> = deps_json.iter().map(fetch_collection).collect();
                to_process = try_join_all(to_fetch).await.unwrap();
            } else {
                break;
            }
        } else {
            break;
        }
    }
    Ok(())
}
