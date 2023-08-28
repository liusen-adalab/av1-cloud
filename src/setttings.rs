use std::sync::OnceLock;

use anyhow::{Context, Result};
use config::Config;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct Settings {
    pub log: crate::logger::Config,
    pub http_server: HttpServer,
}

#[derive(Deserialize, Debug)]
pub struct HttpServer {
    pub bind: String,
    pub port: u16,
}

static SETTINGS: OnceLock<Settings> = OnceLock::new();

pub fn load_settings(path: Option<&str>) -> Result<&'static Settings> {
    println!("loading settings. path = {:?}", path);
    let path = path.unwrap_or_default();
    let settings = Config::builder()
        .add_source(config::File::with_name("configs/default.toml").required(false))
        .add_source(config::File::with_name(path).required(false))
        .add_source(config::Environment::with_prefix("AV1"))
        .build()
        .context("cannot load config")?;
    let settings: Settings = settings.try_deserialize().context("wrong config format")?;
    Ok(SETTINGS.get_or_init(|| settings))
}

pub fn get_settings() -> &'static Settings {
    unsafe { SETTINGS.get().unwrap_unchecked() }
}
