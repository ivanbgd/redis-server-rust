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

    let mut cursor = 0usize;
    while let Some(i) = buf[cursor..]
        .windows(b"PING".len())
        .position(|window| window.eq_ignore_ascii_case(b"PING"))
    {
        let len = handle_ping(&mut stream, &buf[cursor..], i).await?;
        cursor += i + len;
    }

    trace!("Stop handling request from {}", stream.peer_addr()?);

    Ok(())
}
