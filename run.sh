#!/bin/sh

set -e # Exit early if any command fails

(
  cd "$(dirname "$0")" # Ensure compile steps are run within the repository directory
  cargo build --release --target-dir=/tmp/build-redis-server --manifest-path Cargo.toml
)

exec /tmp/build-redis-server/release/redis-server "$@"
