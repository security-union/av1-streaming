#[macro_use]
extern crate log;

use anyhow::Result;
use bus::{Bus, BusReader};
use futures_util::{SinkExt, StreamExt};
use image::codecs;
use image::ImageBuffer;
use image::Rgb;
use nokhwa::{Camera, CameraFormat, CaptureAPIBackend, FrameFormat};
use rav1e::prelude::ChromaSampling;
use rav1e::*;
use rav1e::{config::SpeedSettings};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use std::{env, thread};
use video_streaming::types::oculus_controller_state::OculusControllerState;
use warp::{
    ws::{Message, WebSocket},
    Filter,
};

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug)]
struct VideoPacket {
    data: Option<String>,
    frameType: Option<String>,
    epochTime: Duration,
    encoding: Encoder,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
enum Encoder {
    MJPEG,
    AV1,
}

impl FromStr for Encoder {
    type Err = ();

    fn from_str(input: &str) -> Result<Encoder, Self::Err> {
        match input {
            "MJPEG" => Ok(Encoder::MJPEG),
            "AV1" => Ok(Encoder::AV1),
            _ => Err(()),
        }
    }
}

static THRESHOLD_MILLIS: u128 = 100;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    let mut enc = EncoderConfig::default();
    let width: usize = env::var("VIDEO_WIDTH")
        .ok()
        .map(|n| n.parse::<usize>().ok())
        .flatten()
        .unwrap_or(1920);
    let height: usize = env::var("VIDEO_HEIGHT")
        .ok()
        .map(|n| n.parse::<usize>().ok())
        .flatten()
        .unwrap_or(1080);

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
    let port: u16 = env::var("PORT")
        .ok()
        .map(|n| n.parse::<u16>().ok())
        .flatten()
        .unwrap_or(8080u16);
    let encoder = env::var("ENCODER")
        .ok()
        .map(|o| Encoder::from_str(o.as_ref()).ok())
        .flatten()
        .unwrap_or(Encoder::AV1);

    warn!("Framerate {framerate}");
    enc.width = width;
    enc.height = height;
    enc.bit_depth = 8;
    enc.error_resilient = true;
    enc.speed_settings = SpeedSettings::from_preset(10);
    enc.rdo_lookahead_frames = 1;
    enc.min_key_frame_interval = 20;
    enc.max_key_frame_interval = 50;
    enc.low_latency = true;
    enc.min_quantizer = 50;
    enc.quantizer = 100;
    enc.still_picture = false;
    enc.tiles = 4;
    enc.chroma_sampling = ChromaSampling::Cs444;

    let bus: Arc<Mutex<Bus<Vec<u8>>>> = Arc::new(Mutex::new(bus::Bus::new(10)));
    let bus_copy = bus.clone();
    let add_bus = warp::any().map(move || bus.clone());

    let client_counter: Arc<Mutex<u16>> = Arc::new(Mutex::new(0));
    let web_socket_counter = client_counter.clone();

    // Add counter to warp so that we can access it when we add/remove connections
    let add_counter = warp::any().map(move || web_socket_counter.clone());

    let (cam_tx, cam_rx): (
        Sender<(ImageBuffer<Rgb<u8>, Vec<u8>>, u128)>,
        Receiver<(ImageBuffer<Rgb<u8>, Vec<u8>>, u128)>,
    ) = mpsc::channel();

    let devices = nokhwa::query_devices(CaptureAPIBackend::Video4Linux)?;
    info!("available cameras: {:?}", devices);

    let camera_thread = thread::spawn(move || {
        loop {
            {
                info!("waiting for browser...");
                thread::sleep(Duration::from_millis(200));
                let counter = client_counter.lock().unwrap();
                if *counter <= 0 {
                    continue;
                }
            }
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
                    let counter = client_counter.lock().unwrap();
                    if *counter <= 0 {
                        break;
                    }
                }
                let frame = camera.frame().unwrap();
                cam_tx.send((frame, since_the_epoch().as_millis()));
            }
        }
    });

    let encoder_thread = thread::spawn(move || {
        loop {
            loop {
                let (mut frame, age) = cam_rx.recv().unwrap();
                // If age older than threshold, throw it away.
                let frame_age = since_the_epoch().as_millis() - age;
                debug!("frame age {}", frame_age);
                if frame_age > THRESHOLD_MILLIS {
                    info!("throwing away old frame with age {} ms", frame_age);
                    continue;
                }
                if encoder == Encoder::MJPEG {
                    let mut buf: Vec<u8> = Vec::new();
                    let mut jpeg_encoder =
                        codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, 30);
                    jpeg_encoder
                        .encode_image(&frame)
                        .map_err(|e| error!("{:?}", e));
                    bus_copy.lock().unwrap().broadcast(buf);
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
            |ws: warp::ws::Ws, bus: Arc<Mutex<Bus<Vec<u8>>>>, counter: Arc<Mutex<u16>>| {
                debug!("before creating upgrade");
                // And then our closure will be called when it completes...
                let reader = bus.lock().unwrap().add_rx();
                let counter_copy = counter.clone();
                debug!("adding client connection");
                ws.on_upgrade(|ws| client_connection(ws, reader, counter_copy))
            },
        );
    // WebSocker server thread
    warp::serve(routes).run(([0, 0, 0, 0], port)).await;
    encoder_thread.join().unwrap();
    camera_thread.join().unwrap();
    Ok(())
}

pub async fn client_connection(
    ws: WebSocket,
    mut reader: BusReader<Vec<u8>>,
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

    let sender = tokio::task::spawn(async move {
        loop {
            let next = reader.recv().unwrap();
            debug!("Forwarding video message");
            let time_serializing = Instant::now();
            match client_ws_sender.send(Message::binary(next)).await {
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
    });
    sender.await;
}

pub fn since_the_epoch() -> Duration {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap()
}
