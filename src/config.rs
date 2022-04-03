pub use config::ConfigError;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: i32,
}

#[derive(Deserialize)]
pub struct Config {
    pub server: ServerConfig,
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let environment = config::Environment::default().try_parsing(true);
        let cfg = config::Config::builder().add_source(environment).build()?;
        cfg.try_deserialize()
    }
}
