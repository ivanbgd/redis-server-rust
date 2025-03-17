//! # Request Handlers
//!
//! [COMMAND](https://redis.io/docs/latest/commands/command/): Redis command names are case-insensitive.

use crate::cmd::handle_ping;
use crate::constants::BUFFER_LEN;
use crate::errors::RequestError;
use anyhow::Result;
use log::trace;
use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;

/// Handles a request.
pub async fn handle_request(mut stream: TcpStream) -> Result<(), RequestError> {
    trace!("Start handling request from {}", stream.peer_addr()?);

    let mut buf = [0u8; BUFFER_LEN];
    let _read = stream.read(&mut buf).await?;

    if let Some(i) = buf
        .windows(b"PING".len())
        .position(|window| window.eq_ignore_ascii_case(b"PING"))
    {
        handle_ping(&mut stream, &buf, i).await?;
    }

    trace!("Stop handling request from {}", stream.peer_addr()?);

    Ok(())
}
