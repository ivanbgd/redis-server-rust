//! # Connection Handler(s)
//!
//! [COMMAND](https://redis.io/docs/latest/commands/command/): Redis command names are case-insensitive.

use crate::cmd::{handle_echo, handle_ping};
use crate::constants::BUFFER_LEN;
use crate::errors::ConnectionError;
use anyhow::Result;
use log::{trace, warn};
use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;

/// Handles multiple successive requests from the same connection.
pub async fn handle_connection(mut stream: TcpStream) -> Result<(), ConnectionError> {
    trace!("Start handling requests from {}", stream.peer_addr()?);
    eprintln!("Start handling requests from {}", stream.peer_addr()?);

    let mut buf = [0u8; BUFFER_LEN];

    loop {
        match stream.read(&mut buf).await {
            Ok(0) => break,
            Ok(n) => assert!(0 < n && n <= buf.len()),
            Err(err) => {
                warn!("{}", err);
                return Err(ConnectionError::from(err));
            }
        }

        for (i, _) in buf.iter().enumerate() {
            if buf[i..].to_ascii_uppercase().starts_with(b"PING") {
                handle_ping(&mut stream, &buf, i).await?;
            } else if buf[i..].to_ascii_uppercase().starts_with(b"ECHO") {
                handle_echo(&mut stream, &buf, i).await?;
            }
        }
    }

    trace!("Stop handling requests from {}", stream.peer_addr()?);
    eprintln!("Stop handling requests from {}", stream.peer_addr()?);

    Ok(())
}
