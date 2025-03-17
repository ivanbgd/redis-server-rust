//! # Command Handlers
//!
//! [Commands](https://redis.io/docs/latest/commands/)
//!
//! [COMMAND](https://redis.io/docs/latest/commands/command/): Redis command names are case-insensitive.

use crate::errors::RequestError;
use anyhow::Result;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

/// Handler for the `PING` command
///
/// Writes to the provided TCP stream.
///
/// Writes `PONG` as a simple string if no argument is provided, otherwise writes a copy of the argument as a bulk.
///
/// [PING](https://redis.io/docs/latest/commands/ping/)
///
/// [Simple strings](https://redis.io/docs/latest/develop/reference/protocol-spec/#simple-strings)
pub(crate) async fn handle_ping(
    stream: &mut TcpStream,
    buf: &[u8],
    i: usize,
) -> Result<(), RequestError> {
    let aux_res;
    let result = if buf[i..][..b"PING\r\n".len()].eq_ignore_ascii_case(b"PING\r\n") {
        b"+PONG\r\n"
    } else {
        let rest = buf[i + b"PING".len()..].trim_ascii_start();
        let ind = rest
            .windows(b"\r\n".len())
            .position(|w| w == b"\r\n")
            .ok_or_else(|| RequestError::MissingCRLF)?;
        let argument = String::from_utf8_lossy(&rest[..ind]);
        aux_res = format!("${}\r\n{argument}\r\n", argument.len());
        aux_res.as_bytes()
    };

    Ok(stream.write_all(result).await?)
}
