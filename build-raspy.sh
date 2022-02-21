#!/bin/bash -e

<<<<<<< HEAD
PI_IP=192.168.7.116
TARGET=armv7-unknown-linux-gnueabihf # Pi 2/3/4
#TARGET=arm-unknown-linux-gnueabihf # Pi 0/1
=======
USER=ubuntu
PI_IP=192.168.7.233
TARGET=aarch64-unknown-linux-gnu
#TARGET=armv7-unknown-linux-gnueabihf
#TARGET=arm-unknown-linux-gnueabihf
>>>>>>> 0fa9752 (Testing on raspberry pi)

sudo apt update
sudo apt install -y libclang-dev libv4l-dev

export NATS_URL=localhost:4222
# build binary
cargo build --release --target $TARGET

# upload binary
<<<<<<< HEAD
ssh-copy-id pi@$PI_IP
scp -r ./target/$TARGET/release/video-streaming pi@$PI_IP:/tmp/
scp -r ./target/$TARGET/release/websocket-server pi@$PI_IP:/tmp/
=======
ssh-copy-id $USER@$PI_IP
scp -r ./target/$TARGET/release/video-streaming $USER@$PI_IP:/tmp/
>>>>>>> 0fa9752 (Testing on raspberry pi)
