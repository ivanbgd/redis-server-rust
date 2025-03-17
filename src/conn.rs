//! # Connection and Request Handlers

use crate::cmd::handle_ping;
use crate::errors::ConnectionError;
use anyhow::Result;
use log::trace;
use tokio::net::TcpStream;

pub async fn handle_connection(mut stream: TcpStream) -> Result<(), ConnectionError> {
    trace!("Start handling request from {}", stream.peer_addr()?);

    handle_ping(&mut stream).await?;

    trace!("Stop handling request from {}", stream.peer_addr()?);

    Ok(())
}
