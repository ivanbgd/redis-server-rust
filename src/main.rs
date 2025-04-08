//! # An Implementation of a Redis Server

use anyhow::Result;
use clap::Parser;
use log::info;
use redis_server::cli::Args;
use redis_server::constants::LOCAL_SOCKET_ADDR_STR;
use redis_server::errors::ApplicationError;
use redis_server::expiry::eviction_loop;
use redis_server::server::Server;
use redis_server::storage::Storage;
use redis_server::types::{InMemoryExpiryTimeHashMap, InMemoryStorageHashMap, StorageType};
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
