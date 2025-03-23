[![progress-banner](https://backend.codecrafters.io/progress/redis/61e551a0-f64f-47c6-a6e1-ce0777abad69)](https://app.codecrafters.io/users/codecrafters-bot?r=2qF)

This is a starting point for Rust solutions to the
["Build Your Own Redis" Challenge](https://codecrafters.io/challenges/redis).

In this challenge, you'll build a toy Redis clone that's capable of handling
basic commands like `PING`, `SET` and `GET`. Along the way we'll learn about
event loops, the Redis protocol and more.

**Note**: If you're viewing this repo on GitHub, head over to
[codecrafters.io](https://codecrafters.io) to try the challenge.

# Passing the first stage

The entry point for your Redis implementation is in `src/main.rs`. Study and
uncomment the relevant code, and push your changes to pass the first stage:

```sh
git commit -am "pass 1st stage" # any msg
git push origin master
```

That's all!

# Stage 2 & beyond

Note: This section is for stages 2 and beyond.

1. Ensure you have `cargo (1.82)` installed locally
2. Run `./your_program.sh` to run your Redis server, which is implemented in
   `src/main.rs`. This command compiles your Rust project, so it might be slow
   the first time you run it. Subsequent runs will be fast.
3. Commit your changes and run `git push origin master` to submit your solution
   to CodeCrafters. Test output will be streamed to your terminal.

# Notes

- Some features that were not required in the challenge were added.
- Logging was added.
- Some commands were fully implemented, per official Redis specification,
  and not just partially as per the challenge requirements.
- Redis command names are case-insensitive, and we made them that way, while retaining case of their arguments.
- Handles multiple successive requests from the same connection.
- Supports multiple concurrent clients.
    - In addition to handling multiple commands from the same client,
      Redis servers are also designed to handle multiple clients at once.

# Running the Program

- If you would like to enable the added logging functionality, first set the `RUST_LOG` environment variable.
    - `export RUST_LOG=[trace | debug | info | warn]`


- Run the following to start our Redis server:

```shell
$ ./your_program.sh
$ export RUST_LOG=trace && ./your_program.sh
```

- Our Redis server will listen at the address `127.0.0.1:6379`.

- We can test it in another Terminal tab using, for example, `netcat`, like this:

```shell
$ echo -ne "*1\r\n$4\r\nPING\r\n" | nc localhost 6379
+PONG

$ echo -ne "*1\r\n$4\r\nPING\r\n*1\r\n$4\r\nPING\r\n" | nc localhost 6379
+PONG
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
```
