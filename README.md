# A Redis Clone Implementation in Rust

# Notes

- Logging has been added.
- Some commands were fully implemented, per official Redis specification.
- Redis command names are case-insensitive, and we made them that way, while retaining case of their arguments.
- Handles multiple successive requests from the same connection.
- Supports multiple concurrent clients.
    - In addition to handling multiple commands from the same client,
      Redis servers are also designed to handle multiple clients at once.

# Running the Program

- Help is available.

```shell
$ ./run.sh --help
```

- If you would like to enable the added logging functionality, first set the `RUST_LOG` environment variable.
    - `export RUST_LOG=[trace | debug | info | warn]`


- Run the following to start our Redis server:

```shell
$ ./run.sh
$ ./run.sh --port 6380
$ RUST_LOG=trace ./run.sh
```

- Our Redis server will listen at the default address `127.0.0.1:6379`, but port can be changed through a
  command line argument.

- We can test it in another Terminal tab using, for example, `netcat`, like this:

```shell
$ echo -ne "*1\r\n$4\r\nPING\r\n" | nc localhost 6379
+PONG

$ echo -ne "*2\r\n$4\r\nPING\r\n$4\r\nPING\r\n" | nc localhost 6379
+PONG
+PONG

$ echo -ne "*2\r\n$4\r\nPING\r\n$13\r\nHello, world!\r\n" | nc localhost 6379 
$13
Hello, world!

$ echo -ne "*2\r\n$4\r\nECHO\r\n$3\r\nHey\r\n" | nc localhost 6379
$3
Hey

$ echo -ne "*3\r\n\$3\r\nSET\r\n\$6\r\norange\r\n\$9\r\npineapple\r\n" | nc localhost 6379
+OK
$ echo -ne "*2\r\n\$3\r\nGET\r\n\$6\r\norange\r\n" | nc localhost 6379
$9
pineapple
```

# Supported Redis Commands

- [ECHO](https://redis.io/docs/latest/commands/echo/)
- [GET](https://redis.io/docs/latest/commands/get/)
- [PING](https://redis.io/docs/latest/commands/ping/)
- [SET [EX | PX]](https://redis.io/docs/latest/commands/set/)
