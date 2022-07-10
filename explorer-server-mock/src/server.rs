use axum::Extension;
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener},
    sync::Arc,
};

use bitcoinsuite_chronik_client::ChronikClient;
use bitcoinsuite_error::Result;
use explorer_server::server::Server;

pub async fn setup(chronik_url: String) -> Result<Server> {
    let chronik = ChronikClient::new(chronik_url).expect("Impossive");
    Ok(Server::setup(chronik).await?)
}

pub async fn setup_and_run(chronik_url: String) -> Result<String> {
    let server = Arc::new(setup(chronik_url).await?);
    let app = server.router().layer(Extension(server));

    let free_port = bitcoinsuite_test_utils::pick_ports(1)?[0];
    let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), free_port);
    let tcp_listener = TcpListener::bind(address)?;

    tokio::spawn(async move {
        axum::Server::from_tcp(tcp_listener)
            .expect("Impossible")
            .serve(app.into_make_service())
            .await
            .unwrap();
    });

    Ok(address.to_string())
}
