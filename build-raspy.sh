#!/bin/bash -e

USER=ubuntu
PI_IP=192.168.7.233
TARGET=aarch64-unknown-linux-gnu
#TARGET=armv7-unknown-linux-gnueabihf
#TARGET=arm-unknown-linux-gnueabihf

sudo apt update
sudo apt install -y libclang-dev libv4l-dev

export NATS_URL=localhost:4222
# build binary
cargo build --release --target $TARGET

# upload binary
ssh-copy-id $USER@$PI_IP
scp -r ./target/$TARGET/release/video-streaming $USER@$PI_IP:/tmp/
