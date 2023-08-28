//! postgres 连接池

use std::sync::OnceLock;
use std::time::Duration;

use anyhow::Result;
use diesel_async::pooled_connection::deadpool::{Object, Pool};
use diesel_async::pooled_connection::AsyncDieselConnectionManager;
use diesel_async::AsyncPgConnection;
use serde::{Deserialize, Serialize};

/// pg pool connection
pub type PgConn = Object<AsyncPgConnection>;
type PgPool = Pool<AsyncPgConnection>;

static POOL: OnceLock<PgPool> = OnceLock::new();

/// 连接池配置
#[derive(Debug, Serialize, Deserialize)]
pub struct PgPoolConfig {
    /// 最小连接数，当连接池中存活的连接数小于这个值时，会自动建立新连接
    pub min_conn: u32,
    /// 最大连接数
    pub max_conn: u32,
    /// postgres 数据库地址
    pub url: String,
}

impl Default for PgPoolConfig {
    fn default() -> Self {
        Self {
            min_conn: 1,
            max_conn: 10,
            url: "postgresql://postgres:postgres@127.0.0.1:5432/postgres".to_string(),
        }
    }
}

/// 初始化一个静态的 postgres 连接池
pub async fn init(config: &PgPoolConfig) -> Result<()> {
    let url_cfg = AsyncDieselConnectionManager::<diesel_async::AsyncPgConnection>::new(&config.url);
    let pool = Pool::builder(url_cfg)
        .max_size(config.max_conn as usize)
        .build()?;
    let _conn = pool.get().await?;

    POOL.get_or_init(|| {
        tokio::spawn(async move {
            // 每 30 秒检查一次，删除超过一分钟没使用的连接
            let interval = Duration::from_secs(30);
            let max_age = Duration::from_secs(60);
            loop {
                tokio::time::sleep(interval).await;

                unsafe {
                    // SAFETY: 这里只会在初始化后才会调用
                    POOL.get()
                        .unwrap_unchecked()
                        .retain(|_, metrics| metrics.last_used() < max_age);
                }
            }
        });

        pool
    });

    Ok(())
}

/// 获取一个 postgres 连接
///
/// # Panics
///
/// 在成功调用 [`init`] 之前调用这个函数，将会 panic
pub async fn pg_conn() -> Result<PgConn> {
    let conn = POOL.get().unwrap().get().await?;
    Ok(conn)
}

#[cfg(test)]
mod test {
    use super::*;
    use diesel::sql_types::Int4;
    use diesel::{sql_query, QueryableByName};
    use diesel_async::RunQueryDsl;

    #[tokio::test]
    async fn test_pg_pool() -> Result<()> {
        #[derive(Debug, QueryableByName)]
        pub struct User {
            #[diesel(sql_type = Int4)]
            pub id: i32,
        }

        init(&PgPoolConfig::default()).await?;
        let mut conn = pg_conn().await?;

        let a: User = sql_query("select 1 as id")
            .get_result(&mut conn)
            .await
            .unwrap();
        dbg!(a);
        Ok(())
    }
}
