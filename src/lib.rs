use actix_cors::Cors;
use actix_identity::IdentityMiddleware;
use actix_session::{config::PersistentSession, storage::RedisSessionStore, SessionMiddleware};
use actix_web::{
    cookie::{time::Duration, Key},
    dev::Server,
    web, App, HttpServer,
};
use anyhow::{Context, Result};
use settings::get_settings;
use tracing::info;

use crate::{
    presentation::{employee, user},
    settings::load_settings,
};

pub mod application;
pub mod domain;
pub mod infrastructure;
mod presentation;

pub mod http;
pub mod logger;
mod schema;
pub mod settings;

pub use redis_conn_switch::*;
pub mod redis_conn_switch {
    #[cfg(feature = "keydb")]
    pub use utils::db_pools::keydb::keydb_conn as redis_conn;
    #[cfg(feature = "redis")]
    pub use utils::db_pools::redis::redis_conn;
}

pub async fn build_http_server() -> Result<Server> {
    let settings = &get_settings().http_server;
    info!(?settings, "building http server. Powered by actix-web!");

    let store = RedisSessionStore::new(&settings.session.url).await?;
    let server: Server = HttpServer::new(move || {
        let sss = build_session_mw(store.clone());
        let cors = Cors::permissive();
        App::new()
            .configure(user::config)
            .configure(employee::config)
            .route("/ping", web::get().to(http_ping))
            .wrap(IdentityMiddleware::default())
            .wrap(sss)
            .wrap(cors)
    })
    .bind((&*settings.bind, settings.port))?
    .run();

    Ok(server)
}

fn build_session_mw(store: RedisSessionStore) -> SessionMiddleware<RedisSessionStore> {
    let config = &get_settings().http_server.session;
    // TODO: load from configure or env
    let key: Vec<u8> = (0..64).collect();
    let life =
        PersistentSession::default().session_ttl(Duration::seconds(config.max_age_secs as i64));
    let key = Key::try_from(&*key).unwrap();
    let session = SessionMiddleware::builder(store.clone(), key.clone())
        .cookie_secure(config.secure)
        .cookie_http_only(config.http_only)
        .session_lifecycle(life)
        .build();
    session
}

pub async fn http_ping() -> &'static str {
    "pong"
}

pub async fn init_global() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();

    let cfg_path = if args.len() > 1 {
        // 在测试中，会默认传入多个参数
        #[cfg(test)]
        {
            None
        }
        #[cfg(not(test))]
        {
            Some(&*args[1])
        }
    } else {
        None
    };

    let settings = load_settings(cfg_path)?;
    logger::init(&settings.log)?;

    infrastructure::email::load_email_code_template().context("load email-code-template")?;

    utils::db_pools::postgres::init(&settings.postgres).await?;
    {
        #[cfg(feature = "keydb")]
        utils::db_pools::keydb::init(&settings.redis).await?;
        #[cfg(feature = "redis")]
        utils::db_pools::redis::init(&settings.redis).await?;
    }

    info!("global environment loaded");
    Ok(())
}
