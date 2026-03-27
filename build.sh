#!/bin/bash
export CARGO_TARGET_DIR=/tmp/target
cargo build --release "$@"
