mod cli;
mod config;
mod sync;
mod web;
use cli::{Command, Config};
use std::process;
use structopt::StructOpt;
use sync::{mirror_content, process_requirements};
use url::Url;
use web::start_actix_server;

fn main() {
    let conf = Config::from_args();
    if conf.serve {
        start_actix_server();
    }
    if conf.command.is_none() {
        eprintln!("Please refer to: --help");
        process::exit(1);
    }
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
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
        });
}
