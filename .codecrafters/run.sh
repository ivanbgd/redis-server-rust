#!/bin/sh

set -e # Exit on failure

exec /tmp/build-redis-rust/release/redis-server "$@"
