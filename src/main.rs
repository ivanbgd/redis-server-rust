//! # An Implementation of a Redis Server

use anyhow::Result;
use clap::Parser;
use log::info;
use redis::cli::Args;
use redis::constants::LOCAL_SOCKET_ADDR_STR;
use redis::errors::ApplicationError;
use redis::expiry::eviction_loop;
use redis::server::Server;
use redis::storage::Storage;
use redis::types::{InMemoryExpiryTimeHashMap, InMemoryStorageHashMap, StorageType};
use std::sync::{Arc, RwLock};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), ApplicationError> {
    env_logger::init();
    info!("Starting the server...");

    let args = Args::parse();
    let port = args.port;

    let listener = TcpListener::bind(format!("{LOCAL_SOCKET_ADDR_STR}:{port}")).await?;
    let storage = Storage::<
        StorageType<InMemoryStorageHashMap, InMemoryExpiryTimeHashMap>,
        InMemoryStorageHashMap,
        InMemoryExpiryTimeHashMap,
    >::new();

    let storage = Arc::new(RwLock::new(storage));

    let evictor_store = Arc::clone(&storage);
    std::thread::Builder::new()
        .name("evictor-thread".to_string())
        .spawn(move || eviction_loop(evictor_store))?;

    let core_store = Arc::clone(&storage);
    let server = Server::new(listener, core_store).await?;
    server.start().await?;

    Ok(())
}
