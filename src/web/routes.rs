use crate::models::{self, Collection};
use crate::sync::{import_task, mirror_content, process_requirements};
use actix_multipart::Multipart;
use actix_web::{error, HttpResponse, Responder};
use diesel::prelude::*;
use diesel::{
    r2d2::{ConnectionManager, Pool},
    PgConnection,
};
use futures::TryStreamExt;
use paperclip::actix::{api_v2_operation, get, post, web};
use r2d2_redis::redis::{Commands, FromRedisValue};
use r2d2_redis::RedisConnectionManager;
use semver::Version;
use serde_json::json;
use std::collections::HashMap;
use std::time::Duration;
use url::Url;
use uuid::Uuid;

type DbPool = Pool<ConnectionManager<PgConnection>>;

#[api_v2_operation]
#[get("/api/")]
async fn api_metadata() -> impl Responder {
    let resp = json!({"current_version": "v1", "available_versions": {"v1": "v1/", "v2": "v2/"}});
    HttpResponse::Ok().json(resp)
}

#[api_v2_operation]
#[get("/api/status/")]
async fn api_status(
    pool: web::Data<DbPool>,
    redis_pool: web::Data<Pool<RedisConnectionManager>>,
) -> impl Responder {
    pool.get().expect("couldn't get db connection from pool");
    redis_pool
        .get()
        .expect("couldn't get redis connection from pool");
    let state = pool.state();
    let redis_state = redis_pool.state();
    let resp = json!({
        "db": {
            "connections": state.connections,
            "idle_connections": state.idle_connections
        },
        "redis": {
            "connections": redis_state.connections,
            "idle_connections": redis_state.idle_connections
        },
        "status": "ok"
    });
    HttpResponse::Ok().json(resp)
}

#[api_v2_operation]
#[post("/sync/{content_type}/")]
async fn start_sync(path: web::Path<String>, pool: web::Data<DbPool>) -> impl Responder {
    let content_type = path.into_inner();
    let resp = json!({ "syncing": content_type });
    let galaxy_url = dotenv::var("GALAXY_URL").unwrap_or("https://galaxy.ansible.com/".to_string());
    let root = Url::parse(galaxy_url.as_str()).unwrap();
    actix_web::rt::spawn(async move { mirror_content(root, content_type.as_str(), pool).await });
    HttpResponse::Ok().json(resp)
}

#[actix_web::post("/sync/")]
async fn start_req_sync(mut payload: Multipart, pool: web::Data<DbPool>) -> impl Responder {
    let mut field = payload.try_next().await.unwrap().unwrap();
    while field.name() != "requirements" {
        field = payload.try_next().await.unwrap().unwrap();
    }
    let mut data = Vec::new();
    while let Ok(Some(chunk)) = field.try_next().await {
        data.extend_from_slice(&chunk);
    }
    let galaxy_url = dotenv::var("GALAXY_URL").unwrap_or("https://galaxy.ansible.com/".to_string());
    let root = Url::parse(galaxy_url.as_str()).unwrap();
    actix_web::rt::spawn(async move { process_requirements(root, data, pool).await });

    let resp = json!({ "syncing": "requirements file" });
    HttpResponse::Ok().json(resp)
}

#[api_v2_operation]
#[get("/api/v1/")]
async fn list_v1() -> impl Responder {
    let resp = json!({ "roles": "/api/v1/roles/" });
    HttpResponse::Ok().json(resp)
}

#[api_v2_operation]
#[get("/api/v1/roles/")]
async fn role_retrieve(query: web::Query<HashMap<String, String>>) -> impl Responder {
    let empty_string = String::from("");
    let namespace = query.get("owner__username").unwrap_or(&empty_string);
    let name = query.get("name").unwrap_or(&empty_string);
    if namespace.is_empty() || name.is_empty() {
        let msg = json!({"Please specify the following query params": ["owner__username", "name"]});
        return HttpResponse::BadRequest().json(msg);
    }
    let resp = json!({ "id": format!("{namespace}/{name}") });
    let results = json!({ "results": [resp] });
    HttpResponse::Ok().json(results)
}

#[api_v2_operation]
#[get("/api/v1/roles/{namespace}/{name}/versions/")]
async fn role_version_list(path: web::Path<(String, String)>) -> impl Responder {
    let (namespace, name) = path.into_inner();
    let config = crate::config::Config::from_env().unwrap();
    let path = format!("roles/{namespace}/{name}/versions");
    let mut refs = Vec::new();
    for entry in std::fs::read_dir(&format!("content/{path}")).unwrap() {
        let version_number = entry.unwrap().file_name().into_string().unwrap();
        let current = json!({
            "name": version_number,
            "source": format!("http://{}:{}/{}/{}/{}.tar.gz", config.server.host, config.server.port, path, version_number,version_number)
        });
        let to_check = if version_number.starts_with('v') {
            version_number[1..version_number.len()].to_string()
        } else {
            version_number
        };
        if Version::parse(&to_check).is_ok() {
            refs.push(current)
        }
    }
    let data = json!({ "results": refs });
    HttpResponse::Ok().json(data)
}

#[api_v2_operation]
#[get("/api/v2/")]
async fn list_v2() -> impl Responder {
    let resp = json!({ "collections": "/api/v2/collections/" });
    HttpResponse::Ok().json(resp)
}

#[api_v2_operation]
#[get("/api/v2/collections/")]
async fn collection_list(pool: web::Data<DbPool>) -> impl Responder {
    use crate::schema::*;
    let mut conn = pool.get().expect("couldn't get db connection from pool");
    let results = collections::table
        .select(Collection::as_select())
        .load(&mut conn)
        .unwrap();

    let resp = json!({
        "count": results.len(),
        "results": results,
    });
    HttpResponse::Ok().json(resp)
}

#[actix_web::post("/api/v2/collections/")]
async fn collection_post(
    mut payload: Multipart,
    db_pool: web::Data<DbPool>,
    redis_pool: web::Data<Pool<RedisConnectionManager>>,
) -> impl Responder {
    let mut field = payload.try_next().await.unwrap().unwrap();
    while field.name() != "file" {
        field = payload.try_next().await.unwrap().unwrap();
    }
    let filename = field.content_disposition().get_filename().unwrap();
    let parts = filename.split('-').collect::<Vec<&str>>();
    if parts.len() != 3 {
        return HttpResponse::BadRequest().json(
            json!({"error": "Collection name should follow the pattern: <namespace>-<name>-<version>.tar.gz"})
        );
    }
    let mut conn = redis_pool
        .get_timeout(Duration::from_secs(1))
        .expect("couldn't get redis connection from pool");
    let task_uuid = Uuid::new_v4().to_string();
    conn.set::<&str, &str, bool>(task_uuid.as_str(), "waiting")
        .expect("Error setting key");
    let resp = json!({ "task": task_uuid });
    let mut data = Vec::new();
    while let Ok(Some(chunk)) = field.try_next().await {
        data.extend_from_slice(&chunk);
    }
    actix_web::rt::spawn(async move {
        import_task(
            task_uuid.as_str(),
            field.content_disposition().get_filename().unwrap(),
            field.headers(),
            data,
            db_pool,
            redis_pool,
        )
        .await
    });

    HttpResponse::Ok().json(resp)
}

#[actix_web::get("/api/v2/collection-imports/{task_id}/")]
async fn collection_import(
    pool: web::Data<Pool<RedisConnectionManager>>,
    path: web::Path<String>,
) -> impl Responder {
    let mut conn = pool
        .get_timeout(Duration::from_secs(1))
        .expect("couldn't get redis connection from pool");
    let task_id = path.into_inner();
    let value = conn.get(task_id.as_str()).expect("Error getting key");
    let state: String = FromRedisValue::from_redis_value(&value).expect("Error getting value");
    let mut resp = json!({"state": state});
    if state == "completed" {
        resp = json!({"state": state, "finished_at": "now"});
    }
    HttpResponse::Ok().json(resp)
}

#[api_v2_operation]
#[get("/api/v2/collections/{namespace}/{name}/")]
async fn collection_retrieve(
    pool: web::Data<DbPool>,
    path: web::Path<(String, String)>,
) -> impl Responder {
    use crate::schema::*;
    let mut conn = pool.get().expect("couldn't get db connection from pool");
    let config = crate::config::Config::from_env().unwrap();
    let (namespace, name) = path.into_inner();
    let results = collections::table
        .inner_join(collection_versions::table)
        .select((collections::id, (collection_versions::version)))
        .filter(
            collections::namespace
                .eq(&namespace)
                .and(collections::name.eq(&name)),
        )
        .load::<(i32, String)>(&mut conn)
        .map_err(error::ErrorNotFound)
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
    let versions_url = format!("{href}versions/");
    let latest_href = format!("{versions_url}{latest_version}/");
    let resp = json!({
        "href": href,
        "id": collection_id,
        "name": name,
        "namespace": {"name": namespace},
        "versions_url": versions_url,
        "latest_version":{"version": latest_version, "href": latest_href}
    });
    HttpResponse::Ok().json(resp)
}

#[api_v2_operation]
#[get("/api/v2/collections/{namespace}/{name}/versions/")]
async fn collection_version_list(path: web::Path<(String, String)>) -> impl Responder {
    let config = crate::config::Config::from_env().unwrap();
    let (namespace, name) = path.into_inner();
    let path = format!("collections/{namespace}/{name}/versions");
    let mut refs = Vec::new();
    for entry in std::fs::read_dir(&format!("content/{path}")).unwrap() {
        let version_number = entry.unwrap().file_name().into_string().unwrap();
        refs.push(json!({
            "version": version_number,
            "href": format!("http://{}:{}/api/v2/{}/{}/", config.server.host, config.server.port, path, version_number)
        }))
    }
    let data = json!({ "results": refs });
    HttpResponse::Ok().json(data)
}

#[api_v2_operation]
#[get("/api/v2/collections/{namespace}/{name}/versions/{version}/")]
async fn collection_version_retrieve(
    pool: web::Data<DbPool>,
    path: web::Path<(String, String, String)>,
) -> impl Responder {
    use crate::schema::*;
    let mut conn = pool.get().expect("couldn't get db connection from pool");
    let config = crate::config::Config::from_env().unwrap();
    let (namespace, name, version) = path.into_inner();
    let result = collections::table
        .inner_join(collection_versions::table)
        .select(collection_versions::all_columns)
        .filter(
            collections::namespace
                .eq(&namespace)
                .and(collections::name.eq(&name))
                .and(collection_versions::version.eq(&version)),
        )
        .load::<models::CollectionVersion>(&mut conn)
        .unwrap();
    let current_version = result.first().unwrap();
    let collection_href = format!(
        "http://{}:{}/api/v2/collections/{}/{}/",
        config.server.host, config.server.port, namespace, name
    );
    let version_url = format!("{collection_href}versions/{version}/");
    let download_url = format!(
        "http://{}:{}/content/collections/{}/{}/versions/{}/{}",
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
    HttpResponse::Ok().json(resp)
}
