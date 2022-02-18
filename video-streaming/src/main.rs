use base64::encode;
use nokhwa::{Camera, CameraFormat, FrameFormat};
use rav1e::*;
use rav1e::prelude::{ChromaSampling, ColorDescription, ColorPrimaries, TransferCharacteristics, MatrixCoefficients};
use rav1e::{config::SpeedSettings, prelude::FrameType};
use serde::{Deserialize, Serialize};
use serde_json;
use std::thread;
use std::sync::mpsc::{self, Sender, Receiver};
use image::{ImageBuffer, Rgb};
use log::info;

#[derive(Serialize, Deserialize, Debug)]
struct VideoPacket {
    data: String,
    frameType: String,
}

fn main() {
    let mut enc = EncoderConfig::default();
    let nc = nats::connect("nats:4222").unwrap();
    let width = 640;
    let height = 480;

    enc.width = width;
    enc.height = height;
    enc.bit_depth = 8;
    enc.error_resilient = true;
    enc.speed_settings = SpeedSettings::from_preset(10);
    enc.rdo_lookahead_frames = 1;
    enc.bitrate = 150;
    enc.min_key_frame_interval = 20;
    enc.max_key_frame_interval = 50;
    enc.low_latency = true;
    enc.min_quantizer = 30;
    enc.quantizer = 50;
    enc.still_picture = false;
    enc.tiles = 8;
    enc.chroma_sampling = ChromaSampling::Cs420;
    enc.color_description = Some(ColorDescription {
        color_primaries: ColorPrimaries::BT709,
        transfer_characteristics: TransferCharacteristics::BT709,
        matrix_coefficients: MatrixCoefficients::BT709 
    });
    

    let cfg = Config::new().with_encoder_config(enc).with_threads(4);

    let (tx, rx): (Sender<ImageBuffer<Rgb<u8>, Vec<u8>>>, Receiver<ImageBuffer<Rgb<u8>, Vec<u8>>>) = mpsc::channel();

    let write_thread = thread::spawn(move || {
        info!(r#"write thread: Opening camera"#);
        let mut camera = Camera::new(
            0,                                                             // index
            Some(CameraFormat::new_from(width as u32, height as u32, FrameFormat::MJPEG, 30)), // format
        )
        .unwrap();
        camera.open_stream().unwrap();
        info!("write thread: Starting write loop");
        loop {
            // info!("write thread: Waiting for camera frame");
            let frame = camera.frame().unwrap();
            // info!("write thread: sending frame");
            tx.send(frame).unwrap();
        }
    });

    let read_thread = thread::spawn(move || {
        let mut ctx: Context<u8> = cfg.new_context().unwrap();

        loop {
            let frame = rx.recv().unwrap();
            info!("read thread: Creating new frame");
            let mut encoding_frame = ctx.new_frame();
            let flat_samples = frame.as_flat_samples();
            for p in &mut encoding_frame.planes {
                
                let stride = (enc.width + p.cfg.xdec) >> p.cfg.xdec;
                p.copy_from_raw_u8(flat_samples.samples, stride, 1);
            } 
            match ctx.send_frame(encoding_frame) {
                Ok(_) => {
                    info!("read thread: queued frame");
                }
                Err(e) => match e {
                    EncoderStatus::EnoughData => {
                        info!("read thread: Unable to append frame to the internal queue");
                    }
                    _ => {
                        panic!("read thread: Unable to send frame");
                    }
                },
            }
            info!("read thread: receiving encoded frame");
            match ctx.receive_packet() {
                Ok(pkt) => {
                    info!("read thread: base64 Encoding packet {}", pkt.input_frameno);
                    let frame_type = if pkt.frame_type == FrameType::KEY {
                        "key"
                    } else {
                        "delta"
                    };
                    let data = encode(pkt.data);
                    info!("read thread: base64 Encoded packet {}", pkt.input_frameno);
                    let frame = VideoPacket {
                        data,
                        frameType: frame_type.to_string(),
                    };
                    let json = serde_json::to_string(&frame).unwrap();
                    nc.publish("video.1", json).unwrap();
                }
                Err(e) => match e {
                    EncoderStatus::LimitReached => {
                        info!("read thread: Limit reached");
                    }
                    EncoderStatus::Encoded => info!("read thread: Encoded"),
                    EncoderStatus::NeedMoreData => info!("read thread: Need more data"),
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
