//! # Connection and Request Handlers
//!
//! [COMMAND](https://redis.io/docs/latest/commands/command/): Redis command names are case-insensitive.

use crate::cmd::{handle_echo, handle_ping};
use crate::constants::BUFFER_LEN;
use crate::errors::ConnectionError;
use anyhow::Result;
use log::trace;
use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;

/// Handles multiple successive requests from the same connection.
pub async fn handle_connection(mut stream: TcpStream) -> Result<(), ConnectionError> {
    trace!("Start handling requests from {}", stream.peer_addr()?);
    eprintln!("Start handling requests from {}", stream.peer_addr()?);

    loop {
        let mut buf = [0u8; BUFFER_LEN];
        let _read = stream.read(&mut buf).await?;

        if let Some(i) = buf
            .windows(b"PING".len())
            .position(|window| window.eq_ignore_ascii_case(b"PING"))
        {
            handle_ping(&mut stream, &buf, i).await?;
        } else if let Some(i) = buf
            .windows(b"ECHO".len())
            .position(|window| window.eq_ignore_ascii_case(b"ECHO"))
        {
            handle_echo(&mut stream, &buf, i).await?;
        } else {
            break;
        }
    }

    trace!("Stop handling requests from {}", stream.peer_addr()?);
    eprintln!("Stop handling requests from {}", stream.peer_addr()?);

    Ok(())
}
