use super::routes::*;
use actix_web::{
    middleware::{normalize::TrailingSlash, Logger, NormalizePath},
    rt, App, HttpServer,
};
use dotenv::dotenv;

pub fn start_actix_server() {
    std::env::set_var("RUST_LOG", "actix_web=info");
    pretty_env_logger::init();
    rt::System::new("my actix server").block_on(async move {
        dotenv().ok();
        let config = crate::config::Config::from_env().unwrap();
        println!(
            "Starting server at http://{}:{}",
            config.server.host, config.server.port
        );
        HttpServer::new(|| {
            App::new()
                .wrap(NormalizePath::new(TrailingSlash::Always))
                .wrap(Logger::default())
                .service(api_metadata)
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
    })
}
