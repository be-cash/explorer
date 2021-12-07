use std::{collections::HashMap, convert::Infallible, sync::Arc, fs};

use anyhow::Result;
use indexdb::IndexDb;
use indexer::{Indexer, IndexerProduction};
use server::Server;
use warp::{Filter, Rejection, Reply, hyper::StatusCode};
use serde::Serialize;

mod blockchain;
mod grpc;
mod server;
mod server_primitives;
mod indexdb;
mod indexer;
mod primitives;
mod config;
mod templating;

type ServerRef = Arc<Server>;

fn with_server(
    server: &ServerRef,
) -> impl Filter<Extract = (ServerRef,), Error = std::convert::Infallible> + Clone {
    let server = Arc::clone(&server);
    warp::any().map(move || Arc::clone(&server))
}

#[tokio::main]
async fn main() -> Result<()> {
    let config_string = fs::read_to_string("config.toml")?;
    let config = config::load_config(&config_string)?;

    //let db = Db::open("../db.sled")?;
    let indexdb = IndexDb::open(&config.index_database)?;

    let indexer: Arc<dyn Indexer> = Arc::new(IndexerProduction::connect(indexdb).await?);

    tokio::spawn({
        let indexer = Arc::clone(&indexer);
        indexer.run_indexer()
    });

    let server = Arc::new(Server::setup(indexer).await?);

    let dashboard = warp::path::end()
        .and(with_server(&server))
        .and_then(|server: ServerRef| async move {
            server.homepage().await.map_err(err)
        });

    let blocks = warp::path!("blocks")
        .and(with_server(&server))
        .and_then(|server: ServerRef| async move {
            server.blocks().await.map_err(err)
        });

    let block = warp::path!("block" / String)
        .and(with_server(&server))
        .and_then(|block_hash: String, server: ServerRef| async move {
            server.block(&block_hash).await.map_err(err)
        });

    let block_height = warp::path!("block-height" / u32)
        .and(with_server(&server))
        .and_then(|block_height: u32, server: ServerRef| async move {
            server.block_height(block_height).await.map_err(err)
        });
    
    let tx = warp::path!("tx" / String)
        .and(with_server(&server))
        .and_then(|tx_hash: String, server: ServerRef| async move {
            server.tx(&tx_hash).await.map_err(err)
        });
    
    let address = warp::path!("address" / String)
        .and(with_server(&server))
        .and(warp::query::<HashMap<String, String>>())
        .and_then(|address: String, server: ServerRef, query: HashMap<String, String>| async move {
            server.address(&address, query).await.map_err(err)
        });

    let address_qr = warp::path!("address-qr" / String)
        .and(with_server(&server))
        .and_then(|address: String, server: ServerRef| async move {
            server.address_qr(&address).await.map_err(err)
        });

    let search = warp::path!("search" / String)
        .and(with_server(&server))
        .and_then(|query: String, server: ServerRef| async move {
            server.search(&query).await.map_err(err)
        });

    let data_blocks =
        warp::path!("api" / "blocks" / u32 / u32 )
        .and(with_server(&server))
        .and_then(|start_height, end_height, server: ServerRef| async move {
            server.data_blocks(start_height, end_height).await.map_err(err)
        });

    let data_block_txs =
        warp::path!("data" / "block" / String / "dat.js")
        .and(with_server(&server))
        .and_then(|block_hash: String, server: ServerRef| async move {
            server.data_block_txs(&block_hash).await.map_err(err)
        });

    let js = warp::path("code")
        .and(warp::fs::dir("./code"));

    let favicon = warp::path!("favicon.ico")
        .and(warp::fs::file("./assets/favicon.png"));

    let assets = warp::path("assets")
        .and(warp::fs::dir("./assets/"));

    let routes = dashboard
        .or(blocks)
        .or(block)
        .or(block_height)
        .or(tx)
        .or(address)
        .or(address_qr)
        .or(search)
        .or(data_blocks)
        .or(data_block_txs)
        .or(js)
        .or(favicon)
        .or(assets)
        .recover(handle_rejection);

    warp::serve(routes).run(config.host).await;

    Ok(())
}

#[derive(Debug)]
struct AnyhowError(anyhow::Error);
impl warp::reject::Reject for AnyhowError {}
fn err(err: anyhow::Error) -> Rejection {
    warp::reject::custom(AnyhowError(err))
}

#[derive(Serialize)]
struct ErrorMessage {
    success: bool,
    msg: String,
}

async fn handle_rejection(err: Rejection) -> Result<impl Reply, Infallible> {
    let msg;
    if let Some(AnyhowError(anyhow_error)) = err.find::<AnyhowError>() {
        println!("Anyhow error: {:?}", anyhow_error);
        msg = anyhow_error.to_string();
    } else {
        println!("Other error: {:?}", err);
        msg = "Unknown message".to_string();
    }
    return Ok(warp::reply::with_status(
        warp::reply::json(&ErrorMessage {
            success: false,
            msg,
        }),
        StatusCode::INTERNAL_SERVER_ERROR,
    ));
}
