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

use crate::constants::COMMANDS;
use crate::errors::CmdError;
use crate::is_enum_variant;
use crate::resp::{Message, Value};
use crate::storage::generic::Crud;
use crate::types::{ConcurrentStorageType, ExpirationTime, ExpirationTimeType};
use anyhow::Result;
use bytes::{BufMut, Bytes, BytesMut};
use std::time::{SystemTime, UNIX_EPOCH};
// use tokio::io::AsyncReadExt;

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
pub(crate) async fn handle_request<KV: Crud, KE: Crud>(
    storage: &ConcurrentStorageType<KV, KE>,
    bytes: &Bytes,
) -> Result<BytesMut, CmdError> {
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
    let _num_arr_elts = match bytes_arr_len {
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

    // Check in advance whether all array elements are bulk strings, because they all need to be.
    // Return early if at least one command is not a bulk string.
    if !request_arr
        .iter()
        .all(|cmd| is_enum_variant!(cmd, Value::BulkString))
    {
        return Err(CmdError::NotAllBulk);
    }

    let result = handle_words(storage, request_arr).await?;

    Ok(result)
}

/// Handles the request words and routes them to the appropriate functions.
///
/// Routes commands and their arguments to the appropriate command handlers.
async fn handle_words<KV: Crud, KE: Crud>(
    storage: &ConcurrentStorageType<KV, KE>,
    request_arr: &[Value],
) -> Result<BytesMut, CmdError> {
    // Clients send commands to a Redis server as an array of bulk strings.
    // The first (and sometimes also the second) bulk string in the array is the command's name.
    // Subsequent elements of the array are the arguments for the command.
    // For example: b"*2\r\n\$4\r\nECHO\r\n\$9\r\nraspberry\r\n" => b"$9\r\nraspberry\r\n"

    // But, commands can be pipelined, meaning that a single request can contain multiple commands.
    // num_flattened >= bytes_arr_len
    let num_flattened = request_arr.len();

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
            b"ECHO" => {
                if i < num_flattened - 1 {
                    result.put(handle_echo(&request_arr[i..i + 2]).await?);
                } else {
                    return Err(CmdError::MissingArg);
                }
            }
            b"GET" => {
                if i < num_flattened - 1 {
                    result.put(handle_get(&request_arr[i..i + 2], storage).await?);
                } else {
                    return Err(CmdError::MissingArg);
                }
            }
            b"PING" => {
                if i < num_flattened - 1 {
                    if let Value::BulkString(word) = &request_arr[i + 1] {
                        if is_cmd(word) {
                            result.put(handle_ping(&request_arr[i..i + 1]).await?);
                        } else {
                            result.put(handle_ping(&request_arr[i..i + 2]).await?);
                        }
                    }
                } else {
                    result.put(handle_ping(&request_arr[i..i + 1]).await?);
                }
            }
            b"SET" => {
                if num_flattened >= 4 && i < num_flattened - 4 {
                    result.put(handle_set(&request_arr[i..i + 5], storage).await?);
                } else if i < num_flattened - 2 {
                    result.put(handle_set(&request_arr[i..i + 3], storage).await?);
                } else {
                    return Err(CmdError::MissingArg);
                }
            }
            _ => {}
        }
        i += 1;
    }

    Ok(result)
}

/// Checks whether `word` is a Redis command.
///
/// `PING` makes use of this, as it can echo back the next received word, but that word can be a command.
///
/// `ECHO` doesn't use it, as it can echo back anything. It's up to the sender to compose input properly.
fn is_cmd(word: &[u8]) -> bool {
    let mut res = false;
    for cmd in COMMANDS {
        if word.eq_ignore_ascii_case(cmd) {
            res = true;
        }
    }
    res
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

/// Handler for the [GET](https://redis.io/docs/latest/commands/get/) command
///
/// Handles a single `GET` request.
///
/// `GET key` => `value`
///
/// Get the value of key. If the key does not exist the special value nil is returned.
/// An error is returned if the value stored at key is not a string, because GET only handles string values.
///
/// If the key exists, returns the value of the key as a
/// [bulk string](https://redis.io/docs/latest/develop/reference/protocol-spec/#bulk-strings).
///
/// If a key is passively expired, deletes it.
///
/// From the [EXPIRE](https://redis.io/docs/latest/commands/expire/#how-redis-expires-keys) docs:
/// "A key is passively expired simply when some client tries to access it, and the key is found to be timed out."
///
/// Examples:
/// - `"*2\r\n$3\r\nGET\r\n$6\r\norange\r\n"` => `$9\r\npineapple\r\n` - returns value `pineapple` for existing key `orange`
/// - `"*2\r\n$3\r\nGET\r\n$11\r\nnonexistent\r\n"` => `$-1\r\n` - returns `nil` value for nonexistent key `nonexistent`
async fn handle_get<KV: Crud, KE: Crud>(
    words: &[Value],
    storage: &ConcurrentStorageType<KV, KE>,
) -> Result<Bytes, CmdError> {
    if words.len() == 2 {
        let key_arg = if let Value::BulkString(arg) = &words[1] {
            arg
        } else {
            panic!("Expected GET argument and as bulk string");
        };
        let key = String::from_utf8(key_arg.to_vec())?;
        let mut should_delete = false;
        let response = {
            let s = storage.read().expect("RwLockReadGuard");
            match s.read(&key) {
                None => "$-1\r\n".to_string(),
                Some((value, expiry)) => match expiry {
                    None => format!("${}\r\n{value}\r\n", value.len()),
                    Some(expiry) => {
                        let time_now_ms = match SystemTime::now().duration_since(UNIX_EPOCH) {
                            Ok(since) => since,
                            Err(err) => return Err(CmdError::TimeError(err)),
                        }
                        .as_millis();
                        if time_now_ms > expiry {
                            should_delete = true;
                            "$-1\r\n".to_string()
                        } else {
                            format!("${}\r\n{value}\r\n", value.len())
                        }
                    }
                },
            }
        };
        if should_delete {
            let mut s = storage.write().expect("RwLockWriteGuard");
            s.delete(&key);
        }
        Ok(Bytes::from(response))
    } else {
        panic!("GET should consist of exactly two words");
    }
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

/// Handler for the [SET](https://redis.io/docs/latest/commands/set/) command
///
/// Handles a single `SET` request.
///
/// `SET key value [EX s]` => `+OK\r\n`
/// `SET key value [PX ms]` => `+OK\r\n`
///
/// Sets `key` to hold the string `value`. If key already holds a value, it is overwritten, regardless of its type.
/// Any previous time to live associated with the key is discarded on successful SET operation.
///
/// Supports setting expiry time (time-to-live) for the key with second precision, using the `PX` argument and value,
/// and with millisecond precision, using the `PX` argument and value.
///
/// Returns `OK` as a [simple string](https://redis.io/docs/latest/develop/reference/protocol-spec/#simple-strings),
/// in case of success.
///
/// Example:
/// - `"*3\r\n$3\r\nSET\r\n$6\r\norange\r\n$9\r\npineapple\r\n"` => `+OK\r\n` - sets key `orange` to value `pineapple`
/// - `"*5\r\n$3\r\nSET\r\n$6\r\nbanana\r\n$5\r\nmango\r\n$2\r\nEX\r\n$3\r\n10\r\n"` => `+OK\r\n` - sets key `banana`
///   to value `mango` with expiry time of 10 s
/// - `"*5\r\n$3\r\nSET\r\n$6\r\nbanana\r\n$5\r\nmango\r\n$2\r\nPX\r\n$3\r\n100\r\n"` => `+OK\r\n` - sets key `banana`
///   to value `mango` with expiry time of 100 ms
pub(crate) async fn handle_set<KV: Crud, KE: Crud>(
    words: &[Value],
    storage: &ConcurrentStorageType<KV, KE>,
) -> Result<Bytes, CmdError> {
    if words.len() >= 2 {
        let key_arg = if let Value::BulkString(arg) = &words[1] {
            arg
        } else {
            panic!("Expected SET key argument and as bulk string");
        };
        let value_arg = if let Value::BulkString(arg) = &words[2] {
            arg
        } else {
            panic!("Expected SET value argument and as bulk string");
        };
        let key = String::from_utf8(key_arg.to_vec())?;
        let value = String::from_utf8(value_arg.to_vec())?;

        let expiry: ExpirationTime = if words.len() == 5 {
            let time_cmd = if let Value::BulkString(arg) = &words[3] {
                arg
            } else {
                panic!("Expected SET time subcommand and as bulk string");
            };
            let time_val = if let Value::BulkString(arg) = &words[4] {
                arg
            } else {
                panic!("Expected SET time value and as bulk string");
            };
            let time_cmd = String::from_utf8(time_cmd.to_vec())?;
            let time_val = String::from_utf8(time_val.to_vec())?;
            // In case of "EX", the TTL is in seconds, but we'll just multiply by 1000 in that case to get milliseconds.
            let mut ttl_ms = time_val.parse::<ExpirationTimeType>()?;
            match time_cmd.to_ascii_uppercase().as_str() {
                "EX" => ttl_ms *= 1000,
                "PX" => {}
                tc => return Err(CmdError::WrongArg(tc.to_string())),
            }
            let time_now_ms = match SystemTime::now().duration_since(UNIX_EPOCH) {
                Ok(since) => since,
                Err(err) => return Err(CmdError::TimeError(err)),
            }
            .as_millis();
            Some(time_now_ms + ttl_ms)
        } else {
            None
        };

        let mut s = storage.write().expect("RwLockWriteGuard");
        (*s).create(&key, value, expiry);
        Ok(Bytes::from("+OK\r\n"))
    } else {
        panic!("SET should consist of at least three words");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Storage;
    use crate::types::{InMemoryExpiryTimeHashMap, InMemoryStorageHashMap, StorageType};
    use bytes::Bytes;
    use std::sync::RwLock;
    use std::sync::{Arc, OnceLock};
    use std::time::Duration;
    // use tokio::sync::RwLock;

    /// We only get one storage instance that is shared between all tests, which, by the way,
    /// run concurrently, so pay attention when naming keys!
    ///
    /// Keys should be different in all tests that involve `SET` and `GET`, whether they employ
    /// setting TTL or not, because TTL is implicitly set to infinity if not set explicitly,
    /// and this may affect other tests that expect shorter TTL, obviously.
    static STORAGE: OnceLock<
        ConcurrentStorageType<InMemoryStorageHashMap, InMemoryExpiryTimeHashMap>,
    > = OnceLock::new();

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
        let storage = STORAGE.get_or_init(|| {
            Arc::new(RwLock::new(Storage::<
                StorageType<InMemoryStorageHashMap, InMemoryExpiryTimeHashMap>,
                InMemoryStorageHashMap,
                InMemoryExpiryTimeHashMap,
            >::new()))
        });
        let input = "*1\r\n$4\r\nPING";
        let input = Bytes::from(input);
        let result = handle_request(storage, &input).await;

        if let Err(CmdError::CRLFNotAtEnd) = result {
        } else {
            assert_eq!(0, 1)
        };
    }

    #[tokio::test]
    async fn handle_request_ping_pong_pass() {
        let storage = STORAGE.get_or_init(|| {
            Arc::new(RwLock::new(Storage::<
                StorageType<InMemoryStorageHashMap, InMemoryExpiryTimeHashMap>,
                InMemoryStorageHashMap,
                InMemoryExpiryTimeHashMap,
            >::new()))
        });
        let input = "*1\r\n$4\r\nPING\r\n";
        let input = Bytes::from(input);
        let result = handle_request(storage, &input).await.unwrap();

        let expected = Bytes::from("+PONG\r\n");

        assert_eq!(expected, result);
    }

    #[tokio::test]
    async fn handle_request_ping_pong_fail_missing_array_len() {
        let storage = STORAGE.get_or_init(|| {
            Arc::new(RwLock::new(Storage::<
                StorageType<InMemoryStorageHashMap, InMemoryExpiryTimeHashMap>,
                InMemoryStorageHashMap,
                InMemoryExpiryTimeHashMap,
            >::new()))
        });
        let input = "$4\r\nPING\r\n";
        let input = Bytes::from(input);
        let result = handle_request(storage, &input).await;

        if let Err(CmdError::CmdNotArray) = result {
        } else {
            assert_eq!(0, 1)
        };
    }

    #[tokio::test]
    async fn handle_request_ping_ping_ping() {
        let storage = STORAGE.get_or_init(|| {
            Arc::new(RwLock::new(Storage::<
                StorageType<InMemoryStorageHashMap, InMemoryExpiryTimeHashMap>,
                InMemoryStorageHashMap,
                InMemoryExpiryTimeHashMap,
            >::new()))
        });
        let input = "*3\r\n$4\r\nPinG\r\n$4\r\nPinG\r\n$4\r\nPinG\r\n";
        let input = Bytes::from(input);
        let result = handle_request(storage, &input).await.unwrap();

        let expected = Bytes::from("+PONG\r\n+PONG\r\n+PONG\r\n");

        assert_eq!(expected, result);
    }

    #[tokio::test]
    async fn handle_request_ping_with_arg() {
        let storage = STORAGE.get_or_init(|| {
            Arc::new(RwLock::new(Storage::<
                StorageType<InMemoryStorageHashMap, InMemoryExpiryTimeHashMap>,
                InMemoryStorageHashMap,
                InMemoryExpiryTimeHashMap,
            >::new()))
        });
        let input = "*2\r\n$4\r\nPinG\r\n$13\r\nHello, world!\r\n";
        let input = Bytes::from(input);
        let result = handle_request(storage, &input).await.unwrap();

        let expected = Bytes::from("$13\r\nHello, world!\r\n");

        assert_eq!(expected, result);
    }

    #[tokio::test]
    async fn handle_request_echo_hey() {
        let storage = STORAGE.get_or_init(|| {
            Arc::new(RwLock::new(Storage::<
                StorageType<InMemoryStorageHashMap, InMemoryExpiryTimeHashMap>,
                InMemoryStorageHashMap,
                InMemoryExpiryTimeHashMap,
            >::new()))
        });
        let input = "*2\r\n$4\r\nECHO\r\n$3\r\nHey\r\n";
        let input = Bytes::from(input);
        let result = handle_request(storage, &input).await.unwrap();

        let expected = Bytes::from("$3\r\nHey\r\n");

        assert_eq!(expected, result);
    }

    #[tokio::test]
    async fn handle_request_echo_hey_hey() {
        let storage = STORAGE.get_or_init(|| {
            Arc::new(RwLock::new(Storage::<
                StorageType<InMemoryStorageHashMap, InMemoryExpiryTimeHashMap>,
                InMemoryStorageHashMap,
                InMemoryExpiryTimeHashMap,
            >::new()))
        });
        let input = "*4\r\n$4\r\nEchO\r\n$3\r\nHey\r\n$4\r\nEchO\r\n$3\r\nHey\r\n";
        let input = Bytes::from(input);
        let result = handle_request(storage, &input).await.unwrap();

        let expected = Bytes::from("$3\r\nHey\r\n$3\r\nHey\r\n");

        assert_eq!(expected, result);
    }

    #[tokio::test]
    async fn handle_request_ping_echo_ping_arg() {
        let storage = STORAGE.get_or_init(|| {
            Arc::new(RwLock::new(Storage::<
                StorageType<InMemoryStorageHashMap, InMemoryExpiryTimeHashMap>,
                InMemoryStorageHashMap,
                InMemoryExpiryTimeHashMap,
            >::new()))
        });
        let input = "*5\r\n$4\r\nPinG\r\n$4\r\nEchO\r\n$15\r\nHey, what's up?\r\n$4\r\nPinG\r\n$13\r\nHello, world!\r\n";
        let input = Bytes::from(input);
        let result = handle_request(storage, &input).await.unwrap();

        let expected = Bytes::from("+PONG\r\n$15\r\nHey, what's up?\r\n$13\r\nHello, world!\r\n");

        assert_eq!(expected, result);
    }

    #[tokio::test]
    async fn handle_request_ping_arg_echo_ping() {
        let storage = STORAGE.get_or_init(|| {
            Arc::new(RwLock::new(Storage::<
                StorageType<InMemoryStorageHashMap, InMemoryExpiryTimeHashMap>,
                InMemoryStorageHashMap,
                InMemoryExpiryTimeHashMap,
            >::new()))
        });
        let input = "*5\r\n$4\r\nPinG\r\n$13\r\nHello, world!\r\n$4\r\nEchO\r\n$15\r\nHey, what's up?\r\n$4\r\nPinG\r\n";
        let input = Bytes::from(input);
        let result = handle_request(storage, &input).await.unwrap();

        let expected = Bytes::from("$13\r\nHello, world!\r\n$15\r\nHey, what's up?\r\n+PONG\r\n");

        assert_eq!(expected, result);
    }

    #[tokio::test]
    async fn handle_request_set_01_get() {
        let storage = STORAGE.get_or_init(|| {
            Arc::new(RwLock::new(Storage::<
                StorageType<InMemoryStorageHashMap, InMemoryExpiryTimeHashMap>,
                InMemoryStorageHashMap,
                InMemoryExpiryTimeHashMap,
            >::new()))
        });

        let input = "*3\r\n$3\r\nSET\r\n$5\r\nKey01\r\n$7\r\nValue01\r\n";
        let input = Bytes::from(input);
        let result = handle_request(storage, &input).await.unwrap();
        let expected = Bytes::from("+OK\r\n");
        assert_eq!(expected, result);

        let input = "*2\r\n$3\r\nGET\r\n$5\r\nApple\r\n";
        let input = Bytes::from(input);
        let result = handle_request(storage, &input).await.unwrap();
        let expected = Bytes::from("$-1\r\n");
        assert_eq!(expected, result);

        let input = "*2\r\n$3\r\nGET\r\n$5\r\nKey01\r\n";
        let input = Bytes::from(input);
        let result = handle_request(storage, &input).await.unwrap();
        let expected = Bytes::from("$7\r\nValue01\r\n");
        assert_eq!(expected, result);
    }

    #[tokio::test]
    async fn handle_request_set_02_px_get_on_time() {
        let storage = STORAGE.get_or_init(|| {
            Arc::new(RwLock::new(Storage::<
                StorageType<InMemoryStorageHashMap, InMemoryExpiryTimeHashMap>,
                InMemoryStorageHashMap,
                InMemoryExpiryTimeHashMap,
            >::new()))
        });

        let input = "*5\r\n$3\r\nSET\r\n$5\r\nkey02\r\n$7\r\nvalue02\r\n$2\r\nPX\r\n$3\r\n100\r\n";
        let input = Bytes::from(input);
        let result = handle_request(storage, &input).await.unwrap();
        let expected = Bytes::from("+OK\r\n");
        assert_eq!(expected, result);

        tokio::time::sleep(Duration::from_millis(20)).await;

        let input = "*2\r\n$3\r\nGET\r\n$5\r\nkey02\r\n";
        let input = Bytes::from(input);
        let result = handle_request(storage, &input).await.unwrap();
        let expected = Bytes::from("$7\r\nvalue02\r\n");
        assert_eq!(expected, result);
    }

    #[tokio::test]
    async fn handle_request_set_03_px_get_expired() {
        let storage = STORAGE.get_or_init(|| {
            Arc::new(RwLock::new(Storage::<
                StorageType<InMemoryStorageHashMap, InMemoryExpiryTimeHashMap>,
                InMemoryStorageHashMap,
                InMemoryExpiryTimeHashMap,
            >::new()))
        });

        let input = "*5\r\n$3\r\nSET\r\n$5\r\nkey03\r\n$7\r\nvalue03\r\n$2\r\nPX\r\n$3\r\n100\r\n";
        let input = Bytes::from(input);
        let result = handle_request(storage, &input).await.unwrap();
        let expected = Bytes::from("+OK\r\n");
        assert_eq!(expected, result);

        tokio::time::sleep(Duration::from_millis(120)).await;

        let input = "*2\r\n$3\r\nGET\r\n$5\r\nkey03\r\n";
        let input = Bytes::from(input);
        let result = handle_request(storage, &input).await.unwrap();
        let expected = Bytes::from("$-1\r\n");
        assert_eq!(expected, result);
    }

    #[tokio::test]
    async fn handle_request_set_04_ex_get_on_time() {
        let storage = STORAGE.get_or_init(|| {
            Arc::new(RwLock::new(Storage::<
                StorageType<InMemoryStorageHashMap, InMemoryExpiryTimeHashMap>,
                InMemoryStorageHashMap,
                InMemoryExpiryTimeHashMap,
            >::new()))
        });

        let input = "*5\r\n$3\r\nSET\r\n$5\r\nkey04\r\n$7\r\nvalue04\r\n$2\r\nex\r\n$2\r\n10\r\n";
        let input = Bytes::from(input);
        let result = handle_request(storage, &input).await.unwrap();
        let expected = Bytes::from("+OK\r\n");
        assert_eq!(expected, result);

        tokio::time::sleep(Duration::from_secs(1)).await;

        let input = "*2\r\n$3\r\nGET\r\n$5\r\nkey04\r\n";
        let input = Bytes::from(input);
        let result = handle_request(storage, &input).await.unwrap();
        let expected = Bytes::from("$7\r\nvalue04\r\n");
        assert_eq!(expected, result);
    }

    #[tokio::test]
    async fn handle_request_set_05_ex_get_expired() {
        let storage = STORAGE.get_or_init(|| {
            Arc::new(RwLock::new(Storage::<
                StorageType<InMemoryStorageHashMap, InMemoryExpiryTimeHashMap>,
                InMemoryStorageHashMap,
                InMemoryExpiryTimeHashMap,
            >::new()))
        });

        let input = "*5\r\n$3\r\nSET\r\n$5\r\nkey05\r\n$7\r\nvalue05\r\n$2\r\nex\r\n$1\r\n1\r\n";
        let input = Bytes::from(input);
        let result = handle_request(storage, &input).await.unwrap();
        let expected = Bytes::from("+OK\r\n");
        assert_eq!(expected, result);

        tokio::time::sleep(Duration::from_millis(1200)).await;

        let input = "*2\r\n$3\r\nGET\r\n$5\r\nkey05\r\n";
        let input = Bytes::from(input);
        let result = handle_request(storage, &input).await.unwrap();
        let expected = Bytes::from("$-1\r\n");
        assert_eq!(expected, result);
    }

    #[tokio::test]
    async fn handle_request_set_06_set_set_px_get_on_time_then_expired() {
        let storage = STORAGE.get_or_init(|| {
            Arc::new(RwLock::new(Storage::<
                StorageType<InMemoryStorageHashMap, InMemoryExpiryTimeHashMap>,
                InMemoryStorageHashMap,
                InMemoryExpiryTimeHashMap,
            >::new()))
        });

        let input = "*3\r\n$3\r\nSET\r\n$5\r\nkey06\r\n$7\r\nvalue06\r\n";
        let input = Bytes::from(input);
        let result = handle_request(storage, &input).await.unwrap();
        let expected = Bytes::from("+OK\r\n");
        assert_eq!(expected, result);
        let input = "*2\r\n$3\r\nGET\r\n$5\r\nkey06\r\n";
        let input = Bytes::from(input);
        let result = handle_request(storage, &input).await.unwrap();
        let expected = Bytes::from("$7\r\nvalue06\r\n");
        assert_eq!(expected, result);

        let input = "*5\r\n$3\r\nSET\r\n$5\r\nkey06\r\n$7\r\nvalue06\r\n$2\r\npX\r\n$3\r\n100\r\n";
        let input = Bytes::from(input);
        let result = handle_request(storage, &input).await.unwrap();
        let expected = Bytes::from("+OK\r\n");
        assert_eq!(expected, result);
        tokio::time::sleep(Duration::from_millis(20)).await;
        let input = "*2\r\n$3\r\nGET\r\n$5\r\nkey06\r\n";
        let input = Bytes::from(input);
        let result = handle_request(storage, &input).await.unwrap();
        let expected = Bytes::from("$7\r\nvalue06\r\n");
        assert_eq!(expected, result);
        tokio::time::sleep(Duration::from_millis(120)).await;
        let input = "*2\r\n$3\r\nGET\r\n$5\r\nkey06\r\n";
        let input = Bytes::from(input);
        let result = handle_request(storage, &input).await.unwrap();
        let expected = Bytes::from("$-1\r\n");
        assert_eq!(expected, result);
    }

    #[tokio::test]
    async fn handle_request_set_07_set_px_set_get_on_time_should_not_expire() {
        let storage = STORAGE.get_or_init(|| {
            Arc::new(RwLock::new(Storage::<
                StorageType<InMemoryStorageHashMap, InMemoryExpiryTimeHashMap>,
                InMemoryStorageHashMap,
                InMemoryExpiryTimeHashMap,
            >::new()))
        });

        let input = "*5\r\n$3\r\nSET\r\n$5\r\nkey07\r\n$7\r\nvalue07\r\n$2\r\nPx\r\n$3\r\n100\r\n";
        let input = Bytes::from(input);
        let result = handle_request(storage, &input).await.unwrap();
        let expected = Bytes::from("+OK\r\n");
        assert_eq!(expected, result);

        tokio::time::sleep(Duration::from_millis(20)).await;

        let input = "*2\r\n$3\r\nGET\r\n$5\r\nkey07\r\n";
        let input = Bytes::from(input);
        let result = handle_request(storage, &input).await.unwrap();
        let expected = Bytes::from("$7\r\nvalue07\r\n");
        assert_eq!(expected, result);

        let input = "*3\r\n$3\r\nSET\r\n$5\r\nkey07\r\n$7\r\nvalue07\r\n";
        let input = Bytes::from(input);
        let result = handle_request(storage, &input).await.unwrap();
        let expected = Bytes::from("+OK\r\n");
        assert_eq!(expected, result);
        let input = "*2\r\n$3\r\nGET\r\n$5\r\nkey07\r\n";
        let input = Bytes::from(input);
        let result = handle_request(storage, &input).await.unwrap();
        let expected = Bytes::from("$7\r\nvalue07\r\n");
        assert_eq!(expected, result);

        tokio::time::sleep(Duration::from_millis(120)).await;

        let input = "*2\r\n$3\r\nGET\r\n$5\r\nkey07\r\n";
        let input = Bytes::from(input);
        let result = handle_request(storage, &input).await.unwrap();
        let expected = Bytes::from("$7\r\nvalue07\r\n");
        assert_eq!(expected, result);
    }

    #[tokio::test]
    async fn handle_request_set_08_set_px_set_px_get_on_time_twice_then_expired() {
        let storage = STORAGE.get_or_init(|| {
            Arc::new(RwLock::new(Storage::<
                StorageType<InMemoryStorageHashMap, InMemoryExpiryTimeHashMap>,
                InMemoryStorageHashMap,
                InMemoryExpiryTimeHashMap,
            >::new()))
        });

        let input = "*5\r\n$3\r\nSET\r\n$5\r\nkey08\r\n$7\r\nvalue08\r\n$2\r\nPX\r\n$3\r\n100\r\n";
        let input = Bytes::from(input);
        let result = handle_request(storage, &input).await.unwrap();
        let expected = Bytes::from("+OK\r\n");
        assert_eq!(expected, result);
        tokio::time::sleep(Duration::from_millis(20)).await;
        let input = "*2\r\n$3\r\nGET\r\n$5\r\nkey08\r\n";
        let input = Bytes::from(input);
        let result = handle_request(storage, &input).await.unwrap();
        let expected = Bytes::from("$7\r\nvalue08\r\n");
        assert_eq!(expected, result);

        let input = "*5\r\n$3\r\nSET\r\n$5\r\nkey08\r\n$7\r\nvalue08\r\n$2\r\nPX\r\n$3\r\n100\r\n";
        let input = Bytes::from(input);
        let result = handle_request(storage, &input).await.unwrap();
        let expected = Bytes::from("+OK\r\n");
        assert_eq!(expected, result);
        tokio::time::sleep(Duration::from_millis(20)).await;
        let input = "*2\r\n$3\r\nGET\r\n$5\r\nkey08\r\n";
        let input = Bytes::from(input);
        let result = handle_request(storage, &input).await.unwrap();
        let expected = Bytes::from("$7\r\nvalue08\r\n");
        assert_eq!(expected, result);

        tokio::time::sleep(Duration::from_millis(70)).await;
        let input = "*2\r\n$3\r\nGET\r\n$5\r\nkey08\r\n";
        let input = Bytes::from(input);
        let result = handle_request(storage, &input).await.unwrap();
        let expected = Bytes::from("$7\r\nvalue08\r\n");
        assert_eq!(expected, result);

        tokio::time::sleep(Duration::from_millis(20)).await;
        let input = "*2\r\n$3\r\nGET\r\n$5\r\nkey06\r\n";
        let input = Bytes::from(input);
        let result = handle_request(storage, &input).await.unwrap();
        let expected = Bytes::from("$-1\r\n");
        assert_eq!(expected, result);
    }
}
