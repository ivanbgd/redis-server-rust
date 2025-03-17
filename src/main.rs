//! # Redis Application

use anyhow::Result;
use codecrafters_redis::constants::{ExitCode, LOCAL_SOCKET_ADDR_STR};
use codecrafters_redis::errors::ApplicationError;
use codecrafters_redis::requests::handle_request;
use log::{error, info, warn};
use std::process::exit;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), ApplicationError> {
    env_logger::init();
    info!("Starting the server...");

    let listener = TcpListener::bind(LOCAL_SOCKET_ADDR_STR).await?;

    main_loop(listener).await
}

/// Resolve DNS queries
async fn main_loop(listener: TcpListener) -> Result<(), ApplicationError> {
    info!("Waiting for requests...");

    loop {
        let (stream, _) = listener.accept().await?;

        // A new task is spawned for each inbound socket. The socket is
        // moved to the new task and processed there.
        tokio::spawn(async move {
            // Process each socket (stream) concurrently.
            handle_request(stream)
                .await
                .map_err(|e| {
                    warn!("error: {}", e);
                })
                .expect("Failed to handle request");

            shutdown().await;
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
                exit(ExitCode::Shutdown as i32)
            }
        };
    });
}
