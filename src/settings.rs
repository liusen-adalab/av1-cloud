use std::sync::OnceLock;

use anyhow::{Context, Result};
use config::Config;
use serde::{Deserialize, Serialize};

use crate::infrastructure::email::EmailCode;

#[derive(Deserialize, Debug)]
pub struct Settings {
    pub log: crate::logger::Config,
    pub http_server: HttpServer,

    #[cfg(feature = "keydb")]
    #[serde(rename = "keydb")]
    pub redis: utils::db_pools::keydb::Config,
    #[cfg(feature = "redis")]
    pub redis: utils::db_pools::redis::Config,

    pub postgres: utils::db_pools::postgres::PgPoolConfig,

    pub email_code: EmailCode,
}

#[derive(Deserialize, Debug)]
pub struct HttpServer {
    pub bind: String,
    pub port: u16,
    pub session: SessionRedis,
}

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct SessionRedis {
    pub url: String,
    pub secure: bool,
    pub http_only: bool,
    #[serde(default = "default_max_age")]
    pub max_age_secs: u32,
}

fn default_max_age() -> u32 {
    3600 * 24
}

static SETTINGS: OnceLock<Settings> = OnceLock::new();

pub fn load_settings(cfg_path: Option<&str>) -> Result<&'static Settings> {
    println!("loading settings. path = {:?}", cfg_path);
    let path = cfg_path.unwrap_or_default();
    let settings = Config::builder()
        .add_source(config::File::with_name("configs/default.toml").required(false))
        .add_source(config::File::with_name(path).required(cfg_path.is_some()))
        .add_source(config::Environment::with_prefix("AV1"))
        .build()
        .context("cannot load config")?;
    let settings: Settings = settings.try_deserialize().context("wrong config format")?;
    Ok(SETTINGS.get_or_init(|| settings))
}

pub fn get_settings() -> &'static Settings {
    unsafe { SETTINGS.get().unwrap_unchecked() }
}
