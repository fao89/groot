use actix_web::{get, web, Responder};
use semver::Version;
use serde_json::json;
use std::collections::HashMap;

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
    web::Path((namespace, name)): web::Path<(String, String)>,
) -> impl Responder {
    let empty_string = String::from("");
    let path = format!("collections/{}/{}/metadata.json", namespace, name);
    let data = std::fs::read_to_string(&path).unwrap_or(empty_string);
    let resp: serde_json::Value = serde_json::from_str(&data).unwrap();
    web::HttpResponse::Ok().json(resp)
}

#[get("/api/v2/collections/{namespace}/{name}/versions/")]
async fn collection_version_list(
    web::Path((namespace, name)): web::Path<(String, String)>,
) -> impl Responder {
    let config = crate::config::Config::from_env().unwrap();
    let path = format!("collections/{}/{}/versions", namespace, name);
    let mut refs = Vec::new();
    for entry in std::fs::read_dir(&path).unwrap() {
        let version_number = entry.unwrap().file_name().into_string().unwrap();
        refs.push(json!({"version": version_number, "href": format!("http://{}:{}/api/v2/{}/{}/", config.server.host, config.server.port, path, version_number)}))
    }
    let data = json!({ "results": refs });
    web::HttpResponse::Ok().json(data)
}

#[get("/api/v2/collections/{namespace}/{name}/versions/{version}/")]
async fn collection_version_retrieve(
    web::Path((namespace, name, version)): web::Path<(String, String, String)>,
) -> impl Responder {
    let empty_string = String::from("");
    let path = format!(
        "collections/{}/{}/versions/{}/metadata.json",
        namespace, name, version
    );
    let data = std::fs::read_to_string(&path).unwrap_or(empty_string);
    let resp: serde_json::Value = serde_json::from_str(&data).unwrap();
    web::HttpResponse::Ok().json(resp)
}
