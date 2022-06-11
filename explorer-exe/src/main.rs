use std::{fs, sync::Arc};

use axum::Extension;
use bitcoinsuite_chronik_client::ChronikClient;
use bitcoinsuite_error::Result;
use explorer_server::{config, server::Server};

#[tokio::main]
async fn main() -> Result<()> {
    let config_string = fs::read_to_string("config.toml")?;
    let config = config::load_config(&config_string)?;

    let chronik = ChronikClient::new(config.chronik_api_url)?;
    let server = Arc::new(Server::setup(chronik).await?);
    let app = server.router().layer(Extension(server));

    axum::Server::bind(&config.host)
        .serve(app.into_make_service())
        .await
        .unwrap();

    Ok(())
}
