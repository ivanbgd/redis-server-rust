//! The Redis Server

use crate::conn::handle_connection;
use crate::constants::{ConcurrentStorageType, ExitCode, StorageType};
use crate::errors::ApplicationError;
use crate::log_and_stderr;
use anyhow::Result;
use log::{error, info, warn};
use std::process::exit;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::RwLock;

/// Redis server
#[derive(Debug)]
pub struct Server {
    listener: TcpListener,
    storage: ConcurrentStorageType,
}

impl Server {
    /// Create an instance of the Redis server
    pub async fn new(
        listener: TcpListener,
        storage: StorageType,
    ) -> Result<Self, ApplicationError> {
        let addr = listener.local_addr()?;
        log_and_stderr!(info, "Listening on", addr);

        Ok(Self {
            listener,
            storage: Arc::new(RwLock::new(storage)),
        })
    }

    /// Start the server
    pub async fn start(&self) -> Result<(), ApplicationError> {
        self.main_loop().await
    }

    /// Resolve Redis queries
    ///
    /// Supports multiple concurrent clients in addition to multiple requests from the same connection.
    async fn main_loop(&self) -> Result<(), ApplicationError> {
        info!("Waiting for requests...");
        let storage = &self.storage;

        loop {
            let storage = Arc::clone(storage);
            let (stream, _) = self.listener.accept().await?;

            // A new task is spawned for each inbound socket. The socket is moved to the new task and processed there.
            tokio::spawn(async move {
                // Process each socket (stream) concurrently.
                // Each connection can process multiple successive requests (commands) from the same client.
                handle_connection(storage, stream)
                    .await
                    .map_err(|e| {
                        warn!("error: {}", e);
                    })
                    .expect("Failed to handle request");

                Self::shutdown().await;
            });
        }
    }

    /// Await the shutdown signal
    async fn shutdown() {
        tokio::spawn(async move {
            match tokio::signal::ctrl_c().await {
                Ok(()) => {
                    info!("CTRL+C received. Shutting down...");
                    exit(ExitCode::Ok as i32);
                }
                Err(err) => {
                    // We also shut down in case of error.
                    error!("Unable to listen for the shutdown signal: {}", err);
                    error!("Terminating the app ({})...", ExitCode::Shutdown as i32);
                    exit(ExitCode::Shutdown as i32);
                }
            };
        });
    }
}
