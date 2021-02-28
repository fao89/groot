use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "groot")]
pub struct Config {
    #[structopt(name = "serve", short = "s", long = "serve")]
    pub serve: bool,
    #[structopt(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, StructOpt)]
#[structopt()]
pub enum Command {
    Sync(SyncParams),
}

#[derive(Debug, StructOpt)]
#[structopt(about = "groot sync command parameters")]
pub struct SyncParams {
    #[structopt(
        name = "content",
        long = "content",
        short = "c",
        required = false,
        default_value = ""
    )]
    pub content: String,
    #[structopt(
        name = "url",
        long = "url",
        short = "u",
        required = false,
        default_value = "https://galaxy.ansible.com/"
    )]
    pub url: String,
    #[structopt(
        name = "requirement",
        long = "requirement",
        short = "r",
        required = false,
        default_value = ""
    )]
    pub requirement: String,
}
