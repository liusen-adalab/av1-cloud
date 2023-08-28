use std::sync::OnceLock;

use anyhow::{ensure, Result};
use redis::{cluster::ClusterClient, cluster_async::ClusterConnection};
use serde::{Deserialize, Serialize};

#[allow(missing_docs)]
#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub keydb_urls: Vec<String>,
}

static KEYDB_POOL: OnceLock<ClusterClient> = OnceLock::new();

/// 初始化一个静态的 redis 连接池
pub async fn init(config: &Config) -> Result<()> {
    ensure!(!config.keydb_urls.is_empty(), "no keydb urls");

    let client = ClusterClient::new(config.keydb_urls.clone())?;
    client.get_async_connection().await?;

    KEYDB_POOL.get_or_init(|| client);

    Ok(())
}

/// 获取一个 keydb 连接
///
/// # Panics
///
/// 在成功调用 [`init_keydb`] 之前调用这个函数，将会 panic
pub async fn keydb_conn() -> Result<ClusterConnection> {
    let c = KEYDB_POOL.get().unwrap().get_async_connection().await?;
    Ok(c)
}
