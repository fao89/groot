use anyhow::Result;
use serde_json::json;
use std::collections::HashMap;
use std::fs;
use warp::{
    hyper::{header, Body, Response, StatusCode},
    Filter,
};

pub async fn serve_content() -> Result<()> {
    pretty_env_logger::init();
    let log = warp::log("groot::api");
    let collection_prefix = warp::path!("api" / "v2" / "collections" / ..);
    let roles = warp::get()
        .and(warp::path!("api" / "v1" / "roles"))
        .and(warp::query::<HashMap<String, String>>())
        .map(|p: HashMap<String, String>| {
            let empty_string = String::from("");
            let namespace = p.get("owner__username").unwrap_or(&empty_string);
            let name = p.get("name").unwrap_or(&empty_string);
            if namespace.is_empty() || name.is_empty() {
                return Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        "{\"Please specify the following query params\": [\"owner__username\", \"name\"]}",
                    ));
            }
            let path = format!("roles/{}/{}/metadata.json", namespace, name);
            let data = fs::read_to_string(&path).unwrap_or(empty_string);
            if data.is_empty() {
                return Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        format!("{{\"File not found\": \"{}\"}}", path),
                    ));
            }
            let results = format!("{{\"results\": [{}]}}", data);
            Response::builder()
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(results))
        })
        .with(log);

    let role_versions = warp::path!("api" / "v1" / "roles" / String / String / "versions").map(|namespace, name| {
            let path = format!("roles/{}/{}/versions", namespace, name);
            let mut refs = Vec::new();
            for entry in fs::read_dir(&path).unwrap() {
                let version_number = entry.unwrap().file_name().into_string().unwrap();
                refs.push(json!({"name": version_number, "source": format!("http://127.0.0.1:3030/{}/{}/{}.tar.gz", path, version_number, version_number)}))
            }
            let data = json!({ "results": refs });
            warp::reply::json(&data)
        }).with(log);

    let download_role = warp::path("roles").and(warp::fs::dir("roles")).with(log);

    let api = warp::path("api")
        .map(|| {
            let data =
                json!({"current_version": "v1", "available_versions": {"v1": "v1/", "v2": "v2/"}});
            warp::reply::json(&data)
        })
        .with(log);

    let collection = warp::get()
        .and(warp::path!(String / String))
        .map(|namespace, name| {
            let empty_string = String::from("");
            let path = format!("collections/{}/{}/metadata.json", namespace, name);
            let data = fs::read_to_string(&path).unwrap_or(empty_string);
            if data.is_empty() {
                return Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(format!("{{\"File not found\": \"{}\"}}", path)));
            }
            Response::builder()
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(data))
        })
        .with(log);

    let collection_versions = warp::path!(String / String / "versions").map(|namespace, name| {
            let path = format!("collections/{}/{}/versions", namespace, name);
            let mut refs = Vec::new();
            for entry in fs::read_dir(&path).unwrap() {
                let version_number = entry.unwrap().file_name().into_string().unwrap();
                refs.push(json!({"version": version_number, "href": format!("http://127.0.0.1:3030/api/v2/{}/{}/", path, version_number)}))
            }
            let data = json!({ "results": refs });
            warp::reply::json(&data)
        }).with(log);

    let collection_version = warp::get()
        .and(warp::path!(String / String / "versions" / String))
        .map(|namespace, name, version| {
            let empty_string = String::from("");
            let path = format!(
                "collections/{}/{}/versions/{}/metadata.json",
                namespace, name, version
            );
            let data = fs::read_to_string(&path).unwrap_or(empty_string);
            if data.is_empty() {
                return Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(format!("{{\"File not found\": \"{}\"}}", path)));
            }
            Response::builder()
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(data))
        })
        .with(log);

    let download_collection = warp::path("collections")
        .and(warp::fs::dir("collections"))
        .with(log);

    let routes = collection_prefix
        .and(collection.or(collection_versions).or(collection_version))
        .or(roles.or(role_versions))
        .or(download_collection)
        .or(download_role)
        .or(api);
    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
    Ok(())
}
