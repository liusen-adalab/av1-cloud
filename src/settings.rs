use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};

use anyhow::{Context, Result};
use clap::Parser;
use config::Config;
use serde::{Deserialize, Serialize};

use crate::{application::file_system::FileSystemCfg, infrastructure::email::EmailCodeCfg};

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

    pub email_code: EmailCodeCfg,

    pub init_system: InitSystem,

    pub file_system: FileSystemCfg,
}

#[derive(Deserialize, Debug, Serialize)]
pub struct InitSystem {
    pub register_test_user: bool,
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

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[command(arg_required_else_help(false))]
pub struct Args {
    /// Config file path
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Should register root user
    #[arg(short, long)]
    register_test_user: bool,
}

pub fn load_settings() -> Result<&'static Settings> {
    let default = config::File::from(Path::new("./configs/default.toml")).required(false);
    let mut builder = Config::builder().add_source(default);
    builder = builder;

    // 在测试中，会默认传入多个测试相关的参数，所以跳过解析
    #[cfg(not(test))]
    {
        let args: Args = Args::parse();
        if let Some(path) = args.config {
            println!("loading settings. path = {:?}", path);
            builder = builder.add_source(config::File::from(path).required(true));
        }

        #[derive(Debug, Serialize)]
        struct CmdSettings {
            init_system: InitSystem,
        }

        let c = CmdSettings {
            init_system: InitSystem {
                register_test_user: args.register_test_user,
            },
        };

        builder = builder.add_source(Config::try_from(&c)?);
    }

    let settings: Settings = builder
        .build()
        .context("cannot load config")?
        .try_deserialize()
        .context("wrong config format")?;

    Ok(SETTINGS.get_or_init(|| settings))
}

pub fn get_settings() -> &'static Settings {
    unsafe { SETTINGS.get().unwrap_unchecked() }
}
