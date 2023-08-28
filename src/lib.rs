use actix_web::{dev::Server, web, App, HttpServer};
use anyhow::Result;
use setttings::get_settings;
use tracing::info;

pub mod http;
pub mod logger;
pub mod setttings;
pub mod user;

pub fn build_http_server() -> Result<Server> {
    let settings = &get_settings().http_server;
    info!(?settings, "building http server. Powered by actix-web!");

    let server: Server = HttpServer::new(|| {
        App::new()
            .configure(user::interface::config)
            .route("/ping", web::get().to(http_ping))
    })
    .bind((&*settings.bind, settings.port))?
    .run();

    Ok(server)
}

pub async fn http_ping() -> &'static str {
    "pong"
}
