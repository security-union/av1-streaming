#!/bin/bash -e

sudo apt-get install -qq gcc-aarch64-linux-gnu
rustup target add aarch64-unknown-linux-gnu
mkdir -p ~/.cargo
cat >>~/.cargo/config <<EOF
[target.aarch64-unknown-linux-gnu]
linker = "aarch64-linux-gnu-gcc"
EOF
