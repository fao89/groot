mod cli;
mod sync;
use cli::Command;
use cli::Config;
use error_chain::error_chain;
use serde_json::Value;
use structopt::StructOpt;
use sync::{sync_collections, sync_roles};
use url::Url;

error_chain! {
     foreign_links {
         HttpRequest(reqwest::Error);
         ParseUrl(url::ParseError);
     }
}

#[tokio::main]
async fn main() -> Result<()> {
    let conf = Config::from_args();
    let Command::Sync(sync_params) = conf.command;
    let content_type = sync_params.content.as_str();
    let root = Url::parse(sync_params.url.as_str())?;
    let mut target = match content_type {
        "roles" => root.join("api/v1/roles/?page_size=100")?,
        "collections" => root.join("api/v2/collections/?page_size=10")?,
        _ => panic!("Invalid content type!"),
    };
    loop {
        let response = reqwest::get(target.as_str()).await?;
        let results = response.json::<Value>().await?;
        match content_type {
            "roles" => sync_roles(&results).await.unwrap(),
            "collections" => sync_collections(&results).await.unwrap(),
            _ => panic!("Invalid content type!"),
        };
        if results.as_object().unwrap()["next"].as_str().is_none() {
            println!("Sync is complete!");
            break;
        }
        target = root.join(results.as_object().unwrap()["next_link"].as_str().unwrap())?
    }
    Ok(())
}
