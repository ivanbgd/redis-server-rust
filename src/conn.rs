//! # Connection Handler

use crate::cmd::handle_request;
use crate::constants::BUFFER_LEN;
use crate::errors::ConnectionError;
use anyhow::Result;
use bytes::Bytes;
use log::{trace, warn};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// Handles multiple successive requests from the same connection.
///
/// Since a single request is always an array, it can contain multiple commands. This is called
/// [pipelining](https://redis.io/docs/latest/develop/reference/protocol-spec/#multiple-commands-and-pipelining).
/// Pipelining enables clients to send multiple commands at once and wait for replies later.
/// A client can use the same connection to issue multiple commands.
/// Pipelining is supported, so multiple commands can be sent with a single write operation by the client.
/// The client can skip reading replies and continue to send the commands one after the other.
/// All the replies can be read at the end.
/// For more information, see [Pipelining](https://redis.io/docs/latest/develop/use/pipelining/).
pub async fn handle_connection(mut stream: TcpStream) -> Result<(), ConnectionError> {
    let peer_addr = stream.peer_addr()?;
    trace!("Start handling requests from {}", peer_addr);
    eprintln!("Start handling requests from {}", peer_addr);

    let mut buf = [0u8; BUFFER_LEN];

    loop {
        let n = match stream.read(&mut buf).await {
            Ok(0) => break,
            Ok(n) => {
                assert!(0 < n && n <= buf.len());
                n
            }
            Err(err) => {
                warn!("{}", err);
                return Err(ConnectionError::from(err));
            }
        };
        // [`cmd::handle_request`] will forward the buffer to [`resp::deserialize`] which depends on the byte stream
        // ending in CRLF, beside also being cheaper to copy only the necessary elements.
        let bytes = Bytes::copy_from_slice(&buf[..n]);
        dbg!(&bytes, n);
        let response = handle_request(&bytes).await?;
        dbg!(&response);
        stream.write_all(&response).await?;
        stream.flush().await?;
    }

    trace!("Stop handling requests from {}", peer_addr);
    eprintln!("Stop handling requests from {}", peer_addr);

    Ok(())
}
