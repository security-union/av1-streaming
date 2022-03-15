#[macro_use]
extern crate log;

use anyhow::Result;
use base64::encode;
use bus::{Bus, BusReader};
use futures_util::{SinkExt, StreamExt};
use image::Rgb;
use nokhwa::{Camera, CameraFormat, CaptureAPIBackend, FrameFormat};
use rav1e::prelude::ChromaSampling;
use rav1e::*;
use rav1e::{config::SpeedSettings, prelude::FrameType};
use serde::{Deserialize, Serialize};
use serde_json;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use std::{env, thread};
use warp::{
    ws::{Message, WebSocket},
    Filter,
};

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug)]
struct VideoPacket {
    data: Option<String>,
    frameType: String,
    epochTime: Duration,
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    let mut enc = EncoderConfig::default();
    let width = 640;
    let height = 480;
    let video_device_index: usize = env::var("VIDEO_DEVICE_INDEX")
        .ok()
        .map(|n| n.parse::<usize>().ok())
        .flatten()
        .unwrap_or(0);
    let framerate: u32 = env::var("FRAMERATE")
        .ok()
        .map(|n| n.parse::<u32>().ok())
        .flatten()
        .unwrap_or(10u32);
    warn!("Framerate {framerate}");

    enc.width = width;
    enc.height = height;
    enc.bit_depth = 8;
    enc.error_resilient = true;
    enc.speed_settings = SpeedSettings::from_preset(10);
    enc.rdo_lookahead_frames = 1;
    enc.bitrate = 256;
    enc.min_key_frame_interval = 20;
    enc.max_key_frame_interval = 50;
    enc.low_latency = true;
    enc.min_quantizer = 100;
    enc.quantizer = 120;
    enc.still_picture = false;
    enc.tiles = 8;
    enc.chroma_sampling = ChromaSampling::Cs444;

    let bus: Arc<Mutex<Bus<String>>> = Arc::new(Mutex::new(bus::Bus::new(10)));
    let counter: Arc<Mutex<u16>> = Arc::new(Mutex::new(0));
    let bus_copy = bus.clone();
    let add_bus = warp::any().map(move || bus.clone());
    let web_socket_counter = counter.clone();
    let add_counter = warp::any().map(move || web_socket_counter.clone());

    let cfg = Config::new().with_encoder_config(enc).with_threads(4);

    let (fps_tx, fps_rx): (Sender<u128>, Receiver<u128>) = mpsc::channel();

    let devices = nokhwa::query_devices(CaptureAPIBackend::Video4Linux)?;
    info!("available cameras: {:?}", devices);

    let fps_thread = thread::spawn(move || {
        let mut num_frames = 0;
        let mut now_plus_1 = since_the_epoch().as_millis() + 1000;
        warn!("Starting fps loop");
        loop {
            match fps_rx.recv() {
                Ok(dur) => {
                    if now_plus_1 < dur {
                        warn!("FPS: {:?}", num_frames);
                        num_frames = 0;
                        now_plus_1 = since_the_epoch().as_millis() + 1000;
                    } else {
                        num_frames += 1;
                    }
                }
                Err(e) => {
                    error!("Receive error: {:?}", e);
                }
            }
        }
    });

    let encoding_thread = thread::spawn(move || {
        loop {
            {
                info!("waiting for browser...");
                thread::sleep(Duration::from_millis(200));
                let counter = counter.lock().unwrap();
                if *counter <= 0 {
                    continue;
                }
            }
            let fps_tx_copy = fps_tx.clone();
            let mut ctx: Context<u8> = cfg.new_context().unwrap();
            let mut camera = Camera::new(
                video_device_index, // index
                Some(CameraFormat::new_from(
                    width as u32,
                    height as u32,
                    FrameFormat::MJPEG,
                    framerate,
                )), // format
            )
            .unwrap();
            camera.open_stream().unwrap();
            loop {
                {
                    debug!("blocking after starting camera");
                    let counter = counter.lock().unwrap();
                    if *counter <= 0 {
                        warn!("stopping the recording");
                        break;
                    }
                }
                debug!("grabbing frame");
                let mut frame = camera.frame().unwrap();
                let mut r_slice: Vec<u8> = vec![];
                let mut g_slice: Vec<u8> = vec![];
                let mut b_slice: Vec<u8> = vec![];
                for pixel in frame.pixels_mut() {
                    let (r, g, b) = to_ycbcr(pixel);
                    r_slice.push(r);
                    g_slice.push(g);
                    b_slice.push(b);
                }
                let planes = vec![r_slice, g_slice, b_slice];
                debug!("Creating new frame");
                let mut frame = ctx.new_frame();
                let encoding_time = Instant::now();
                for (dst, src) in frame.planes.iter_mut().zip(planes) {
                    dst.copy_from_raw_u8(&src, enc.width, 1);
                }

                match ctx.send_frame(frame) {
                    Ok(_) => {
                        debug!("queued frame");
                    }
                    Err(e) => match e {
                        EncoderStatus::EnoughData => {
                            debug!("Unable to append frame to the internal queue");
                        }
                        _ => {
                            panic!("Unable to send frame");
                        }
                    },
                }
                debug!("receiving encoded frame");
                match ctx.receive_packet() {
                    Ok(pkt) => {
                        debug!("time encoding {:?}", encoding_time.elapsed());
                        debug!("read thread: base64 Encoding packet {}", pkt.input_frameno);
                        let frame_type = if pkt.frame_type == FrameType::KEY {
                            "key"
                        } else {
                            "delta"
                        };
                        let time_serializing = Instant::now();
                        let data = encode(pkt.data);
                        debug!("read thread: base64 Encoded packet {}", pkt.input_frameno);
                        let frame = VideoPacket {
                            data: Some(data),
                            frameType: frame_type.to_string(),
                            epochTime: since_the_epoch(),
                        };
                        let json = serde_json::to_string(&frame).unwrap();
                        bus_copy.lock().unwrap().broadcast(json);
                        debug!("time serializing {:?}", time_serializing.elapsed());
                        fps_tx_copy.send(since_the_epoch().as_millis()).unwrap();
                    }
                    Err(e) => match e {
                        EncoderStatus::LimitReached => {
                            warn!("read thread: Limit reached");
                        }
                        EncoderStatus::Encoded => debug!("read thread: Encoded"),
                        EncoderStatus::NeedMoreData => debug!("read thread: Need more data"),
                        _ => {
                            warn!("read thread: Unable to receive packet");
                        }
                    },
                }
            }
        }
    });

    let routes = warp::path("ws")
        // The `ws()` filter will prepare the Websocket handshake.
        .and(warp::ws())
        .and(add_bus)
        .and(add_counter)
        .map(
            |ws: warp::ws::Ws, bus: Arc<Mutex<Bus<String>>>, counter: Arc<Mutex<u16>>| {
                debug!("before creating upgrade");
                // And then our closure will be called when it completes...
                let reader = bus.lock().unwrap().add_rx();
                let counter_copy = counter.clone();
                debug!("adding client connection");
                ws.on_upgrade(|ws| client_connection(ws, reader, counter_copy))
            },
        );
    warp::serve(routes).run(([0, 0, 0, 0], 8080)).await;
    encoding_thread.join().unwrap();
    fps_thread.join().unwrap();
    Ok(())
}

fn clamp(val: f32) -> u8 {
    return (val.round() as u8).max(0_u8).min(255_u8);
}

fn to_ycbcr(pixel: &Rgb<u8>) -> (u8, u8, u8) {
    let [r, g, b] = pixel.0;

    let y = 16_f32 + (65.481 * r as f32 + 128.553 * g as f32 + 24.966 * b as f32) / 255_f32;
    let cb = 128_f32 + (-37.797 * r as f32 - 74.203 * g as f32 + 112.000 * b as f32) / 255_f32;
    let cr = 128_f32 + (112.000 * r as f32 - 93.786 * g as f32 - 18.214 * b as f32) / 255_f32;

    return (clamp(y), clamp(cb), clamp(cr));
}

pub async fn client_connection(
    ws: WebSocket,
    mut reader: BusReader<String>,
    counter: Arc<Mutex<u16>>,
) {
    info!("establishing client connection... {:?}", ws);
    let (mut client_ws_sender, _client_ws_rcv) = ws.split();
    {
        info!("blocking before adding connection {:?}", counter);
        let mut counter_ref = counter.lock().unwrap();
        *counter_ref = *counter_ref + 1;
        info!("adding connection, connection counter: {:?}", *counter_ref);
        drop(counter_ref);
    }
    loop {
        let next = reader.recv().unwrap();
        debug!("Forwarding video message");
        let time_serializing = Instant::now();
        match client_ws_sender.send(Message::text(next)).await {
            Ok(_) => {}
            Err(_e) => {
                info!("blocking before removing connection {:?}", counter);
                let mut counter_ref = counter.lock().unwrap();
                *counter_ref = *counter_ref - 1;
                info!(
                    "Removing connection, connection counter: {:?}",
                    *counter_ref
                );
                break;
            }
        }
        debug!("web_socket serializing {:?}", time_serializing.elapsed());
    }
}

pub fn since_the_epoch() -> Duration {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap()
}
