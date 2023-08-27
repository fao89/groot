use super::routes::*;
use crate::db_utils::{get_db_pool, get_redis_pool};
use actix_web::{
    middleware::{Logger, NormalizePath, TrailingSlash},
    web::Data,
    App, HttpServer,
};
use dotenv::dotenv;
use paperclip::actix::OpenApiExt;

pub async fn start_actix_server() {
    std::env::set_var("RUST_LOG", "actix_web=info");
    pretty_env_logger::init();
    let db_url = dotenv::var("DATABASE_URL").expect("DATABASE_URL");
    let pool: diesel::r2d2::Pool<diesel::r2d2::ConnectionManager<diesel::PgConnection>> =
        get_db_pool(&db_url);
    let redis_url = dotenv::var("REDIS_URL").expect("REDIS_URL");
    let redis_pool = get_redis_pool(&redis_url);

    std::fs::create_dir_all("collections").unwrap();
    std::fs::create_dir_all("roles").unwrap();

    dotenv().ok();
    let config = crate::config::Config::from_env().unwrap();
    println!(
        "Starting server at http://{}:{}",
        config.server.host, config.server.port
    );
    HttpServer::new(move || {
        App::new()
            .app_data(Data::new(pool.clone()))
            .app_data(Data::new(redis_pool.clone()))
            .wrap_api()
            .wrap(NormalizePath::new(TrailingSlash::Always))
            .wrap(Logger::default())
            .service(list_v1)
            .service(role_retrieve)
            .service(role_version_list)
            .service(list_v2)
            .service(collection_list)
            .service(collection_retrieve)
            .service(collection_version_retrieve)
            .service(collection_version_list)
            .service(api_metadata)
            .service(api_status)
            .service(start_sync)
            .with_json_spec_at("/api/spec/v2/")
            .build()
            .service(start_req_sync)
            .service(collection_post)
            .service(collection_import)
            .service(actix_files::Files::new("/collections", "collections"))
            .service(actix_files::Files::new("/roles", "roles"))
    })
    .bind(format!("{}:{}", config.server.host, config.server.port))
    .unwrap()
    .run()
    .await
    .unwrap()
}
