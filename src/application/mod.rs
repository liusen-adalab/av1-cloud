pub mod email;
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
    (not $predict:expr, $err:expr) => {
        if $predict {
            return Ok(Err($err.into()));
        }
    };

    ($predict:expr, $err:expr) => {
        if !$predict {
            return Ok(Err($err.into()));
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
            None => return Ok(Err($err.into())),
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

#[macro_export]
macro_rules! pg_tx {
    ($func:path, $($params:expr),*) => {{
        use diesel_async::AsyncConnection;
        use diesel_async::scoped_futures::ScopedFutureExt;

        let mut conn = utils::db_pools::postgres::pg_conn().await?;
        let res = conn
            .transaction(|conn| {
                async {
                    if false {
                        return Err(anyhow::anyhow!(""))
                    }
                    $func($($params),*, conn).await
                }
                .scope_boxed()
            })
            .await;
        res
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
                    res
               }
               .scope_boxed()
           })
           .await
    }};
}
