use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "groot")]
pub struct Config {
    #[structopt(subcommand)]
    pub command: Command,
}

#[derive(Debug, StructOpt)]
#[structopt()]
pub enum Command {
    Sync(SyncParams),
}

#[derive(Debug, StructOpt)]
#[structopt(about = "groot sync command parameters")]
pub struct SyncParams {
    #[structopt(name = "content", long = "content", short = "c", required = true)]
    pub content: String,
    #[structopt(
        name = "url",
        long = "url",
        short = "u",
        required = false,
        default_value = "https://galaxy.ansible.com/"
    )]
    pub url: String,
}
