#!/bin/bash -e

<<<<<<< HEAD
sudo apt-get install -qq gcc-arm-linux-gnueabihf
rustup target add armv7-unknown-linux-gnueabihf
mkdir -p ~/.cargo
cat >>~/.cargo/config <<EOF
[target.armv7-unknown-linux-gnueabihf]
linker = "arm-linux-gnueabihf-gcc"
=======
sudo apt-get install -qq gcc-aarch64-linux-gnu
rustup target add aarch64-unknown-linux-gnu
mkdir -p ~/.cargo
cat >>~/.cargo/config <<EOF
[target.aarch64-unknown-linux-gnu]
linker = "aarch64-linux-gnu-gcc"
>>>>>>> 0fa9752 (Testing on raspberry pi)
EOF
