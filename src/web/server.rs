use super::routes::*;
use actix_web::{
    middleware::{Logger, NormalizePath, TrailingSlash},
    web::Data,
    App, HttpServer,
};
use diesel::{
    r2d2::{ConnectionManager, Pool},
    PgConnection,
};
use dotenv::dotenv;
use log::info;
use paperclip::actix::OpenApiExt;
use r2d2_redis::RedisConnectionManager;
use std::time::Duration;

const CACHE_POOL_MAX_OPEN: u32 = 16;
const CACHE_POOL_MIN_IDLE: u32 = 8;
const CACHE_POOL_EXPIRE_SECONDS: u64 = 60;

pub async fn start_actix_server() {
    std::env::set_var("RUST_LOG", "actix_web=info,groot=info");
    pretty_env_logger::init();
    let db_url = dotenv::var("DATABASE_URL").expect("DATABASE_URL");
    let manager = ConnectionManager::<PgConnection>::new(db_url);
    let db_pool = Pool::builder()
        .build(manager)
        .expect("Error building a db connection pool");
    let redis_url = dotenv::var("REDIS_URL").expect("REDIS_URL");
    let manager =
        RedisConnectionManager::new(redis_url).expect("Error with redis connection manager");
    let redis_pool = Pool::builder()
        .max_size(CACHE_POOL_MAX_OPEN)
        .max_lifetime(Some(Duration::from_secs(CACHE_POOL_EXPIRE_SECONDS)))
        .min_idle(Some(CACHE_POOL_MIN_IDLE))
        .build(manager)
        .expect("Error building a redis connection pool");

    std::fs::create_dir_all("content/collections").unwrap();
    std::fs::create_dir_all("content/roles").unwrap();

    dotenv().ok();
    let config = crate::config::Config::from_env().unwrap();
    info!(
        "Starting server at http://{}:{}",
        config.server.host, config.server.port
    );
    HttpServer::new(move || {
        App::new()
            .app_data(Data::new(db_pool.clone()))
            .app_data(Data::new(redis_pool.clone()))
            .wrap_api()
            .wrap(NormalizePath::new(TrailingSlash::Always))
            .wrap(Logger::default())
            .service(list_v1)
            .service(role_retrieve)
            .service(role_version_list)
            .service(list_v2)
            .service(task_list)
            .service(task_retrieve)
            .service(collection_list)
            .service(collection_retrieve)
            .service(collection_version_retrieve)
            .service(collection_version_list)
            .service(api_metadata)
            .service(api_status)
            .service(start_sync)
            .service(collection_import)
            .with_json_spec_at("/api/spec/v2/")
            .with_swagger_ui_at("/openapi")
            .build()
            .service(start_req_sync)
            .service(collection_post)
            .service(actix_files::Files::new("/content", "content").show_files_listing())
    })
    .bind(format!("{}:{}", config.server.host, config.server.port))
    .unwrap()
    .run()
    .await
    .unwrap()
}
