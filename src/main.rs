use anyhow::Result;
use av1_cloud::logger;
use tracing::info;

#[actix_web::main]
async fn main() -> Result<()> {
    let settings = av1_cloud::setttings::load_settings(None)?;
    logger::init(&settings.log)?;

    info!("starting");

    let server = av1_cloud::build_http_server()?;
    server.await?;

    Ok(())
}
