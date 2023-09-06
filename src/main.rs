use anyhow::Result;

#[actix_web::main]
async fn main() -> Result<()> {
    av1_cloud::init_global().await?;

    let server = av1_cloud::build_http_server().await?;
    server.await?;

    Ok(())
}
