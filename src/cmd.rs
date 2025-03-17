//! # Command Handlers
//!
//! [Commands](https://redis.io/docs/latest/commands/)

use anyhow::Result;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

/// Returns `PONG` as a simple string.
///
/// [PING](https://redis.io/docs/latest/commands/ping/)
///
/// [Simple strings](https://redis.io/docs/latest/develop/reference/protocol-spec/#simple-strings)
pub(crate) async fn handle_ping(stream: &mut TcpStream) -> Result<()> {
    stream.write_all(b"+PONG\r\n").await?;

    Ok(())
}
