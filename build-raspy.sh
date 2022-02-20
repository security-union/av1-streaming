#!/bin/bash -e

PI_IP=192.168.7.116
TARGET=armv7-unknown-linux-gnueabihf # Pi 2/3/4
#TARGET=arm-unknown-linux-gnueabihf # Pi 0/1

sudo apt update
sudo apt install -y libclang-dev libv4l-dev

export NATS_URL=localhost:4222
# build binary
cargo build --release --target $TARGET

# upload binary
ssh-copy-id pi@$PI_IP
scp -r ./target/$TARGET/release/video-streaming pi@$PI_IP:/tmp/
scp -r ./target/$TARGET/release/websocket-server pi@$PI_IP:/tmp/
