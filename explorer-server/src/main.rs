use std::{collections::HashMap, fs, sync::Arc};

use axum::{
    extract::Path,
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
    routing::{get, get_service, MethodRouter},
    Extension, Json, Router,
};
use bitcoinsuite_chronik_client::ChronikClient;
use bitcoinsuite_error::Result;
use futures::future::ready;
use server::Server;
use server_error::{to_server_error, ServerError};
use server_primitives::{JsonBlocksResponse, JsonTxsResponse};
use tower_http::services::ServeDir;

mod api;
mod blockchain;
mod config;
mod server;
mod server_error;
mod server_primitives;
mod templating;

async fn homepage(server: Extension<Arc<Server>>) -> Result<Html<String>, ServerError> {
    Ok(Html(server.homepage().await.map_err(to_server_error)?))
}

async fn blocks(server: Extension<Arc<Server>>) -> Result<Html<String>, ServerError> {
    Ok(Html(server.blocks().await.map_err(to_server_error)?))
}

async fn tx(
    Path(hash): Path<String>,
    server: Extension<Arc<Server>>,
) -> Result<Html<String>, ServerError> {
    Ok(Html(server.tx(&hash).await.map_err(to_server_error)?))
}

async fn block(
    Path(hash): Path<String>,
    server: Extension<Arc<Server>>,
) -> Result<Html<String>, ServerError> {
    Ok(Html(server.block(&hash).await.map_err(to_server_error)?))
}

async fn address(
    Path(hash): Path<String>,
    server: Extension<Arc<Server>>,
) -> Result<Html<String>, ServerError> {
    Ok(Html(server.address(&hash).await.map_err(to_server_error)?))
}

async fn address_qr(
    Path(hash): Path<String>,
    server: Extension<Arc<Server>>,
) -> Result<impl IntoResponse, ServerError> {
    let qr_code = server.address_qr(&hash).await.map_err(to_server_error)?;
    Ok((StatusCode::OK, [("content-type", "image/png")], qr_code))
}

async fn block_height(
    Path(height): Path<u32>,
    server: Extension<Arc<Server>>,
) -> Result<Redirect, ServerError> {
    Ok(server.block_height(height).await.map_err(to_server_error)?)
}

async fn search(
    Path(query): Path<String>,
    server: Extension<Arc<Server>>,
) -> Result<Redirect, ServerError> {
    server.search(&query).await.map_err(to_server_error)
}

async fn data_blocks(
    Path((start_height, end_height)): Path<(i32, i32)>,
    server: Extension<Arc<Server>>,
) -> Result<Json<JsonBlocksResponse>, ServerError> {
    Ok(Json(
        server
            .data_blocks(start_height, end_height)
            .await
            .map_err(to_server_error)?,
    ))
}

async fn data_block_txs(
    Path(hash): Path<String>,
    server: Extension<Arc<Server>>,
) -> Result<Json<JsonTxsResponse>, ServerError> {
    Ok(Json(
        server
            .data_block_txs(&hash)
            .await
            .map_err(to_server_error)?,
    ))
}

async fn data_address_txs(
    Path(hash): Path<String>,
    Path(query): Path<HashMap<String, String>>,
    server: Extension<Arc<Server>>,
) -> Result<Json<JsonTxsResponse>, ServerError> {
    Ok(Json(
        server
            .data_address_txs(&hash, query)
            .await
            .map_err(to_server_error)?,
    ))
}

fn serve_files(path: &str) -> MethodRouter {
    get_service(ServeDir::new(path)).handle_error(|_| ready(StatusCode::INTERNAL_SERVER_ERROR))
}

#[tokio::main]
async fn main() -> Result<()> {
    let config_string = fs::read_to_string("config.toml")?;
    let config = config::load_config(&config_string)?;

    let chronik = ChronikClient::new(config.chronik_api_url)?;
    let server = Arc::new(Server::setup(chronik).await?);

    let app = Router::new()
        .route("/", get(homepage))
        .route("/tx/:hash", get(tx))
        .route("/blocks", get(blocks))
        .route("/block/:hash", get(block))
        .route("/block-height/:height", get(block_height))
        .route("/address/:hash", get(address))
        .route("/address-qr/:hash", get(address_qr))
        .route("/search/:query", get(search))
        .route("/api/blocks/:start_height/:end_height", get(data_blocks))
        .route("/api/block/:hash/transactions", get(data_block_txs))
        .route("/api/address/:hash/transactions", get(data_address_txs))
        .nest("/code", serve_files("./code"))
        .nest("/assets", serve_files("./assets"))
        .nest("/favicon.ico", serve_files("./assets/favicon.png"))
        .layer(Extension(server));

    axum::Server::bind(&config.host)
        .serve(app.into_make_service())
        .await
        .unwrap();

    Ok(())
}
