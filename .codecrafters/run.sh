#!/bin/sh

set -e # Exit on failure

exec /tmp/build-redis-server/release/redis-server "$@"
