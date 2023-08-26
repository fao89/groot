use super::routes::*;
use crate::db_utils::get_pool;
use actix_web::{
    middleware::{Logger, NormalizePath, TrailingSlash},
    web::Data,
    App, HttpServer,
};
use dotenv::dotenv;

pub async fn start_actix_server() {
    std::env::set_var("RUST_LOG", "actix_web=info");
    pretty_env_logger::init();
    let db_url = dotenv::var("DATABASE_URL").expect("DATABASE_URL");
    let pool: diesel::r2d2::Pool<diesel::r2d2::ConnectionManager<diesel::PgConnection>> =
        get_pool(&db_url);

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
            .wrap(NormalizePath::new(TrailingSlash::Always))
            .wrap(Logger::default())
            .service(api_metadata)
            .service(start_sync)
            .service(start_req_sync)
            .service(role_retrieve)
            .service(role_version_list)
            .service(collection_retrieve)
            .service(collection_version_retrieve)
            .service(collection_version_list)
            .service(actix_files::Files::new("/collections", "collections"))
            .service(actix_files::Files::new("/roles", "roles"))
    })
    .bind(format!("{}:{}", config.server.host, config.server.port))
    .unwrap()
    .run()
    .await
    .unwrap()
}
