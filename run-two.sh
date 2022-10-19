#!/bin/bash -e

export RUST_LOG=info

WEBSOCKET_PORT=8081 VIDEO_DEVICE_INDEX=0 ENCODER=MJPEG ~/video-streaming-with-port & VIDEO_DEVICE_INDEX=2 WEBSOCKET_PORT=8082 ENCODER=MJPEG ~/video-streaming-with-port
