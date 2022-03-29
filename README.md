![Ww0Xt1mAMy31Ofar0GYu8Oab0v2k0uF1XT_zTt5kPU1M8o58sT5OOXCsSxv3nNGxsG8dG4zI=w1060-fcrop64=1,00005a57ffffa5a8-k-c0xffffffff-no-nd-rj (1)](https://user-images.githubusercontent.com/1176339/155262320-ce1406f0-d35d-418e-a8b9-60b928cceeb2.jpeg)


# 🔥🔥 Fearless AV1 live streaming for Linux and Raspberry PI using the rav1e encoder and Chrome's WebCodecs library 🔥🔥

[![IMAGE ALT TEXT](http://img.youtube.com/vi/ysqn2kKsvoE/0.jpg)](https://www.youtube.com/watch?v=ysqn2kKsvoE "Video Streaming")

## Goal
Use the latest and greatest open source technology to live stream from a Raspberry PI to the Chrome browser.

## TLDR

1. Start Docker `docker-compose up`

2. Download Chrome Canary from https://www.chromium.org/getting-involved/dev-channel/ for your OS, (App tested with Linux)

3. Go to `localhost:3000`

### Customization

Depending on your webcam, you might need to customize the framerate in the docker-compose.yaml file:

```
    environment:
      - RUST_LOG=info
      - FRAMERATE=30
      - VIDEO_DEVICE_INDEX=0
```

If you notice that the FPS is too low, just set FRAMERATE to 10

![Peek 2022-03-15 00-26](https://user-images.githubusercontent.com/1176339/158306781-101b8cae-5b9f-4f1d-aec6-b6f097a54be1.gif)


## AV1 Streaming
AOMedia Video 1 (AV1) is an open, royalty-free video coding format initially designed for video transmissions over the Internet. It was developed as a successor to VP9 by the Alliance for Open Media (AOMedia),

## WebCodecs
Modern web technologies provide ample ways to work with video. Media Stream API, Media Recording API, Media Source API, and WebRTC API add up to a rich tool set for recording, transferring, and playing video streams. While solving certain high-level tasks, these APIs don't let web programmers work with individual components of a video stream such as frames and unmuxed chunks of encoded video or audio. To get low-level access to these basic components, developers have been using WebAssembly to bring video and audio codecs into the browser. But given that modern browsers already ship with a variety of codecs (which are often accelerated by hardware), repackaging them as WebAssembly seems like a waste of human and computer resources.

WebCodecs API eliminates this inefficiency by giving programmers a way to use media components that are already present in the browser. 

Specifically:

1. Video and audio decoders
2. Video and audio encoders
3. Raw video frames
4. Image decoders
5. The WebCodecs API is useful for web applications that require full control over the way media content is processed, such as video editors, video conferencing, video streaming, etc.


## Tech Stack

### Raspberry PI App (video-streaming)
1. Camera recorder: nokhwa https://crates.io/crates/nokhwa
2. AV1 encoder: rav1e https://crates.io/crates/rav1e
3. WebSocket server: warp https://crates.io/crates/warp

### UI (web-ui)
1. UI Framework: React https://reactjs.org/
2. Web AV1 decoder: WebCodecs https://web.dev/webcodecs/
3. WebSockets: react-use-websocket https://www.npmjs.com/package/react-use-websocket



## 👤 Contributors ✨

<table>
<tr>
<td align="center"><a href="https://github.com/darioalessandro"><img src="https://avatars0.githubusercontent.com/u/1176339?s=400&v=4" width="100" alt=""/><br /><sub><b>Dario</b></sub></a></td>
<td align="center"><a href="https://github.com/griffobeid"><img src="https://avatars1.githubusercontent.com/u/12220672?s=400&u=639c5cafe1c504ee9c68ad3a5e09d1b2c186462c&v=4" width="100" alt=""/><br /><sub><b>Griffin Obeid</b></sub></a></td>    
</tr>
</table>

## Show your support

Give a ⭐️ if this project helped you!
