use crate::models;
use actix_web::{get, web, Responder};
use diesel::prelude::*;
use diesel::{
    r2d2::{ConnectionManager, Pool},
    PgConnection,
};
use semver::Version;
use serde_json::json;
use std::collections::HashMap;

type DbPool = Pool<ConnectionManager<PgConnection>>;

#[get("/api/")]
async fn api_metadata() -> impl Responder {
    let resp = json!({"current_version": "v1", "available_versions": {"v1": "v1/", "v2": "v2/"}});
    web::HttpResponse::Ok().json(resp)
}

#[get("/sync/{content_type}/")]
async fn start_sync(web::Path(content_type): web::Path<String>) -> impl Responder {
    let resp = json!({ "syncing": content_type });
    std::thread::spawn(|| {
        std::process::Command::new("groot")
            .arg("sync")
            .arg("--content")
            .arg(content_type)
            .spawn()
            .expect("start sync")
    });
    web::HttpResponse::Ok().json(resp)
}

#[get("/api/v1/roles/")]
async fn role_retrieve(query: web::Query<HashMap<String, String>>) -> impl Responder {
    let empty_string = String::from("");
    let namespace = query.get("owner__username").unwrap_or(&empty_string);
    let name = query.get("name").unwrap_or(&empty_string);
    if namespace.is_empty() || name.is_empty() {
        let msg = json!({"Please specify the following query params": ["owner__username", "name"]});
        return web::HttpResponse::BadRequest().json(msg);
    }
    let path = format!("roles/{}/{}/metadata.json", namespace, name);
    let data = std::fs::read_to_string(&path).unwrap_or(empty_string);
    if data.is_empty() {
        let msg = json!({ "File not found": path });
        return web::HttpResponse::NotFound().json(msg);
    }
    let resp: serde_json::Value = serde_json::from_str(&data).unwrap();
    let results = json!({ "results": [resp] });
    web::HttpResponse::Ok().json(results)
}

#[get("/api/v1/roles/{namespace}/{name}/versions/")]
async fn role_version_list(
    web::Path((namespace, name)): web::Path<(String, String)>,
) -> impl Responder {
    let config = crate::config::Config::from_env().unwrap();
    let path = format!("roles/{}/{}/versions", namespace, name);
    let mut refs = Vec::new();
    for entry in std::fs::read_dir(&path).unwrap() {
        let version_number = entry.unwrap().file_name().into_string().unwrap();
        if Version::parse(&version_number).is_ok() {
            refs.push(json!({"name": version_number, "source": format!("http://{}:{}/{}/{}/{}.tar.gz", config.server.host, config.server.port, path, version_number,version_number)}))
        }
    }
    let data = json!({ "results": refs });
    web::HttpResponse::Ok().json(data)
}

#[get("/api/v2/collections/{namespace}/{name}/")]
async fn collection_retrieve(
    pool: web::Data<DbPool>,
    web::Path((namespace, name)): web::Path<(String, String)>,
) -> impl Responder {
    use crate::schema::*;
    let conn = pool.get().expect("couldn't get db connection from pool");
    let config = crate::config::Config::from_env().unwrap();
    let results = collections::table
        .inner_join(collection_versions::table)
        .select((collections::id, (collection_versions::version)))
        .filter(
            collections::namespace
                .eq(&namespace)
                .and(collections::name.eq(&name)),
        )
        .load::<(i32, String)>(&conn)
        .unwrap();
    let (collection_id, latest_version) = results
        .iter()
        .max_by(|x, y| {
            Version::parse(&x.1)
                .unwrap()
                .cmp(&Version::parse(&y.1).unwrap())
        })
        .unwrap();

    let href = format!(
        "http://{}:{}/api/v2/collections/{}/{}/",
        config.server.host, config.server.port, namespace, name
    );
    let versions_url = format!("{}versions/", href);
    let latest_href = format!("{}{}/", versions_url, latest_version);
    let resp = json!({
        "href": href,
        "id": collection_id,
        "name": name,
        "namespace": {"name": namespace},
        "versions_url": versions_url,
        "latest_version":{"version": latest_version, "href": latest_href}
    });
    web::HttpResponse::Ok().json(resp)
}

#[get("/api/v2/collections/{namespace}/{name}/versions/")]
async fn collection_version_list(
    pool: web::Data<DbPool>,
    web::Path((namespace, name)): web::Path<(String, String)>,
) -> impl Responder {
    use crate::schema::*;
    let conn = pool.get().expect("couldn't get db connection from pool");
    let config = crate::config::Config::from_env().unwrap();
    let versions = collections::table
        .inner_join(collection_versions::table)
        .select(collection_versions::version)
        .filter(
            collections::namespace
                .eq(&namespace)
                .and(collections::name.eq(&name)),
        )
        .load::<String>(&conn)
        .unwrap();

    let versions_url = format!(
        "http://{}:{}/api/v2/collections/{}/{}/versions/",
        config.server.host, config.server.port, namespace, name
    );
    let refs: Vec<_> = versions
        .iter()
        .map(|ver| json!({"version": ver, "href": format!("{}{}/", versions_url, ver)}))
        .collect();
    let data = json!({ "results": refs });
    web::HttpResponse::Ok().json(data)
}

#[get("/api/v2/collections/{namespace}/{name}/versions/{version}/")]
async fn collection_version_retrieve(
    pool: web::Data<DbPool>,
    web::Path((namespace, name, version)): web::Path<(String, String, String)>,
) -> impl Responder {
    use crate::schema::*;
    let conn = pool.get().expect("couldn't get db connection from pool");
    let config = crate::config::Config::from_env().unwrap();
    let result = collections::table
        .inner_join(collection_versions::table)
        .select(collection_versions::all_columns)
        .filter(
            collections::namespace
                .eq(&namespace)
                .and(collections::name.eq(&name))
                .and(collection_versions::version.eq(&version)),
        )
        .load::<models::CollectionVersion>(&conn)
        .unwrap();
    let current_version = result.first().unwrap();
    let collection_href = format!(
        "http://{}:{}/api/v2/collections/{}/{}/",
        config.server.host, config.server.port, namespace, name
    );
    let version_url = format!("{}versions/{}/", collection_href, version);
    let download_url = format!(
        "http://{}:{}/collections/{}/{}/versions/{}/{}",
        config.server.host,
        config.server.port,
        namespace,
        name,
        version,
        current_version.artifact["filename"].as_str().unwrap()
    );
    let resp = json!({
        "artifact": current_version.artifact,
        "collection": {"name": name, "id": current_version.collection_id, "href": collection_href},
        "download_url": download_url,
        "href": version_url,
        "id": current_version.id,
        "metadata": current_version.metadata,
        "namespace": {"name": namespace},
        "version": version,
    });
    web::HttpResponse::Ok().json(resp)
}
