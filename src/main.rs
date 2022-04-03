mod config;
pub mod db_utils;
pub mod models;
pub mod schema;
mod sync;
mod web;
#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_migrations;
use db_utils::run_migrations;
use dotenv::dotenv;
use web::start_actix_server;

#[actix_web::main]
async fn main() {
    dotenv().ok();
    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
    run_migrations(&db_url);
    start_actix_server().await
}
