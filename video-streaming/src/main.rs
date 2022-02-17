use base64::encode;
use nokhwa::{Camera, CameraFormat, FrameFormat};
use rav1e::*;
use rav1e::{config::SpeedSettings, prelude::FrameType};
use serde::{Deserialize, Serialize};
use serde_json;
use std::thread;
use std::sync::{Arc, Mutex};
use std::cell::Cell;

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
    enc.speed_settings = SpeedSettings::from_preset(7);
    enc.bitrate = 290;

    let cfg = Config::new().with_encoder_config(enc);

    let ctx: Arc<Cell<Context<u16>>> = Arc::new(Cell::new(cfg.new_context().unwrap()));
    let mut write_ctx = Arc::clone(&ctx);
    let write_thread = thread::spawn(move || {
        println!("Opening camera");
        let mut camera = Camera::new(
            0,                                                             // index
            Some(CameraFormat::new_from(320, 240, FrameFormat::YUYV, 30)), // format
        )
        .unwrap();
        camera.open_stream().unwrap();
        println!("Starting write loop");
        loop {
            println!("Creating new frame");
            let mut encoding_frame = write_ctx.into_inner().new_frame();
            println!("Waiting for camera frame");
            let frame = camera.frame().unwrap();
            println!("Copying camera frames");
            let flat_samples = frame.as_flat_samples();
            for p in &mut encoding_frame.planes {
                let stride = (enc.width + p.cfg.xdec) >> p.cfg.xdec;
                p.copy_from_raw_u8(flat_samples.samples, stride, 1);
            }
            println!("sending frame");
            match write_ctx.into_inner().send_frame(encoding_frame) {
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
        }
    });

    let read_ctx = Arc::clone(&ctx);
    let read_thread = thread::spawn(move || {
        loop {
            println!("receiving frame");
            // std::thread::sleep(Duration::from_millis(10));
            match read_ctx.into_inner().receive_packet() {
                Ok(pkt) => {
                    println!("Encoding packet {}", pkt.input_frameno);
                    let frame_type = if pkt.frame_type == FrameType::KEY {
                        "key"
                    } else {
                        "delta"
                    };
                    let data = encode(pkt.data);
                    println!("Encoded packet {}", pkt.input_frameno);
                    let frame = VideoPacket {
                        data,
                        frameType: frame_type.to_string(),
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
    });
    write_thread.join().unwrap();
    read_thread.join().unwrap();
}
