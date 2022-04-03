mod cli;
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
use cli::{Command, Config};
use db_utils::run_migrations;
use dotenv::dotenv;
use std::process;
use structopt::StructOpt;
use sync::{mirror_content, process_requirements};
use url::Url;
use web::start_actix_server;

#[actix_web::main]
async fn main() {
    dotenv().ok();
    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
    run_migrations(&db_url);
    let conf = Config::from_args();
    if conf.serve {
        start_actix_server().await;
    }
    if conf.command.is_none() {
        eprintln!("Please refer to: --help");
        process::exit(1);
    }

    let Command::Sync(sync_params) = conf.command.unwrap();
    let content_type = sync_params.content.as_str();
    let root = Url::parse(sync_params.url.as_str()).unwrap();
    if sync_params.requirement.is_empty() && sync_params.content.is_empty() {
        eprintln!("Please specify a content type or requirements.yml");
        process::exit(1);
    } else if !sync_params.requirement.is_empty() {
        process_requirements(&root, sync_params.requirement)
            .await
            .unwrap();
    } else {
        mirror_content(root, content_type).await.unwrap();
    }
}
