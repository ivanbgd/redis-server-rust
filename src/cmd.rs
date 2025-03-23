//! # Request and Command Handlers
//!
//! [Commands](https://redis.io/docs/latest/commands/)
//!
//! [COMMAND](https://redis.io/docs/latest/commands/command/)
//! - Redis command names are case-insensitive.
//!
//! [Command key specifications](https://redis.io/docs/latest/develop/reference/key-specs/)
//!
//! [Redis serialization protocol specification](https://redis.io/docs/latest/develop/reference/protocol-spec/)
//! - A client sends a request to the Redis server as an array of strings. The array's contents are the command and
//!   its arguments that the server should execute. The server's reply type is command-specific.
//!
//! [Request-Response model](https://redis.io/docs/latest/develop/reference/protocol-spec/#request-response-model)
//! - The Redis server accepts commands composed of different arguments.
//!   Then, the server processes the command and sends the reply back to the client.
//!   This is the simplest model possible; however, there are some exceptions:
//!   - Redis requests can be pipelined.
//!     Pipelining enables clients to send multiple commands at once and wait for replies later.
//!
//! [RESP protocol description](https://redis.io/docs/latest/develop/reference/protocol-spec/#resp-protocol-description)
//! - RESP is essentially a serialization protocol that supports several data types.
//!   In RESP, the first byte of data determines its type.
//! - Redis generally uses RESP as a request-response protocol in the following way:
//!   - Clients send commands to a Redis server as an array of bulk strings.
//!     The first (and sometimes also the second) bulk string in the array is the command's name.
//!     Subsequent elements of the array are the arguments for the command.
//!   - The server replies with a RESP type.
//!     The reply's type is determined by the command's implementation and possibly by the client's protocol version.
//!
//! [Sending commands to a Redis server](https://redis.io/docs/latest/develop/reference/protocol-spec/#sending-commands-to-a-redis-server)
//! - We can further specify how the interaction between the client and the server works:
//!   - A client sends the Redis server an array consisting of only bulk strings.
//!   - A Redis server replies to clients, sending any valid RESP data type as a reply.
//!
//! [Multiple commands and pipelining](https://redis.io/docs/latest/develop/reference/protocol-spec/#multiple-commands-and-pipelining)
//! - A client can use the same connection to issue multiple commands.
//!   Pipelining is supported, so multiple commands can be sent with a single write operation by the client.
//!   The client can skip reading replies and continue to send the commands one after the other.
//!   All the replies can be read at the end.
//!   For more information, see [Pipelining](https://redis.io/docs/latest/develop/use/pipelining/).

use crate::errors::CmdError;
use crate::resp::{Message, Value};
use anyhow::Result;
use bytes::{BufMut, Bytes, BytesMut};

/// Compares against an enum variant without taking the value into account
macro_rules! is_enum_variant {
    ($val:ident, $var:path) => {
        match $val {
            $var(..) => true,
            _ => false,
        }
    };
}

/// Routes request bytes to the appropriate command handler(s) and returns the response bytes.
///
/// The received byte stream is necessarily a RESP request, and consequently the RESP Array type.
///
/// The byte stream buffer should not contain excess bytes, i.e., it is expected to end with the appropriate `CRLF`.
///
/// Since a single request is always an array, it can contain multiple commands. This is called
/// [pipelining](https://redis.io/docs/latest/develop/reference/protocol-spec/#multiple-commands-and-pipelining).
/// Pipelining enables clients to send multiple commands at once and wait for replies later.
///
/// In case of pipelining, the returned bytes contain multiple responses.
pub(crate) async fn handle_request(bytes: &Bytes) -> Result<BytesMut, CmdError> {
    // Do these checks here once per request, so that [`resp::deserialize`] doesn't have to do it multiple times,
    // and it would have to do it, as it depends on the byte stream ending in CRLF.
    let len = bytes.len();
    if len < 2 {
        return Err(CmdError::InputTooShort(String::from_utf8(bytes.to_vec())?));
    }
    if bytes[len - 2].ne(&b'\r') || bytes[len - 1].ne(&b'\n') {
        return Err(CmdError::CRLFNotAtEnd);
    }

    // Get the command-array's length and check against null arrays.
    let (bytes_arr_len, _) = Message::parse_len(bytes)?;
    let num_arr_elts = match bytes_arr_len {
        None => return Err(CmdError::NullArray),
        Some(n) => n,
    };

    // Parse (deserialize) the received byte stream into words (into a Message which holds the words).
    // A client sends a request to the Redis server as an array of strings.
    // The array's contents are the command and its arguments that the server should execute.
    let (msg, _bytes_read) = Message::deserialize(bytes)?;
    let request_arr = match &msg.data {
        Value::Array(array) => array,
        _ => return Err(CmdError::CmdNotArray),
    };
    if request_arr.is_empty() {
        return Err(CmdError::EmptyArray);
    }

    // But, commands can be pipelined, meaning that a single request can contain multiple commands.
    // num_flattened >= bytes_arr_len
    let num_flattened = request_arr.len();

    // Check in advance whether all array elements are bulk strings, because they all need to be.
    // Return early if at least one command is not a bulk string.
    if !request_arr
        .iter()
        .all(|cmd| is_enum_variant!(cmd, Value::BulkString))
    {
        return Err(CmdError::NotAllBulk);
    }

    // Clients send commands to a Redis server as an array of bulk strings.
    // The first (and sometimes also the second) bulk string in the array is the command's name.
    // Subsequent elements of the array are the arguments for the command.
    // For example: b"*2\r\n\$4\r\nECHO\r\n\$9\r\nraspberry\r\n" => b"$9\r\nraspberry\r\n"

    let mut result = BytesMut::new();

    let mut i = 0usize;
    while i < num_flattened {
        let first_word = &request_arr[i];
        let first = if let Value::BulkString(bytes) = first_word {
            bytes
        } else {
            panic!("Expected bulk string")
        };
        match first.to_ascii_uppercase().as_slice() {
            b"PING" => {
                if i == num_flattened - 1 {
                    result.put(handle_ping(&request_arr[i..i + 1]).await?)
                } else {
                    result.put(handle_ping(&request_arr[i..i + 2]).await?)
                }
            }
            b"ECHO" => {
                if i < num_flattened - 1 {
                    result.put(handle_echo(&request_arr[i..i + 2]).await?)
                }
            }
            _ => {}
        }
        i += 1;
    }

    Ok(result)
}

/// Handler for the [PING](https://redis.io/docs/latest/commands/ping/) command
///
/// Handles a single `PING` request.
///
/// Returns `PONG` as a [simple string](https://redis.io/docs/latest/develop/reference/protocol-spec/#simple-strings)
/// if no argument is provided, otherwise returns a copy of the argument as a
/// [bulk string](https://redis.io/docs/latest/develop/reference/protocol-spec/#bulk-strings).
///
/// - Example request from a client: `"*1\r\n$4\r\nPING\r\n"`
///   That's `["PING"]` encoded using the Redis protocol.
///    - Expected response from the server: `+PONG\r\n` (a simple string)
/// - Example request from a client: `"*2\r\n$4\r\PING\r\n$8\r\nTest a B\r\n"`
///   That's `["PING", "Test a B"]` encoded using the Redis protocol.
///    - Expected response from the server: `$8\r\nTest a B\r\n` (a bulk string)
async fn handle_ping(words: &[Value]) -> Result<Bytes, CmdError> {
    if words.len() == 1 {
        Ok(Bytes::from("+PONG\r\n"))
    } else if words.len() == 2 {
        let argument = if let Value::BulkString(arg) = &words[1] {
            arg
        } else {
            panic!("Expected PING argument and as bulk string");
        };
        let argument = String::from_utf8(argument.to_vec())?;
        let response = format!("${}\r\n{argument}\r\n", argument.len());
        Ok(Bytes::from(response))
    } else {
        panic!("PING can't consist of more than two words");
    }
}

/// Handler for the [ECHO](https://redis.io/docs/latest/commands/echo/) command
///
/// Handles a single `ECHO` request.
///
/// Returns a copy of the argument as a
/// [bulk string](https://redis.io/docs/latest/develop/reference/protocol-spec/#bulk-strings).
///
/// - Example request from a client: `"*2\r\n$4\r\nECHO\r\n$3\r\nHey\r\n"`.
///   That's `["ECHO", "Hey"]` encoded using the Redis protocol.
///   - Expected response from the server: `$3\r\nHey\r\n` (a bulk string)
async fn handle_echo(words: &[Value]) -> Result<Bytes, CmdError> {
    if words.len() == 2 {
        let argument = if let Value::BulkString(arg) = &words[1] {
            arg
        } else {
            panic!("Expected ECHO argument and as bulk string");
        };
        let argument = String::from_utf8(argument.to_vec())?;
        let response = format!("${}\r\n{argument}\r\n", argument.len());
        Ok(Bytes::from(response))
    } else {
        panic!("ECHO should consist of exactly two words");
    }
}

/// Handler for the [SET](https://redis.io/docs/latest/commands/set/) command
///
/// Handles a single `SET` request.
///
/// Set key to hold the string value. If key already holds a value, it is overwritten, regardless of its type.
/// Any previous time to live associated with the key is discarded on successful SET operation.
pub(crate) async fn handle_set() {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;

    #[tokio::test]
    async fn handle_ping_ping_pong() {
        let input = "$4\r\nPING\r\n";
        let input = Value::BulkString(Bytes::from(input));
        let input = vec![input];
        let result = handle_ping(&input).await.unwrap();

        let expected = Bytes::from("+PONG\r\n");

        assert_eq!(expected, result);
    }

    #[tokio::test]
    async fn handle_ping_ping_with_arg() {
        let cmd = Value::BulkString(Bytes::from("PING"));
        let arg = Value::BulkString(Bytes::from("Hello, world!"));
        let words = vec![cmd, arg];
        let result = handle_ping(&words).await.unwrap();

        let expected = Bytes::from("$13\r\nHello, world!\r\n");

        assert_eq!(expected, result);
    }

    #[tokio::test]
    async fn handle_request_ping_pong_missing_crlf_at_end() {
        let input = "*1\r\n$4\r\nPING";
        let input = Bytes::from(input);
        let result = handle_request(&input).await;

        if let Err(CmdError::CRLFNotAtEnd) = result {
        } else {
            assert_eq!(0, 1)
        };
    }

    #[tokio::test]
    async fn handle_request_ping_pong_pass() {
        let input = "*1\r\n$4\r\nPING\r\n";
        let input = Bytes::from(input);
        let result = handle_request(&input).await.unwrap();

        let expected = Bytes::from("+PONG\r\n");

        assert_eq!(expected, result);
    }

    #[tokio::test]
    async fn handle_request_ping_pong_fail_missing_array_len() {
        let input = "$4\r\nPING\r\n";
        let input = Bytes::from(input);
        let result = handle_request(&input).await;

        if let Err(CmdError::CmdNotArray) = result {
        } else {
            assert_eq!(0, 1)
        };
    }

    #[tokio::test]
    async fn handle_request_ping_with_arg() {
        let input = "*2\r\n$4\r\nPING\r\n$13\r\nHello, world!\r\n";
        let input = Bytes::from(input);
        let result = handle_request(&input).await.unwrap();

        let expected = Bytes::from("$13\r\nHello, world!\r\n");

        assert_eq!(expected, result);
    }

    #[tokio::test]
    async fn handle_request_echo_hey() {
        let input = "*2\r\n$4\r\nECHO\r\n$3\r\nHey\r\n";
        let input = Bytes::from(input);
        let result = handle_request(&input).await.unwrap();

        let expected = Bytes::from("$3\r\nHey\r\n");

        assert_eq!(expected, result);
    }

    #[tokio::test]
    async fn handle_request_echo_hey_hey() {
        let input = "*4\r\n$4\r\nECHO\r\n$3\r\nHey\r\n$4\r\nECHO\r\n$3\r\nHey\r\n";
        let input = Bytes::from(input);
        let result = handle_request(&input).await.unwrap();

        let expected = Bytes::from("$3\r\nHey\r\n$3\r\nHey\r\n");

        assert_eq!(expected, result);
    }

    #[tokio::test]
    async fn handle_request_ping_echo_ping() {
        let input = "*5\r\n$4\r\nPING\r\n$13\r\nHello, world!\r\n$4\r\nECHO\r\n$15\r\nHey, what's up?\r\n$4\r\nPING\r\n";
        let input = Bytes::from(input);
        let result = handle_request(&input).await.unwrap();

        let expected = Bytes::from("$13\r\nHello, world!\r\n$15\r\nHey, what's up?\r\n+PONG\r\n");

        assert_eq!(expected, result);
    }
}
