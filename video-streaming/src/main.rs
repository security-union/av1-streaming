use base64::encode;
use nokhwa::{Camera, CameraFormat, FrameFormat};
use rav1e::*;
use rav1e::{config::SpeedSettings, prelude::FrameType};
use serde::{Deserialize, Serialize};
use serde_json;
use std::thread;

#[derive(Serialize, Deserialize, Debug)]
struct VideoPacket {
    data: String,
    frameType: String,
}

fn main() {
    let mut enc = EncoderConfig::default();
    let nc = nats::connect("nats:4222").unwrap();

    enc.width = 320;
    enc.height = 240;
    enc.bit_depth = 8;
    enc.error_resilient = true;
    enc.speed_settings = SpeedSettings::from_preset(10);
    enc.low_latency = true;

    let cfg = Config::new().with_encoder_config(enc).with_threads(4);

    let mut ctx: Context<u16> = cfg.new_context().unwrap();

    // set up the Camera
    let mut camera = Camera::new(
        0,                                                             // index
        Some(CameraFormat::new_from(320, 240, FrameFormat::YUYV, 30)), // format
    )
    .unwrap();
    // open stream
    camera.open_stream().unwrap();
    loop {
        println!("Creating new frame");
        let mut encoding_frame = ctx.new_frame();
        println!("Waiting for camera frame");
        let frame = camera.frame().unwrap();
        println!("Copying camera frames");
        let flat_samples = frame.as_flat_samples();
        for p in &mut encoding_frame.planes {
            let stride = (enc.width + p.cfg.xdec) >> p.cfg.xdec;
            p.copy_from_raw_u8(flat_samples.samples, stride, 1);
        }
        println!("sending frame");
        match ctx.send_frame(encoding_frame) {
            Ok(_) => {
                println!("sent frame");
            }
            Err(e) => match e {
                EncoderStatus::EnoughData => {
                    println!("Unable to append frame to the internal queue");
                }
                _ => {
                    panic!("Unable to send frame");
                }
            },
        }
        println!("receiving frame");
        match ctx.receive_packet() {
            Ok(pkt) => {
                println!("Encoding packet {}", pkt.input_frameno);
                let frameType = if pkt.frame_type == FrameType::KEY {
                    "key"
                } else {
                    "delta"
                };
                let data = encode(pkt.data);
                println!("Encoded packet {}", pkt.input_frameno);
                let frame = VideoPacket {
                    data,
                    frameType: frameType.to_string(),
                };
                let json = serde_json::to_string(&frame).unwrap();
                nc.publish("video.1", json).unwrap();
            }
            Err(e) => match e {
                EncoderStatus::LimitReached => {
                    println!("Limit reached");
                }
                EncoderStatus::Encoded => println!("Encoded"),
                EncoderStatus::NeedMoreData => println!("Need more data"),
                _ => {
                    panic!("Unable to receive packet");
                }
            },
        }
    }
}
