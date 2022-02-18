use base64::encode;
use nokhwa::{Camera, CameraFormat, FrameFormat};
use rav1e::*;
use rav1e::prelude::ColorDescription;
use rav1e::{config::SpeedSettings, prelude::FrameType};
use serde::{Deserialize, Serialize};
use serde_json;
use std::thread;
use std::sync::mpsc::{self, Sender, Receiver};
use image::{ImageBuffer, Rgb};


#[derive(Serialize, Deserialize, Debug)]
struct VideoPacket {
    data: String,
    frameType: String,
}

fn main() {
    let mut enc = EncoderConfig::default();
    // let nc = nats::connect("nats:4222").unwrap();

    enc.width = 320;
    enc.height = 240;
    enc.bit_depth = 8;
    enc.error_resilient = true;
    enc.speed_settings = SpeedSettings::from_preset(10);
    enc.rdo_lookahead_frames = 1;
    enc.bitrate = 150;
    enc.min_key_frame_interval = 10;
    enc.max_key_frame_interval = 20;
    enc.low_latency = true;
    enc.min_quantizer = 30;
    enc.quantizer = 50;
    enc.still_picture = false;

    let cfg = Config::new().with_encoder_config(enc).with_threads(8);

    let (tx, rx): (Sender<ImageBuffer<Rgb<u8>, Vec<u8>>>, Receiver<ImageBuffer<Rgb<u8>, Vec<u8>>>) = mpsc::channel();

    let write_thread = thread::spawn(move || {
        println!("write thread: Opening camera");
        let mut camera = Camera::new(
            0,                                                             // index
            Some(CameraFormat::new_from(320, 240, FrameFormat::YUYV, 30)), // format
        )
        .unwrap();
        camera.open_stream().unwrap();
        println!("write thread: Starting write loop");
        loop {
            // println!("write thread: Waiting for camera frame");
            let frame = camera.frame().unwrap();
            // println!("write thread: sending frame");
            tx.send(frame).unwrap();
        }
    });

    let read_thread = thread::spawn(move || {
        let mut ctx: Context<u8> = cfg.new_context().unwrap();

        loop {
            let frame = rx.recv().unwrap();
            println!("read thread: Creating new frame");
            let mut encoding_frame = ctx.new_frame();
            let flat_samples = frame.as_flat_samples();
            for p in &mut encoding_frame.planes {
                let stride = (enc.width + p.cfg.xdec) >> p.cfg.xdec;
                p.copy_from_raw_u8(flat_samples.samples, stride, 1);
            } 
            match ctx.send_frame(encoding_frame) {
                Ok(_) => {
                    println!("read thread: queued frame");
                }
                Err(e) => match e {
                    EncoderStatus::EnoughData => {
                        println!("read thread: Unable to append frame to the internal queue");
                    }
                    _ => {
                        panic!("read thread: Unable to send frame");
                    }
                },
            }
            println!("read thread: receiving encoded frame");
            match ctx.receive_packet() {
                Ok(pkt) => {
                    println!("read thread: base64 Encoding packet {}", pkt.input_frameno);
                    let frame_type = if pkt.frame_type == FrameType::KEY {
                        "key"
                    } else {
                        "delta"
                    };
                    let data = encode(pkt.data);
                    println!("read thread: base64 Encoded packet {}", pkt.input_frameno);
                    let frame = VideoPacket {
                        data,
                        frameType: frame_type.to_string(),
                    };
                    let json = serde_json::to_string(&frame).unwrap();
                    // nc.publish("video.1", json).unwrap();
                }
                Err(e) => match e {
                    EncoderStatus::LimitReached => {
                        println!("read thread: Limit reached");
                    }
                    EncoderStatus::Encoded => println!("read thread: Encoded"),
                    EncoderStatus::NeedMoreData => println!("read thread: Need more data"),
                    _ => {
                        panic!("read thread: Unable to receive packet");
                    }
                },
            }
        }
    });
    write_thread.join().unwrap();
    read_thread.join().unwrap();
}
