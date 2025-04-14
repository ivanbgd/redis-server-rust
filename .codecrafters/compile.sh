#!/bin/sh

set -e # Exit on failure

cargo build --release --target-dir=/tmp/build-redis-server --manifest-path Cargo.toml
