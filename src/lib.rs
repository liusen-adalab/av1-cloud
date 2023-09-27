use actix_casbin_auth::{
    casbin::{function_map::key_match2, CoreApi, DefaultModel, FileAdapter},
    CasbinService,
};
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
use utils::logger;

use crate::{
    application::file_system,
    presentation::{employee, user},
    settings::load_settings,
};

pub mod application;
pub mod domain;
pub mod infrastructure;
mod presentation;

pub mod auth;
mod cqrs;
pub mod http;
mod schema;
pub mod settings;

pub use redis_conn_switch::*;
pub mod redis_conn_switch {
    #[cfg(feature = "keydb")]
    pub use utils::db_pools::keydb::keydb_conn as redis_conn;
    #[cfg(feature = "redis")]
    pub use utils::db_pools::redis::redis_conn;
}

pub type LocalDataTime = chrono::DateTime<chrono::Local>;

pub async fn build_http_server() -> Result<Server> {
    let settings = &get_settings().http_server;
    info!(?settings, "building http server. Powered by actix-web!");

    let casbin_middleware = build_casbin_mw().await?;

    let store = RedisSessionStore::new(&settings.session.url).await?;
    let server: Server = HttpServer::new(move || {
        let session = build_session_mw(store.clone());
        let cors = Cors::permissive();
        App::new()
            .configure(presentation::config)
            .configure(user::config)
            .configure(employee::config)
            .configure(cqrs::actix_config)
            .configure(presentation::file_system::actix_config)
            .configure(presentation::transcode::config)
            .route("/ping", web::get().to(http_ping))
            .wrap(casbin_middleware.clone())
            .wrap(auth::RoleExtractor)
            .wrap(IdentityMiddleware::default())
            .wrap(session)
            .wrap(cors)
    })
    .bind((&*settings.bind, settings.port))?
    .run();

    Ok(server)
}

async fn build_casbin_mw() -> Result<CasbinService, anyhow::Error> {
    let m = DefaultModel::from_file("configs/rbac.conf").await.unwrap();
    let a = FileAdapter::new("configs/rbac.csv");
    let casbin_middleware = CasbinService::new(m, a).await?;
    casbin_middleware
        .write()
        .await
        .get_role_manager()
        .write()
        .matching_fn(Some(key_match2), None);
    Ok(casbin_middleware)
}

pub async fn t_user() -> &'static str {
    "t_user"
}

pub async fn t_admin() -> &'static str {
    "t_admin"
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
    let settings = load_settings().context("load settings")?;
    logger::init(&settings.log)?;

    infrastructure::email::load_email_code_template().context("load email-code-template")?;

    utils::db_pools::postgres::init(&settings.postgres)
        .await
        .context("init pg pool")?;
    {
        #[cfg(feature = "keydb")]
        utils::db_pools::keydb::init(&settings.redis)
            .await
            .context("init keydb pool")?;
        #[cfg(feature = "redis")]
        utils::db_pools::redis::init(&settings.redis)
            .await
            .context("init redis pool")?;
    }

    if settings.init_system.register_test_user {
        application::user::employee::register_root().await?;
        application::user::register_test_user().await?;
    }

    file_system::init().await.context("init file-system")?;

    info!("global environment loaded");
    Ok(())
}
