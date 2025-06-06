//! The Redis Server

use crate::cli::Args;
use crate::conn::handle_connection;
use crate::constants::{ExitCode, CONNECTION_PERMIT_TIMEOUT_MS};
use crate::constants::{LOCAL_SOCKET_ADDR_STR, SHUTDOWN_TIME_MS};
use crate::errors::ServerError;
use crate::log_and_stderr;
use crate::storage::generic::Crud;
use crate::types::{ConcurrentStorageType, ExpirationTime, StorageKey};
use anyhow::Result;
use log::{debug, error, info, warn};
use std::fmt::Debug;
use std::process::exit;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio::time::timeout;

/// Redis server
#[derive(Debug)]
pub struct Server<KV, KE> {
    listener: TcpListener,
    max_conn: Arc<Semaphore>,
    storage: ConcurrentStorageType<KV, KE>,
}

impl<
        KV: 'static + Crud + Send + Sync + Debug,
        KE: 'static
            + Clone
            + Crud
            + Send
            + Sync
            + Debug
            + IntoIterator<Item = (StorageKey, ExpirationTime)>,
    > Server<KV, KE>
{
    /// Create an instance of the Redis server
    pub async fn new(
        args: Args,
        storage: ConcurrentStorageType<KV, KE>,
    ) -> Result<Self, ServerError> {
        let port = args.port;
        let max_conn = args.max_conn;

        let listener = TcpListener::bind(format!("{LOCAL_SOCKET_ADDR_STR}:{port}")).await?;
        let addr = listener.local_addr()?;
        log_and_stderr!(info, "Listening on", addr);

        let max_conn = Arc::new(Semaphore::new(max_conn));

        Ok(Self {
            listener,
            max_conn,
            storage,
        })
    }

    /// Start the server
    ///
    /// Starts the async core thread.
    pub async fn start(&self) -> Result<(), ServerError> {
        self.core_loop().await
    }

    /// Resolve Redis queries
    ///
    /// Supports multiple concurrent clients in addition to multiple requests from the same connection.
    async fn core_loop(&self) -> Result<(), ServerError> {
        debug!("Starting the core loop...");
        info!("Waiting for requests...");
        let storage = &self.storage;

        Self::shutdown(SHUTDOWN_TIME_MS).await;

        loop {
            match self.acquire_socket_permit().await {
                Ok((mut socket, permit)) => {
                    let storage = Arc::clone(storage);

                    // A new task is spawned for each inbound socket. The socket is moved to the new task and processed there.
                    tokio::spawn(async move {
                        // Process each socket (stream) concurrently.
                        // Each connection can process multiple successive requests (commands) from the same client.
                        handle_connection(storage, &mut socket)
                            .await
                            .map_err(|e| {
                                warn!("{e}");
                            })
                            .expect("Failed to handle request");
                        // Drop socket while the permit is still alive.
                        drop(socket);
                        // Drop the permit so more tasks can be created.
                        drop(permit);
                    });
                }
                Err(e) => {
                    warn!("{e}");
                }
            };
        }
    }

    /// Tries to acquire a permit for a connection socket
    ///
    /// It only tries to do that in case there is an incoming connection.
    ///
    /// If there is an incoming connection, tries to acquire a permit from semaphore within a predefined time interval.
    ///
    /// So, it doesn't try to acquire permit if there is no incoming connection.
    ///
    /// # Returns
    ///
    /// Returns a tuple of `(TcpStream, OwnedSemaphorePermit)`.
    ///
    /// # Errors
    /// - [`ServerError::IoError`] in case a new incoming connection from this listener could not be accepted
    /// - [`ServerError::ElapsedError`] in case permit could not be obtained on time
    /// - [`ServerError::AcquireError`] in case permit could not be obtained because semaphore has been closed
    async fn acquire_socket_permit(
        &self,
    ) -> Result<(TcpStream, OwnedSemaphorePermit), ServerError> {
        match self.listener.accept().await {
            Ok((socket, _)) => {
                match timeout(
                    Duration::from_millis(CONNECTION_PERMIT_TIMEOUT_MS),
                    self.max_conn.clone().acquire_owned(),
                )
                .await
                {
                    Ok(Ok(permit)) => Ok((socket, permit)),
                    Ok(Err(e)) => Err(ServerError::AcquireError(e)),
                    Err(e) => {
                        drop(socket);
                        Err(ServerError::ElapsedError(
                            format!("{e} ({CONNECTION_PERMIT_TIMEOUT_MS} ms)").to_string(),
                        ))
                    }
                }
            }
            Err(e) => Err(ServerError::IoError(e)),
        }
    }

    /// Await the shutdown signal
    ///
    /// Gives the server time to finish an ongoing operation for a graceful shutdown.
    async fn shutdown(time_to_wait: u64) {
        tokio::spawn(async move {
            match tokio::signal::ctrl_c().await {
                Ok(()) => {
                    info!("CTRL+C received. Shutting down gracefully...");
                    tokio::time::sleep(Duration::from_millis(time_to_wait)).await;
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
