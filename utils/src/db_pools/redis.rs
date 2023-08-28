//! redis 连接池

use std::sync::OnceLock;

use anyhow::{Context, Result};
use deadpool_redis::{Pool, Runtime};
use serde::{Deserialize, Serialize};

static POOL: OnceLock<Pool> = OnceLock::new();

#[allow(missing_docs)]
#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub url: String,
    /// redis 最大连接数
    pub max_connection: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            url: String::from("redis://127.0.0.1/"),
            max_connection: 2,
        }
    }
}

/// 初始化一个静态的 redis 连接池
pub async fn init(config: &Config) -> Result<()> {
    let pool = deadpool_redis::Config::from_url(&config.url)
        .builder()?
        .max_size(config.max_connection)
        .runtime(Runtime::Tokio1)
        .build()?;

    // test pool connection
    pool.get().await.context("get init conn")?;

    POOL.get_or_init(|| pool);
    Ok(())
}

/// 获取一个 redis 连接
///
/// # Panics
///
/// 在成功调用 [`init`] 之前调用这个函数，将会 panic
pub async fn redis_conn() -> Result<deadpool_redis::Connection> {
    Ok(POOL.get().unwrap().get().await?)
}

/// 获取一个 redis pool 引用
///
/// # Panics
///
/// 在成功调用 [`init`] 之前调用这个函数，将会 panic
pub fn redis_pool() -> &'static deadpool_redis::Pool {
    POOL.get().unwrap()
}
