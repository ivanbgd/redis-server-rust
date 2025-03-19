//! # Command (Request) Handlers
//!
//! [Commands](https://redis.io/docs/latest/commands/)
//!
//! [COMMAND](https://redis.io/docs/latest/commands/command/): Redis command names are case-insensitive.
//!
//! [Redis serialization protocol specification](https://redis.io/docs/latest/develop/reference/protocol-spec/)
//!
//! [Sending commands to a Redis server](https://redis.io/docs/latest/develop/reference/protocol-spec/#sending-commands-to-a-redis-server)

use crate::errors::ConnectionError;
use anyhow::Result;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

/// Handler for the `PING` command
///
/// Handles a single `PING` request.
///
/// Writes to the provided TCP stream.
///
/// Writes `PONG` as a simple string if no argument is provided,
/// otherwise writes a copy of the argument as a bulk string.
///
/// [PING](https://redis.io/docs/latest/commands/ping/)
///
/// [Simple strings](https://redis.io/docs/latest/develop/reference/protocol-spec/#simple-strings)
///
/// Example request from a client: `"*1\r\n$4\r\nPING\r\n"`.
/// That's `["PING"]` encoded using the Redis protocol.
///
/// Expected response from the server: `+PONG\r\n` (a simple string)
pub(crate) async fn handle_ping(
    stream: &mut TcpStream,
    buf: &[u8],
    i: usize,
) -> Result<(), ConnectionError> {
    assert!(buf[i..].to_ascii_uppercase().starts_with(b"PING"));

    let aux_res;
    let result = if buf[i..][..b"PING\r\n".len()].eq_ignore_ascii_case(b"PING\r\n") {
        b"+PONG\r\n"
    } else {
        let rest = buf[i + b"PING".len()..].trim_ascii_start();
        let ind = rest
            .windows(b"\r\n".len())
            .position(|w| w == b"\r\n")
            .ok_or_else(|| ConnectionError::CommandMissingCRLF)?;
        let argument = String::from_utf8_lossy(&rest[..ind]);
        aux_res = format!("${}\r\n{argument}\r\n", argument.len());
        aux_res.as_bytes()
    };
    stream.write_all(result).await?;
    stream.flush().await?;

    Ok(())
}

/// Handler for the `ECHO` command
///
/// Handles a single `ECHO` request.
///
/// Writes to the provided TCP stream.
///
/// Writes the message back to the stream as a bulk string.
///
/// [ECHO](https://redis.io/docs/latest/commands/echo/)
///
/// [Bulk strings](https://redis.io/docs/latest/develop/reference/protocol-spec/#bulk-strings)
///
/// Example request from a client: `"*2\r\n$4\r\nECHO\r\n$3\r\nHey\r\n"`.
/// That's `["ECHO", "Hey"]` encoded using the Redis protocol.
///
/// Expected response from the server: `$3\r\nHey\r\n` (a bulk string)
pub(crate) async fn handle_echo(
    stream: &mut TcpStream,
    buf: &[u8],
    i: usize,
) -> Result<(), ConnectionError> {
    assert!(buf[i..].to_ascii_uppercase().starts_with(b"ECHO"));

    let rest = buf[i + b"ECHO".len()..].trim_ascii_start();
    let ind = rest
        .windows(b"\r\n".len())
        .rposition(|w| w == b"\r\n")
        .ok_or_else(|| ConnectionError::CommandMissingCRLF)?;
    let argument = String::from_utf8_lossy(&rest[..ind]);
    let aux_res = format!("{argument}\r\n");
    let result = aux_res.as_bytes();
    stream.write_all(result).await?;
    stream.flush().await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::LOCAL_SOCKET_ADDR_STR_TEST;
    use tokio::io::AsyncReadExt;
    use tokio::net::{TcpListener, TcpStream};

    #[tokio::test]
    async fn ping_pong() {
        let listener = TcpListener::bind(LOCAL_SOCKET_ADDR_STR_TEST).await.unwrap();
        let addr = listener.local_addr().unwrap();

        let wrbuf = b"*1\r\n$4\r\nPING\r\n";
        let i = 8;
        let mut writer = TcpStream::connect(addr).await.unwrap();
        handle_ping(&mut writer, wrbuf, i).await.unwrap();

        const EXPECTED: &[u8; 7] = b"+PONG\r\n";
        let (mut reader, _addr) = listener.accept().await.unwrap();
        let mut rdbuf = [0u8; EXPECTED.len()];
        let n = reader.read(&mut rdbuf).await.unwrap();

        assert_eq!(EXPECTED.len(), n);
        assert_eq!(EXPECTED, &rdbuf);
    }
}
