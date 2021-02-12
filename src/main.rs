use async_std::fs::File;
use async_std::path::Path;
use async_std::prelude::*;
use error_chain::error_chain;
use url::Url;

error_chain! {
     foreign_links {
         Io(async_std::io::Error);
         HttpRequest(reqwest::Error);
         ParseUrl(url::ParseError);
     }
}

#[tokio::main]
async fn main() -> Result<()> {
    let target =
        Url::parse("https://galaxy.ansible.com/download/pulp-pulp_installer-3.10.0.tar.gz")?;
    let response = reqwest::get(target.as_str()).await?;

    let filename = target.path_segments().unwrap().last().unwrap();
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
