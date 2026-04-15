#!/bin/bash
cargo build --release
sudo install -Dm755 target/release/hyprdeck /usr/local/sbin/hyprdeck
