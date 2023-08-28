pub mod user;

#[macro_export]
macro_rules! ensure_ok {
    ($predict:expr, $err:expr) => {
        if !$predict {
            return Err($err);
        }
    };
}

#[macro_export]
macro_rules! ensure_biz {
    ($predict:expr, $err:expr) => {
        if !$predict {
            return Ok(Err($err));
        }
    };

    ($call:expr) => {
        match $call {
            Ok(value) => value,
            Err(err) => return Ok(Err(err.into())),
        }
    };
}

#[macro_export]
macro_rules! ensure_exist {
    ($predict:expr, $err:expr) => {
        match $predict {
            Some(v) => v,
            None => return Ok(Err($err)),
        }
    };
}

#[macro_export]
macro_rules! biz_ok {
    ($value:expr) => {
        Ok(Ok($value))
    };
}

#[macro_export]
macro_rules! biz_err {
    ($value:expr) => {
        Ok(Err($value))
    };
}

pub enum AppTxError<T> {
    Anyhow(anyhow::Error),
    Biz(T),
}

impl<T> From<diesel::result::Error> for AppTxError<T> {
    fn from(value: diesel::result::Error) -> Self {
        Self::Anyhow(value.into())
    }
}

#[macro_export]
macro_rules! pg_tx {
    ($func:path, $($params:expr),*) => {{
        use diesel_async::AsyncConnection;
        use crate::application::AppTxError;
        use diesel_async::scoped_futures::ScopedFutureExt;
        use crate::biz_err;

        let mut conn = utils::db_pools::postgres::pg_conn().await?;
        let res = conn
            .transaction(|conn| {
                async {
                    let res =
                    match $func($($params),*, conn).await {
                        Ok(ok) => match ok {
                            Ok(ok) => Ok(ok),
                            Err(err) => Err(AppTxError::Biz(err)),
                        },
                        Err(err) => Err(AppTxError::Anyhow(err)),
                    };
                    res
                }
                .scope_boxed()
            })
            .await;

        match res {
            Ok(ok) => {
               crate::biz_ok!(ok)
            }
            Err(err) => match err {
                AppTxError::Anyhow(anyhow) => {
                    Err(anyhow)
                },
                AppTxError::Biz(biz) => {
                    biz_err!(biz)
                },
            },
        }
    }};

}

#[macro_export]
macro_rules! tx_func {
    ($func:path, $($params:expr),*) => {{
        use diesel_async::AsyncConnection;
        use diesel_async::scoped_futures::ScopedFutureExt;

        let mut conn = utils::db_pools::postgres::pg_conn().await?;
        conn
           .transaction(|conn| {
               async {
                    let res = $func($($params),*, conn).await;
                    // if res.is_ok() {
                    //     if conn.version > 0 {
                    //         if let Err(err) = crate::infrastructrure::version::confirm_version(conn.version).await {
                    //             tracing::warn!(?err, version, "failed to sync service version");
                    //         } else {
                    //             tracing::debug!(version, "service version sync successfully");
                    //         }
                    //     }
                    // } else {
                    //     tracing::warn!("no need to sync version [anyhow err]");
                    // }
                    res
               }
               .scope_boxed()
           })
           .await
    }};
}
