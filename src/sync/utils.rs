use async_std::fs::File;
use async_std::path::Path;
use async_std::prelude::*;
use async_std::task;
use error_chain::error_chain;
use std::time::Duration;

error_chain! {
     foreign_links {
         Io(async_std::io::Error);
         HttpRequest(reqwest::Error);
     }
}

pub async fn download_tar(filename: &str, response: reqwest::Response) -> Result<()> {
    println!("Downloading {} ...", filename);

    let path = Path::new(filename);

    let mut file = match File::create(&path).await {
        Err(why) => panic!("couldn't create {}", why),
        Ok(file) => file,
    };
    let content = response.bytes().await?;
    file.write_all(&content).await?;
    Ok(())
}

pub async fn download_json(filename: &str, content: String) -> Result<()> {
    let path = Path::new(filename);

    let mut file = match File::create(&path).await {
        Err(why) => panic!("couldn't create {}", why),
        Ok(file) => file,
    };
    file.write_all(&content.as_bytes()).await?;
    Ok(())
}

pub async fn get_with_retry(url: &str) -> Result<reqwest::Response> {
    let mut response = reqwest::get(url).await?;
    if !response.status().is_success() {
        task::sleep(Duration::from_secs(60)).await;
        println!("\nStatus {} - Retrying...\n", response.status());
        response = reqwest::get(url).await?;
    }
    Ok(response)
}
