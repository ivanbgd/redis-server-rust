//! RESP: Redis Serialization Protocol
//!
//! [Official documentation](https://redis.io/docs/latest/develop/reference/protocol-spec/)

/// Redis serialization protocol (RESP) is the wire protocol that clients implement.
///
/// [ Redis serialization protocol specification](https://redis.io/docs/latest/develop/reference/protocol-spec/)
#[allow(unused)]
#[repr(u8)]
pub(crate) enum RESPType {
    /// Simple strings are encoded as a plus (+) character, followed by a string.
    /// The string mustn't contain a `CR` (`\r`) or `LF` (`\n`) character and is terminated by `CRLF` (i.e., `\r\n`).
    ///
    /// Simple strings transmit short, non-binary strings with minimal overhead.
    ///
    /// When Redis replies with a simple string, a client library should return to the caller a string value composed
    /// of the first character after the + up to the end of the string, excluding the final CRLF bytes.
    ///
    /// Example:
    /// - `"OK"`: `+OK\r\n`.
    SimpleString = b'+',

    /// A bulk string represents a single binary string.
    /// The string can be of any size, but by default, Redis limits it to 512 MB.
    ///
    /// RESP encodes bulk strings in the following way:
    ///
    /// `$<length>\r\n<data>\r\n`
    ///
    /// A bulk string contains length; `-1` represents a `Null` value.
    ///
    /// The null bulk string represents a non-existing value.
    /// The `GET` command returns the Null Bulk String when the target key doesn't exist.
    /// It is encoded as a bulk string with the length of negative one (-1).
    /// A Redis client should return a nil object when the server replies with a null bulk string
    /// rather than the empty string.
    ///
    /// Examples:
    /// - `"hello"`: `$5\r\nhello\r\n`
    /// - The empty string's encoding is: `$0\r\n\r\n`
    /// - A Null Bulk String: `$-1\r\n`
    BulkString(i32) = b'$',

    /// This type is a `CRLF`-terminated string that represents a signed, base-10, 64-bit integer.
    ///
    /// RESP encodes integers in the following way:
    ///
    /// `:[<+|->]<value>\r\n`
    ///
    /// Many Redis commands return RESP integers, including `INCR`, `LLEN`, and `LASTSAVE`.
    /// An integer, by itself, has no special meaning other than in the context of the command that returned it.
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
    /// An array contains length; `-1` represents a `Null` value.
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
    /// - An empty array: `*0\r\n`
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
    /// - A null array: `*-1\r\n`
    /// - An array with null elements:
    ///   `*3\r\n$5\r\nhello\r\n$-1\r\n$5\r\nworld\r\n` <=> `["hello", nil, "world"]`
    Array(i32) = b'*',

    /// The string encoded in the error type is the error message itself.
    ///
    /// Errors are similar to simple strings, but their first character is the minus (-) character.
    ///
    /// Example: `-Error message\r\n` => `Error message`
    ///
    /// The client should raise an exception when it receives an Error reply.
    Error = b'-',
}

/// A RESP message
#[allow(unused)]
pub(crate) struct Message {
    prefix: RESPType,
    data: String,
}
