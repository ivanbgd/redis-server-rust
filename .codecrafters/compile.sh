#!/bin/sh

set -e # Exit on failure

cargo build --release --target-dir=/tmp/build-redis-server-rust --manifest-path Cargo.toml
