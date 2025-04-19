//! # RESP: Redis Serialization Protocol
//!
//! RESP can serialize different data types including integers, strings, and arrays.
//! It also features an error-specific type. A client sends a request to the Redis server as an array of strings.
//! The array's contents are the command and its arguments that the server should execute.
//! The server's reply type is command-specific.
//!
//! RESP is binary-safe and uses prefixed length to transfer bulk data so it does not require processing
//! bulk data transferred from one process to another.
//!
//! The Redis server accepts commands composed of different arguments.
//! Then, the server processes the command and sends the reply back to the client.
//! Redis requests can be pipelined.
//! Pipelining enables clients to send multiple commands at once and wait for replies later.
//!
//! RESP is essentially a serialization protocol that supports several data types.
//! In RESP, the first byte of data determines its type.
//!
//! Redis generally uses RESP as a request-response protocol in the following way:
//! - Clients send commands to a Redis server as an array of bulk strings.
//!   The first (and sometimes also the second) bulk string in the array is the command's name.
//!   Subsequent elements of the array are the arguments for the command.
//! - The server replies with a RESP type.
//!   The reply's type is determined by the command's implementation and possibly by the client's protocol version.
//!
//! RESP is a binary protocol that uses control sequences encoded in standard ASCII.
//! The A character, for example, is encoded with the binary byte of value 65.
//! Similarly, the characters CR (\r), LF (\n) and SP ( ) have binary byte values of 13, 10 and 32, respectively.
//!
//! The \r\n (CRLF) is the protocol's terminator, which **always** separates its parts.
//!
//! The first byte in an RESP-serialized payload always identifies its type.
//! Subsequent bytes constitute the type's contents.
//!
//! We categorize every RESP data type as either simple, bulk or aggregate.
//!
//! Simple types are similar to scalars in programming languages that represent plain literal values.
//! Booleans and Integers are such examples.
//!
//! RESP strings are either simple or bulk. Simple strings never contain carriage return (\r) or line feed (\n)
//! characters. Bulk strings can contain any binary data and may also be referred to as binary or blob. Note that
//! bulk strings may be further encoded and decoded, e.g. with a wide multibyte encoding, by the client.
//!
//! Aggregates, such as Arrays and Maps, can have varying numbers of sub-elements and nesting levels.
//!
//! [Official documentation](https://redis.io/docs/latest/develop/reference/protocol-spec/)

use crate::errors::RESPError;
use anyhow::Result;
use bytes::Bytes;
use memchr::memmem;
use std::fmt::{Display, Formatter};
use std::ops::Neg;

/// A RESP message
///
/// Consists of:
/// - RESP type
/// - data
#[derive(Debug, PartialEq)]
pub(crate) struct Message {
    pub(crate) resp_type: RESPType,
    pub(crate) data: Value,
}

impl Message {
    /// Deserializes (parses) a received byte stream into a [`Message`].
    ///
    /// The byte stream buffer should not contain excess bytes, i.e., it is expected to end with the appropriate `CRLF`.
    ///
    /// This is an associated function that can be used to create a new instance of a [`Message`].
    ///
    /// Returns a tuple of ([`Message`], the length of the complete raw value in bytes).
    pub(crate) fn deserialize(bytes: &Bytes) -> Result<(Message, usize), RESPError> {
        let resp_type = bytes[0].try_into()?;
        let (value, length) = match resp_type {
            RESPType::SimpleString => Self::deserialize_simple_string(bytes)?,
            RESPType::BulkString => Self::deserialize_bulk_string(bytes)?,
            RESPType::Integer => Self::deserialize_integer(bytes)?,
            RESPType::Array => Self::deserialize_array(bytes)?,
            RESPType::Error => Self::deserialize_error(bytes)?,
            #[allow(unreachable_patterns)]
            t => return Err(RESPError::UnsupportedRESPType(u8::from(t))),
        };

        Ok((
            Self {
                resp_type,
                data: value,
            },
            length,
        ))
    }

    /// Gets the length of aggregate types (for example, arrays or bulk strings).
    ///
    /// Returns a tuple of the parsed length and the number of bytes read to extract the length.
    ///
    /// The length is either None or some non-negative integer.
    ///
    /// The number of bytes include the RESP type and the two bytes for `CRLF`.
    ///
    /// In case `-1` is received as length, returns None as length, and 5 as bytes read:
    /// one byte for type plus four bytes for: `b"-1\r\n"`:
    /// - `b"$-1\r\n"` => `(None, 5)`
    /// - `b"*-1\r\n"` => `(None, 5)`
    ///
    /// Length of `-1` carries a special meaning in RESP, to represent null bulk strings or arrays.
    ///
    /// Some successful examples:
    /// - `b"*0\r\n"` => `(Some(0), 4)`
    /// - `b"$123456\r\n"` => `(Some(123456), 9)`
    ///
    /// Reading the length of aggregate types (for example, bulk strings or arrays) can be processed with code that
    /// performs a single operation per character while at the same time scanning for the CR character.
    ///
    /// https://redis.io/docs/latest/develop/reference/protocol-spec/#high-performance-parser-for-the-redis-protocol
    ///
    /// # Errors
    /// - Returns an error in case `LF` is missing after `CR`, but `CR` is assumed to be present, as it's required
    ///   by the algorithm which is designed to be fast, so it doesn't check for it beforehand.
    /// - Length can't be negative, so returns an error if that's the case.
    /// - Characters composing length must be decimal digits `0` through `9`; returns an error if that isn't the case.
    pub(crate) fn parse_len(bytes: &[u8]) -> Result<(Option<usize>, usize), RESPError> {
        let mut ptr: *const u8 = bytes.as_ptr();
        let mut bytes_read: usize = 0;
        let mut len: usize = 0;

        unsafe {
            // Skip the first byte which denotes a RESP type.
            ptr = ptr.add(1);
            bytes_read += 1;

            if (*ptr).eq(&b'-') {
                if (*ptr.add(1)).eq(&b'1') {
                    if (*ptr.add(2)).eq(&b'\r') {
                        if (*ptr.add(3)).eq(&b'\n') {
                            bytes_read += 4;
                            return Ok((None, bytes_read));
                        } else {
                            return Err(RESPError::LFMissing);
                        }
                    } else {
                        return Err(RESPError::NegativeLength);
                    }
                } else {
                    return Err(RESPError::NegativeLength);
                }
            }

            while (*ptr).ne(&b'\r') {
                bytes_read += 1;
                if *ptr < b'0' || *ptr - b'0' > 9 {
                    return Err(RESPError::IntegerParseError(String::from_utf8(
                        bytes[1..bytes_read].to_vec(),
                    )?));
                }
                len = (len * 10) + (*ptr - b'0') as usize;
                ptr = ptr.add(1);
            }

            ptr = ptr.add(1);
            if (*ptr).ne(&b'\n') {
                return Err(RESPError::LFMissing);
            }
        }

        // `CRLF` account for the `+2`:
        Ok((Some(len), bytes_read + 2))
    }

    /// Returns a tuple of deserialized simple string contents and length of the complete raw simple string in bytes.
    ///
    /// The string contents are returned as `Value::SimpleString(contents)`, where contents are [`Bytes`].
    ///
    /// Example:
    /// - `+OK\r\n` => `("OK", 5)`
    fn deserialize_simple_string(bytes: &Bytes) -> Result<(Value, usize), RESPError> {
        // Simple strings can't contain CR or LF unless in the terminator and in that order.
        // But, we are providing `Bytes` and work on slices of it in entire library, so `bytes`
        // may contain multiple CRLF.
        let mut cr_it = memmem::find_iter(bytes, b"\r");
        let cr_pos = match cr_it.next() {
            Some(pos) => pos,
            None => return Err(RESPError::CRMissing),
        };
        let mut lf_it = memmem::find_iter(bytes, b"\n");
        let lf_pos = match lf_it.next() {
            Some(pos) => pos,
            None => return Err(RESPError::LFMissing),
        };
        if lf_pos != cr_pos + 1 {
            return Err(RESPError::CRLFNotAtEnd);
        }

        Ok((Value::SimpleString(bytes.slice(1..cr_pos)), lf_pos + 1))
    }

    /// Returns a tuple of deserialized bulk string contents and the length of the complete raw bulk string in bytes.
    ///
    /// The string contents are returned as `Value::BulkString(contents)`, where contents are [`Bytes`].
    ///
    /// In case `-1` is received as length, returns `(Value::NullBulkString, 5)`.
    ///
    /// Examples:
    /// - `$5\r\nhello\r\n` => `("hello", 11)`
    /// - The empty string's encoding is: `$0\r\n\r\n` => `("", 6)`
    /// - A Null Bulk String: `$-1\r\n` => `(Value::NullBulkString, 5)`
    fn deserialize_bulk_string(bytes: &Bytes) -> Result<(Value, usize), RESPError> {
        // Bulk strings can contain CR or LF or CRLF.
        let (Some(len), start) = Self::parse_len(bytes)? else {
            return Ok((Value::NullBulkString, 5));
        };
        let end = start + len;
        Ok((Value::BulkString(bytes.slice(start..end)), end + 2))
    }

    /// Returns a tuple of deserialized integer contents and the length of the complete raw integer in bytes.
    ///
    /// The integer contents are returned as `Value::Integer(contents)`, where contents are [`i64`].
    ///
    /// Examples:
    /// - `:0\r\n` => `(0, 4)`
    /// - `:1000\r\n` => `(1000, 7)`
    /// - `:+1000\r\n` => `(1000, 8)`
    /// - `:-1000\r\n` => `(-1000, 8)`
    fn deserialize_integer(bytes: &Bytes) -> Result<(Value, usize), RESPError> {
        // We use Self::parse_len(). It skips the first byte, considering it a RESP type.
        if bytes[1].eq(&b'+') {
            let (value, bytes_read) = Self::parse_len(&bytes.slice(1..))?;
            let value = value.expect("Expected some length; got None (-1).");
            Ok((Value::Integer(value as i64), 1 + bytes_read))
        } else if bytes[1].eq(&b'-') {
            let (value, bytes_read) = Self::parse_len(&bytes.slice(1..))?;
            let value = value.expect("Expected some length; got None (-1).");
            Ok((Value::Integer((value as i64).neg()), 1 + bytes_read))
        } else {
            let (value, bytes_read) = Self::parse_len(bytes)?;
            let value = value.expect("Expected some length; got None (-1).");
            Ok((Value::Integer(value as i64), bytes_read))
        }
    }

    /// Returns a tuple of deserialized array contents and the length of the complete raw array in bytes.
    ///
    /// The array contents are returned as `Value::Array(contents)`, where contents are [`Vec<Value>`].
    ///
    /// In case `-1` is received as length, returns `(Value::NullArray, 5)`.
    ///
    /// Examples:
    /// - `[]`: An empty array: `*0\r\n` => `([], 4)`
    /// - Array of one command: `"*1\r\n$4\r\nPING\r\n"` => `(["PING"], 14)`
    /// - Encoding of an array consisting of the two bulk strings "hello" and "world":
    ///   `*2\r\n$5\r\nhello\r\n$5\r\nworld\r\n`
    /// - An Array of three integers is encoded as follows:
    ///   `*3\r\n:1\r\n:2\r\n:3\r\n`
    /// - Mixed data types:
    ///   `*5\r\n:1\r\n:2\r\n:3\r\n:4\r\n$5\r\nhello\r\n` => `[1, 2, 3, 4, "hello"]`
    ///   The first line the server sent is `*5\r\n`.
    ///   This numeric value tells the client that five reply types are about to follow it.
    ///   Then, every successive reply constitutes an element in the array.
    /// - A nested array of two arrays:
    ///   `*2\r\n*3\r\n:1\r\n:2\r\n:3\r\n*2\r\n+Hello\r\n-World\r\n` => `[[1, 2, 3], ["Hello", ERR:"World"]]`
    ///   This encodes a two-element array. The first element is an array that, in turn, contains three integers
    ///   (1, 2, 3). The second element is another array containing a simple string and an error.
    /// - `None`: A Null Array: `*-1\r\n` => `(Value::NullArray, 5)`
    /// - An array with null elements:
    ///   `*3\r\n$5\r\nhello\r\n$-1\r\n$5\r\nworld\r\n` => `["hello", None, "world"]`
    fn deserialize_array(bytes: &Bytes) -> Result<(Value, usize), RESPError> {
        let (Some(num_elts), mut offset) = Self::parse_len(bytes)? else {
            return Ok((Value::NullArray, 5));
        };
        let mut result = Vec::with_capacity(num_elts);
        for _ in 0..num_elts {
            let (msg, bytes_read) = Message::deserialize(&bytes.slice(offset..))?;
            let value = msg.data;
            result.push(value);
            offset += bytes_read;
        }
        Ok((Value::Array(result), offset))
    }

    /// Returns a tuple of deserialized error string contents and length of the complete raw error string in bytes.
    ///
    /// Errors are similar to simple strings, but their first character is the minus (-) character.
    ///
    /// The error string contents are returned as `Value::Error(contents)`, where contents are [`Bytes`].
    ///
    /// Example: `-Error message\r\n` => `Error message`
    fn deserialize_error(bytes: &Bytes) -> Result<(Value, usize), RESPError> {
        let (Value::SimpleString(value), bytes_read) = Self::deserialize_simple_string(bytes)?
        else {
            panic!("Expected a simple string");
        };
        Ok((Value::Error(value), bytes_read))
    }
}

/// Denotes a RESP type of a Redis message.
///
/// Redis serialization protocol (RESP) is the wire protocol that clients implement.
///
/// [ Redis serialization protocol specification](https://redis.io/docs/latest/develop/reference/protocol-spec/)
#[derive(Debug, PartialEq)]
#[repr(u8)]
#[non_exhaustive]
pub(crate) enum RESPType {
    /// Simple strings are encoded as a plus (+) character, followed by a string.
    /// The string mustn't contain a `CR` (`\r`) or `LF` (`\n`) character and is terminated by `CRLF` (i.e., `\r\n`).
    ///
    /// Simple strings transmit short, non-binary strings with minimal overhead.
    ///
    /// Simple strings never contain carriage return (`\r`) or line feed (`\n`) characters.
    ///
    /// When Redis replies with a simple string, a client library should return to the caller a string value composed
    /// of the first character after the `+` up to the end of the string, excluding the final `CRLF` bytes.
    ///
    /// Example:
    /// - `"OK"`: `+OK\r\n`.
    SimpleString = b'+',

    /// A bulk string represents a single binary string.
    /// The string can be of any size, but by default, Redis limits it to 512 MB.
    ///
    /// Bulk strings can contain any binary data and may also be referred to as binary or blob.
    /// Note that bulk strings may be further encoded and decoded, e.g. with a wide multibyte encoding, by the client.
    ///
    /// RESP encodes bulk strings in the following way:
    ///
    /// `$<length>\r\n<data>\r\n`
    ///
    /// The null bulk string represents a non-existing value.
    /// The `GET` command returns the Null Bulk String when the target key doesn't exist.
    /// It is encoded as a bulk string with the length of negative one (-1).
    /// A Redis client should return a nil object when the server replies with a null bulk string
    /// rather than the empty string.
    ///
    /// Examples:
    /// - `"hello"`: `$5\r\nhello\r\n`
    /// - `""`: The empty string's encoding is: `$0\r\n\r\n`
    /// - `None`: A Null Bulk String: `$-1\r\n`
    BulkString = b'$',

    /// This type is a `CRLF`-terminated string that represents a signed, base-10, 64-bit integer.
    ///
    /// RESP encodes integers in the following way:
    ///
    /// `:[<+|->]<value>\r\n`
    ///
    /// Many Redis commands return RESP integers, including `INCR`, `LLEN`, and `LASTSAVE`.
    /// An integer, by itself, has no special meaning other than in the context of the command that returned it.
    ///
    /// Examples:
    /// - `:0\r\n` <=> `0`
    /// - `:1000\r\n` <=> `1000`
    Integer = b':',

    /// Clients send commands to the Redis server as RESP arrays.
    /// Similarly, some Redis commands that return collections of elements use arrays as their replies.
    /// An example is the `LRANGE` command that returns elements of a list.
    ///
    /// RESP Arrays' encoding uses the following format:
    ///
    /// `*<number-of-elements>\r\n<element-1>...<element-n>`
    ///
    /// Array elements are RESP types.
    ///
    /// Arrays can contain mixed data types.
    ///
    /// All of the aggregate RESP types support nesting.
    ///
    /// Null arrays exist as an alternative way of representing a null value.
    /// For instance, when the `BLPOP` command times out, it returns a null array.
    /// The encoding of a null array is that of an array with the length of -1, i.e.: `*-1\r\n`.
    /// When Redis replies with a null array, the client should return a null object rather than an empty array.
    /// This is necessary to distinguish between an empty list and a different condition
    /// (for instance, the timeout condition of the `BLPOP` command).
    ///
    /// Null elements in arrays: Single elements of an array may be null bulk string.
    /// This is used in Redis replies to signal that these elements are missing and not empty strings.
    ///
    /// Examples:
    /// - `[]`: An empty array: `*0\r\n`
    /// - Encoding of an array consisting of the two bulk strings "hello" and "world":
    ///   `*2\r\n$5\r\nhello\r\n$5\r\nworld\r\n`
    /// - An Array of three integers is encoded as follows:
    ///   `*3\r\n:1\r\n:2\r\n:3\r\n`
    /// - Mixed data types:
    ///   `*5\r\n:1\r\n:2\r\n:3\r\n:4\r\n$5\r\nhello\r\n` <=> `[1, 2, 3, 4, "hello"]`
    ///   The first line the server sent is `*5\r\n`.
    ///   This numeric value tells the client that five reply types are about to follow it.
    ///   Then, every successive reply constitutes an element in the array.
    /// - A nested array of two arrays:
    ///   `*2\r\n*3\r\n:1\r\n:2\r\n:3\r\n*2\r\n+Hello\r\n-World\r\n` <=> `[[1, 2, 3], ["Hello", ERR:"World"]]`
    ///   This encodes a two-element array. The first element is an array that, in turn, contains three integers
    ///   (1, 2, 3). The second element is another array containing a simple string and an error.
    /// - `None`: A null array: `*-1\r\n`
    /// - An array with null elements:
    ///   `*3\r\n$5\r\nhello\r\n$-1\r\n$5\r\nworld\r\n` <=> `["hello", None, "world"]`
    Array = b'*',

    /// The string encoded in the error type is the error message itself.
    ///
    /// Errors are similar to simple strings, but their first character is the minus (-) character.
    ///
    /// Example: `-Error message\r\n` <=> `Error message`
    ///
    /// The client should raise an exception when it receives an Error reply.
    Error = b'-',
}

/// In case we'd like to print [`RESPType`] as raw byte, i.e., as [`u8`].
///
/// Use [`Debug`] for human-readable output, which we derived for this enum.
impl Display for RESPType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self as *const Self as u8)
    }
}

impl From<RESPType> for u8 {
    fn from(value: RESPType) -> u8 {
        value as u8
    }
}

impl TryFrom<u8> for RESPType {
    type Error = RESPError;

    fn try_from(value: u8) -> Result<RESPType, RESPError> {
        match value {
            b'+' => Ok(RESPType::SimpleString),
            b'$' => Ok(RESPType::BulkString),
            b':' => Ok(RESPType::Integer),
            b'*' => Ok(RESPType::Array),
            b'-' => Ok(RESPType::Error),
            v => Err(RESPError::UnsupportedRESPType(v)),
        }
    }
}

/// Represents a raw value of a Redis message.
///
/// By raw value is meant a [`Bytes`] or an `i64` or a [`Vec`] of such value, which means that the RESP type,
/// length and `CRLF` are **excluded**.
///
/// Redis serialization protocol (RESP) is the wire protocol that clients implement.
///
/// [ Redis serialization protocol specification](https://redis.io/docs/latest/develop/reference/protocol-spec/)
#[derive(Debug, PartialEq)]
#[non_exhaustive]
pub(crate) enum Value {
    /// Simple strings are encoded as a plus (+) character, followed by a string.
    /// The string mustn't contain a `CR` (`\r`) or `LF` (`\n`) character and is terminated by `CRLF` (i.e., `\r\n`).
    ///
    /// Simple strings transmit short, non-binary strings with minimal overhead.
    ///
    /// Simple strings never contain carriage return (`\r`) or line feed (`\n`) characters.
    ///
    /// When Redis replies with a simple string, a client library should return to the caller a string value composed
    /// of the first character after the `+` up to the end of the string, excluding the final `CRLF` bytes.
    ///
    /// Example:
    /// - `"OK"`: `+OK\r\n`; We only keep the `b"OK"` portion, i.e., the raw contents.
    SimpleString(Bytes),

    /// A bulk string represents a single binary string.
    /// The string can be of any size, but by default, Redis limits it to 512 MB.
    ///
    /// Bulk strings can contain any binary data and may also be referred to as binary or blob.
    /// Note that bulk strings may be further encoded and decoded, e.g. with a wide multibyte encoding, by the client.
    ///
    /// RESP encodes bulk strings in the following way:
    ///
    /// `$<length>\r\n<data>\r\n`
    ///
    /// The null bulk string represents a non-existing value.
    /// The `GET` command returns the Null Bulk String when the target key doesn't exist.
    /// It is encoded as a bulk string with the length of negative one (-1).
    /// A Redis client should return a nil object when the server replies with a null bulk string
    /// rather than the empty string.
    ///
    /// Examples:
    /// - `"hello"`: `$5\r\nhello\r\n`; We only keep the `b"hello"` portion, i.e., the raw contents.
    /// - `""`: The empty string's encoding is: `$0\r\n\r\n`
    /// - `None`: A Null Bulk String: `$-1\r\n`
    BulkString(Bytes),
    NullBulkString,

    /// This type is a `CRLF`-terminated string that represents a signed, base-10, 64-bit integer.
    ///
    /// RESP encodes integers in the following way:
    ///
    /// `:[<+|->]<value>\r\n`
    ///
    /// Many Redis commands return RESP integers, including `INCR`, `LLEN`, and `LASTSAVE`.
    /// An integer, by itself, has no special meaning other than in the context of the command that returned it.
    ///
    /// Examples:
    /// - `:0\r\n` <=> `0`; We only keep the `0` portion, i.e., the raw contents.
    /// - `:1000\r\n` <=> `1000`
    Integer(i64),

    /// Clients send commands to the Redis server as RESP arrays.
    /// Similarly, some Redis commands that return collections of elements use arrays as their replies.
    /// An example is the `LRANGE` command that returns elements of a list.
    ///
    /// RESP Arrays' encoding uses the following format:
    ///
    /// `*<number-of-elements>\r\n<element-1>...<element-n>`
    ///
    /// Array elements are RESP types.
    ///
    /// Arrays can contain mixed data types.
    ///
    /// All of the aggregate RESP types support nesting.
    ///
    /// Null arrays exist as an alternative way of representing a null value.
    /// For instance, when the `BLPOP` command times out, it returns a null array.
    /// The encoding of a null array is that of an array with the length of -1, i.e.: `*-1\r\n`.
    /// When Redis replies with a null array, the client should return a null object rather than an empty array.
    /// This is necessary to distinguish between an empty list and a different condition
    /// (for instance, the timeout condition of the `BLPOP` command).
    ///
    /// Null elements in arrays: Single elements of an array may be null bulk string.
    /// This is used in Redis replies to signal that these elements are missing and not empty strings.
    ///
    /// Examples:
    /// - `[]`: An empty array: `*0\r\n`
    /// - Encoding of an array consisting of the two bulk strings "hello" and "world":
    ///   `*2\r\n$5\r\nhello\r\n$5\r\nworld\r\n`
    /// - An Array of three integers is encoded as follows:
    ///   `*3\r\n:1\r\n:2\r\n:3\r\n`
    /// - Mixed data types:
    ///   `*5\r\n:1\r\n:2\r\n:3\r\n:4\r\n$5\r\nhello\r\n` <=> `[1, 2, 3, 4, "hello"]`
    ///   The first line the server sent is `*5\r\n`.
    ///   This numeric value tells the client that five reply types are about to follow it.
    ///   Then, every successive reply constitutes an element in the array.
    /// - A nested array of two arrays:
    ///   `*2\r\n*3\r\n:1\r\n:2\r\n:3\r\n*2\r\n+Hello\r\n-World\r\n` <=> `[[1, 2, 3], ["Hello", ERR:"World"]]`
    ///   This encodes a two-element array. The first element is an array that, in turn, contains three integers
    ///   (1, 2, 3). The second element is another array containing a simple string and an error.
    /// - `None`: A null array: `*-1\r\n`
    /// - An array with null elements:
    ///   `*3\r\n$5\r\nhello\r\n$-1\r\n$5\r\nworld\r\n` <=> `["hello", None, "world"]`
    Array(Vec<Value>),
    NullArray,

    /// The string encoded in the error type is the error message itself.
    ///
    /// Errors are similar to simple strings, but their first character is the minus (-) character.
    ///
    /// Example:
    /// - `-Error message\r\n` <=> `Error message`; We only keep the `b"Error message"` portion, i.e., the raw contents.
    ///
    /// The client should raise an exception when it receives an Error reply.
    Error(Bytes),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resp::Value;
    use bytes::Bytes;

    #[test]
    fn test_parse_len_123456() {
        let input = Bytes::copy_from_slice(b"$123456\r\n");
        let (value, bytes_read) = Message::parse_len(&input).unwrap();
        let result = (value.unwrap(), bytes_read);
        let expected = (123456, 9);
        assert_eq!(expected, result);
    }

    #[test]
    fn test_parse_len_negative_one() {
        let input = Bytes::copy_from_slice(b"$-1\r\n");
        let result = Message::parse_len(&input).unwrap();
        let expected = (None, 5);
        assert_eq!(expected, result);
    }

    #[test]
    fn test_parse_len_negative_twelve() {
        let input = Bytes::copy_from_slice(b"$-12"); // Intentionally omitted CRLF at the end.
        if let Err(RESPError::NegativeLength) = Message::parse_len(&input) {
        } else {
            assert_eq!(0, 1);
        }
    }

    #[test]
    fn test_parse_len_negative_two() {
        let input = Bytes::copy_from_slice(b"$-2"); // Intentionally omitted CRLF at the end.
        if let Err(RESPError::NegativeLength) = Message::parse_len(&input) {
        } else {
            assert_eq!(0, 1);
        }
    }

    #[test]
    fn test_parse_len_negative_garbage1() {
        let input = Bytes::copy_from_slice(b"$-@"); // Intentionally omitted CRLF at the end.
        if let Err(RESPError::NegativeLength) = Message::parse_len(&input) {
        } else {
            assert_eq!(0, 1);
        }
    }

    #[test]
    fn test_parse_len_negative_garbage2() {
        let input = Bytes::copy_from_slice(b"$-1@\r\n"); // Intentionally omitted CRLF at the end.
        if let Err(RESPError::NegativeLength) = Message::parse_len(&input) {
        } else {
            assert_eq!(0, 1);
        }
    }

    #[test]
    fn test_parse_len_int_parse_err() {
        let input = Bytes::copy_from_slice(b"$1@"); // Intentionally omitted CRLF at the end.
        if let Err(RESPError::IntegerParseError(_)) = Message::parse_len(&input) {
        } else {
            assert_eq!(0, 1);
        }
    }

    #[test]
    fn test_deserialize_simple_string_empty() {
        let input = Bytes::copy_from_slice(b"+\r\n");
        let result = Message::deserialize_simple_string(&input).unwrap();
        let expected = (Value::SimpleString(Bytes::copy_from_slice(b"")), 3);
        assert_eq!(expected, result);
    }

    #[test]
    fn test_deserialize_simple_string_ok() {
        let input = Bytes::copy_from_slice(b"+OK\r\n");
        let result = Message::deserialize_simple_string(&input).unwrap();
        let expected = (Value::SimpleString(Bytes::copy_from_slice(b"OK")), 5);
        assert_eq!(expected, result);
    }

    #[test]
    fn test_deserialize_bulk_string_empty() {
        let input = Bytes::copy_from_slice(b"$0\r\n\r\n");
        let result = Message::deserialize_bulk_string(&input).unwrap();
        let expected = (Value::BulkString(Bytes::copy_from_slice(b"")), 6);
        assert_eq!(expected, result);
    }

    #[test]
    fn test_deserialize_bulk_string_hello() {
        let input = Bytes::copy_from_slice(b"$5\r\nHello\r\n");
        let result = Message::deserialize_bulk_string(&input).unwrap();
        let expected = (Value::BulkString(Bytes::copy_from_slice(b"Hello")), 11);
        assert_eq!(expected, result);
    }

    #[test]
    fn test_deserialize_bulk_string_null() {
        let input = Bytes::copy_from_slice(b"$-1\r\n");
        let result = Message::deserialize_bulk_string(&input).unwrap();
        let expected = (Value::NullBulkString, 5);
        assert_eq!(expected, result);
    }

    #[test]
    fn test_deserialize_integer_zero() {
        let input = Bytes::copy_from_slice(b":0\r\n");
        let result = Message::deserialize_integer(&input).unwrap();
        let expected = (Value::Integer(0), 4);
        assert_eq!(expected, result);
    }

    #[test]
    fn test_deserialize_integer_thousand() {
        let input = Bytes::copy_from_slice(b":1000\r\n");
        let result = Message::deserialize_integer(&input).unwrap();
        let expected = (Value::Integer(1000), 7);
        assert_eq!(expected, result);
    }

    #[test]
    fn test_deserialize_integer_plus_thousand() {
        let input = Bytes::copy_from_slice(b":+1000\r\n");
        let result = Message::deserialize_integer(&input).unwrap();
        let expected = (Value::Integer(1000), 8);
        assert_eq!(expected, result);
    }

    #[test]
    fn test_deserialize_integer_negative_thousand() {
        let input = Bytes::copy_from_slice(b":-1000\r\n");
        let result = Message::deserialize_integer(&input).unwrap();
        let expected = (Value::Integer(-1000), 8);
        assert_eq!(expected, result);
    }

    #[test]
    fn test_deserialize_array_empty() {
        let input = Bytes::copy_from_slice(b"*0\r\n");
        let result = Message::deserialize_array(&input).unwrap();
        let v = vec![];
        let expected = (Value::Array(v), 4);
        assert_eq!(expected, result);
    }

    #[test]
    fn test_deserialize_array_ping() {
        let input = Bytes::copy_from_slice(b"*1\r\n$4\r\nPING\r\n");
        let result = Message::deserialize_array(&input).unwrap();
        let v = vec![Value::BulkString(Bytes::copy_from_slice(b"PING"))];
        let expected = (Value::Array(v), 14);
        assert_eq!(expected, result);
    }

    #[test]
    fn test_deserialize_array_ping_with_arg() {
        let input = Bytes::copy_from_slice(b"*2\r\n$4\r\nPING\r\n$5\r\nHello\r\n");
        let result = Message::deserialize_array(&input).unwrap();
        let v = vec![
            Value::BulkString(Bytes::copy_from_slice(b"PING")),
            Value::BulkString(Bytes::copy_from_slice(b"Hello")),
        ];
        let expected = (Value::Array(v), 25);
        assert_eq!(expected, result);
    }

    #[test]
    fn test_deserialize_array_echo() {
        let input = Bytes::copy_from_slice(b"*2\r\n$4\r\nECHO\r\n$5\r\nHello\r\n");
        let result = Message::deserialize_array(&input).unwrap();
        let v = vec![
            Value::BulkString(Bytes::copy_from_slice(b"ECHO")),
            Value::BulkString(Bytes::copy_from_slice(b"Hello")),
        ];
        let expected = (Value::Array(v), 25);
        assert_eq!(expected, result);
    }

    #[test]
    fn test_deserialize_array_two_elts() {
        let input = Bytes::copy_from_slice(b"*2\r\n$5\r\nhello\r\n$5\r\nworld\r\n");
        let result = Message::deserialize_array(&input).unwrap();
        let v = vec![
            Value::BulkString(Bytes::copy_from_slice(b"hello")),
            Value::BulkString(Bytes::copy_from_slice(b"world")),
        ];
        let expected = (Value::Array(v), 26);
        assert_eq!(expected, result);
    }

    #[test]
    fn test_deserialize_array_integers() {
        let input = Bytes::copy_from_slice(b"*3\r\n:1\r\n:-2\r\n:3\r\n");
        let result = Message::deserialize_array(&input).unwrap();
        let v = vec![Value::Integer(1), Value::Integer(-2), Value::Integer(3)];
        let expected = (Value::Array(v), 17);
        assert_eq!(expected, result);
    }

    #[test]
    fn test_deserialize_array_mixed_data_types() {
        let input = Bytes::copy_from_slice(b"*5\r\n:1\r\n:2\r\n:3\r\n:4\r\n$5\r\nhello\r\n");
        let result = Message::deserialize_array(&input).unwrap();
        let v = vec![
            Value::Integer(1),
            Value::Integer(2),
            Value::Integer(3),
            Value::Integer(4),
            Value::BulkString(Bytes::from("hello")),
        ];
        let expected = (Value::Array(v), 31);
        assert_eq!(expected, result);
    }

    #[test]
    /// A nested array of two arrays:
    /// `*2\r\n*3\r\n:1\r\n:2\r\n:3\r\n*2\r\n+Hello\r\n-World\r\n `=> `[[1, 2, 3], ["Hello", ERR:"World"]]`
    ///
    /// This encodes a two-element array.
    /// The first element is an array that, in turn, contains three integers (1, 2, 3).
    /// The second element is another array containing a simple string and an error.
    fn test_deserialize_array_nested() {
        let input =
            Bytes::copy_from_slice(b"*2\r\n*3\r\n:1\r\n:2\r\n:3\r\n*2\r\n+Hello\r\n-World\r\n");
        let result = Message::deserialize_array(&input).unwrap();
        let v = vec![
            Value::Array(vec![
                Value::Integer(1),
                Value::Integer(2),
                Value::Integer(3),
            ]),
            Value::Array(vec![
                Value::SimpleString(Bytes::from("Hello")),
                Value::Error(Bytes::from("World")),
            ]),
        ];
        let expected = (Value::Array(v), 40);
        assert_eq!(expected, result);
    }

    #[test]
    fn test_deserialize_array_null() {
        let input = Bytes::copy_from_slice(b"*-1\r\n");
        let result = Message::deserialize_array(&input).unwrap();
        let expected = (Value::NullArray, 5);
        assert_eq!(expected, result);
    }

    #[test]
    fn test_deserialize_array_with_null_elt() {
        let input = Bytes::copy_from_slice(b"*3\r\n$5\r\nhello\r\n$-1\r\n$5\r\nworld\r\n");
        let result = Message::deserialize_array(&input).unwrap();
        let v = vec![
            Value::BulkString(Bytes::copy_from_slice(b"hello")),
            Value::NullBulkString,
            Value::BulkString(Bytes::copy_from_slice(b"world")),
        ];
        let expected = (Value::Array(v), 31);
        assert_eq!(expected, result);
    }

    #[test]
    /// A nested array of two arrays:
    /// `*2\r\n*3\r\n:1\r\n:2\r\n:3\r\n*2\r\n+Hello\r\n-World\r\n `=> `[[1, 2, 3], ["Hello", ERR:"World"]]`
    ///
    /// This encodes a two-element array.
    /// The first element is an array that, in turn, contains three integers (1, 2, 3).
    /// The second element is another array containing a simple string and an error.
    fn test_deserialize_a_nested_array() {
        let input =
            Bytes::copy_from_slice(b"*2\r\n*3\r\n:1\r\n:2\r\n:3\r\n*2\r\n+Hello\r\n-World\r\n");
        let (msg, bytes_read) = Message::deserialize(&input).unwrap();
        let value = msg.data;
        let result = (value, bytes_read);
        let v = vec![
            Value::Array(vec![
                Value::Integer(1),
                Value::Integer(2),
                Value::Integer(3),
            ]),
            Value::Array(vec![
                Value::SimpleString(Bytes::from("Hello")),
                Value::Error(Bytes::from("World")),
            ]),
        ];
        let expected = (Value::Array(v), 40);
        assert_eq!(expected, result);
    }
}
