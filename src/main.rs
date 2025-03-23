//! # An Implementation of a Redis Server

use anyhow::Result;
use codecrafters_redis::constants::{StorageType, LOCAL_SOCKET_ADDR_STR};
use codecrafters_redis::errors::ApplicationError;
use codecrafters_redis::server::Server;
use codecrafters_redis::storage::Storage;
use log::info;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), ApplicationError> {
    env_logger::init();
    info!("Starting the server...");

    let listener = TcpListener::bind(LOCAL_SOCKET_ADDR_STR).await?;
    let storage = Storage::<StorageType>::new();

    let server = Server::new(listener, storage).await?;
    server.start().await?;

    Ok(())
}
