//! # Connection Handler

use crate::cmd::handle_request;
use crate::constants::BUFFER_LEN;
use crate::errors::ConnectionError;
use crate::storage::generic::Crud;
use crate::types::ConcurrentStorageType;
use crate::{debug_and_stderr, log_and_stderr};
use anyhow::Result;
use bytes::BytesMut;
use log::warn;
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
pub async fn handle_connection<KV: Crud, KE: Crud>(
    storage: ConcurrentStorageType<KV, KE>,
    mut socket: TcpStream,
) -> Result<(), ConnectionError> {
    let peer_addr = socket.peer_addr()?;
    log_and_stderr!(debug, "Start handling requests from", peer_addr);

    loop {
        let mut buf = BytesMut::new();
        let n = match socket.read_buf(&mut buf).await {
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
        // [`cmd::handle_request`] will forward the buffer to [`resp::deserialize`] which **depends**
        // on the byte stream **ending in CRLF**.
        buf.truncate(n);
        let response = handle_request(&storage, &buf.freeze()).await?;
        socket.write_all(&response).await?;
        socket.flush().await?;
    }

    debug_and_stderr!("Stop handling requests from", peer_addr);

    Ok(())
}
