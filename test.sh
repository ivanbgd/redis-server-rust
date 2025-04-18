#!/bin/bash

# Fail immediately if any command has a non-zero exit status, or if a variable hasn't been defined.
set -eu

# Run test server at this port.
PORT=6380

# Start the test server.
echo "Starting the test server..."
RUST_LOG=info ./run.sh --port $PORT 2>/dev/null &
server_pid=$! # Get the pid of the background process.
sleep 1 # Give test server some time to start.
printf "Test server running...\n\n";

test_nr=0 # Test counter

# Runs a single test. It expects two arguments:
# 1. Redis command
# 2. Expected response
run_test()
{
  test_nr=$((test_nr+1))
  printf 'Running test #%d...' "$test_nr"
  # response=$(echo -ne "$1" | nc localhost $PORT; ret=$?; echo .; exit "$ret")
  response=$(echo -ne "$1" | nc localhost $PORT; echo .)
  response=${response%.}
  if [ "$response" = "$2" ]; then
    printf ' PASSED'
  else
    printf ' FAILED\nGot:\n%s' "$response"
    printf 'Expected:\n%s' "$2"
  fi
  echo
  # sleep 0.1
}


# Run all tests in succession now.

# PING
run_test "*1\r\n\$4\r\nPING\r\n"    $'+PONG\r\n' # 1
run_test "*2\r\n\$4\r\nPING\r\n\$4\r\nPING\r\n"    $'+PONG\r\n+PONG\r\n' # 2
run_test "*3\r\n\$4\r\nPinG\r\n\$4\r\nping\r\n\$4\r\npINg\r\n"    $'+PONG\r\n+PONG\r\n+PONG\r\n' # 3
run_test "*2\r\n\$4\r\nPING\r\n\$5\r\nHello\r\n"    $'$5\r\nHello\r\n' # 4
run_test "*2\r\n\$4\r\nPing\r\n\$8\r\nTest a B\r\n"    $'$8\r\nTest a B\r\n' # 5
run_test "*2\r\n\$4\r\nPING\r\n\$13\r\nHello, world!\r\n"    $'$13\r\nHello, world!\r\n' # 6

# ECHO
run_test "*2\r\n\$4\r\nECHO\r\n\$3\r\nHey\r\n"    $'$3\r\nHey\r\n' # 7
run_test "*2\r\n\$4\r\nECHO\r\n\$13\r\nHello, world!\r\n"    $'$13\r\nHello, world!\r\n' # 8
run_test "*4\r\n\$4\r\nEchO\r\n\$3\r\nHey\r\n\$4\r\nEchO\r\n\$3\r\nHey\r\n"    $'$3\r\nHey\r\n$3\r\nHey\r\n' # 9

# SET & GET
run_test "*3\r\n\$3\r\nSET\r\n\$5\r\nKey01\r\n\$7\r\nValue01\r\n"    $'+OK\r\n' # 10
run_test "*2\r\n\$3\r\nGET\r\n\$5\r\nApple\r\n"    $'$-1\r\n'
run_test "*2\r\n\$3\r\nGET\r\n\$5\r\nKey01\r\n"    $'$7\r\nValue01\r\n'

run_test "*5\r\n\$3\r\nSET\r\n\$5\r\nkey02\r\n\$7\r\nvalue02\r\n\$2\r\nPX\r\n\$3\r\n100\r\n"    $'+OK\r\n'
sleep 0.02
run_test "*2\r\n\$3\r\nGET\r\n\$5\r\nkey02\r\n"    $'$7\r\nvalue02\r\n'

run_test "*5\r\n\$3\r\nSET\r\n\$5\r\nkey03\r\n\$7\r\nvalue03\r\n\$2\r\npx\r\n\$3\r\n100\r\n"    $'+OK\r\n'
sleep 0.12
run_test "*2\r\n\$3\r\nGET\r\n\$5\r\nkey03\r\n"    $'$-1\r\n'

run_test "*5\r\n\$3\r\nSET\r\n\$5\r\nkey04\r\n\$7\r\nvalue04\r\n\$2\r\nEX\r\n\$2\r\n10\r\n"    $'+OK\r\n'
sleep 1.0
run_test "*2\r\n\$3\r\nGET\r\n\$5\r\nkey04\r\n"    $'$7\r\nvalue04\r\n'

run_test "*5\r\n\$3\r\nSET\r\n\$5\r\nkey05\r\n\$7\r\nvalue05\r\n\$2\r\nex\r\n\$1\r\n1\r\n"    $'+OK\r\n'
sleep 1.2
run_test "*2\r\n\$3\r\nGET\r\n\$5\r\nkey05\r\n"    $'$-1\r\n'

run_test "*3\r\n\$3\r\nSET\r\n\$5\r\nkey06\r\n\$7\r\nvalue06\r\n"    $'+OK\r\n'
run_test "*2\r\n\$3\r\nGET\r\n\$5\r\nkey06\r\n"    $'$7\r\nvalue06\r\n'
run_test "*5\r\n\$3\r\nSET\r\n\$5\r\nkey06\r\n\$7\r\nvalue06\r\n\$2\r\npX\r\n\$3\r\n100\r\n"    $'+OK\r\n'
sleep 0.02
run_test "*2\r\n\$3\r\nGET\r\n\$5\r\nkey06\r\n"    $'$7\r\nvalue06\r\n'
sleep 0.12
run_test "*2\r\n\$3\r\nGET\r\n\$5\r\nkey06\r\n"    $'$-1\r\n'

run_test "*5\r\n\$3\r\nSET\r\n\$5\r\nkey07\r\n\$7\r\nvalue07\r\n\$2\r\nPx\r\n\$3\r\n100\r\n"    $'+OK\r\n'
sleep 0.02
run_test "*2\r\n\$3\r\nGET\r\n\$5\r\nkey07\r\n"    $'$7\r\nvalue07\r\n'
run_test "*3\r\n\$3\r\nSET\r\n\$5\r\nkey07\r\n\$7\r\nvalue07\r\n"    $'+OK\r\n'
run_test "*2\r\n\$3\r\nGET\r\n\$5\r\nkey07\r\n"    $'$7\r\nvalue07\r\n'
sleep 0.12
run_test "*2\r\n\$3\r\nGET\r\n\$5\r\nkey07\r\n"    $'$7\r\nvalue07\r\n'

run_test "*5\r\n\$3\r\nSET\r\n\$5\r\nkey08\r\n\$7\r\nvalue08\r\n\$2\r\nPX\r\n\$4\r\n1000\r\n"    $'+OK\r\n'
sleep 0.5
run_test "*2\r\n\$3\r\nGET\r\n\$5\r\nkey08\r\n"    $'$7\r\nvalue08\r\n'
run_test "*5\r\n\$3\r\nSET\r\n\$5\r\nkey08\r\n\$7\r\nvalue08\r\n\$2\r\nPX\r\n\$4\r\n1000\r\n"    $'+OK\r\n'
sleep 0.6
run_test "*2\r\n\$3\r\nGET\r\n\$5\r\nkey08\r\n"    $'$7\r\nvalue08\r\n'
sleep 0.3
run_test "*2\r\n\$3\r\nGET\r\n\$5\r\nkey08\r\n"    $'$7\r\nvalue08\r\n'
sleep 0.2
run_test "*2\r\n\$3\r\nGET\r\n\$5\r\nkey08\r\n"    $'$-1\r\n'


# Stop the test server. First try by its PID, and if that fails, kill all running redis-server processes.
set +e # Don't fail immediately in case of non-zero exit status anymore, as we'll depend on this now.
sleep 0.1
echo
kill $server_pid
kill_status=$?
if [ $kill_status -eq 0 ]; then
  echo "Test server stopped."
else
  echo "kill $server_pid failed; killing all running redis-server processes now..."
  pkill redis-server
  echo "All redis-servers stopped."
fi
