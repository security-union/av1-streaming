#[macro_use]
extern crate log;

use base64::encode;
use bus::{BusReader, Bus};
use nokhwa::{Camera, CameraFormat, FrameFormat};
use rav1e::*;
use rav1e::{config::SpeedSettings, prelude::FrameType};
use serde::{Deserialize, Serialize};
use serde_json;
use std::sync::{Mutex, Arc};
use std::thread;
use std::sync::mpsc::{self, Sender, Receiver};
use std::time::{SystemTime, Instant, Duration, UNIX_EPOCH};
use image::{Rgb};
use futures_util::{StreamExt, SinkExt};
use warp::{Filter, ws::{WebSocket, Message}};



#[derive(Serialize, Deserialize, Debug)]
struct VideoPacket {
    data: Option<String>,
    frameType: String,
    epochTime: Duration,
}



#[tokio::main]
async fn main() {
    env_logger::init();
    let mut enc = EncoderConfig::default();
    let width = 640;
    let height = 480;

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
    // enc.chroma_sampling = ChromaSampling::Cs420;
    // enc.color_description = Some(ColorDescription {
    //     color_primaries: ColorPrimaries::BT709,
    //     transfer_characteristics: TransferCharacteristics::BT709,
    //     matrix_coefficients: MatrixCoefficients::BT709 
    // });
    let bus: Arc<Mutex<Bus<String>>>  = Arc::new(Mutex::new(bus::Bus::new(10)));
    let counter: Arc<Mutex<u16>> = Arc::new(Mutex::new(0));
    let bus_copy = bus.clone();
    let add_bus = warp::any().map(move || bus.clone());
    let add_counter_copy = counter.clone();
    let add_counter = warp::any().map(move || add_counter_copy.clone());

    let cfg = Config::new().with_encoder_config(enc).with_threads(4);

    let (tx, rx): (Sender<Vec<Vec<u8>>>, Receiver<Vec<Vec<u8>>>) = mpsc::channel();
    let (fps_tx, fps_rx): (Sender<u128>, Receiver<u128>) = mpsc::channel();

    let fps_thread = thread::spawn(move || {
        let mut num_frames = 0;
        let mut now_plus_1 = since_the_epoch().as_millis() + 1000;
        println!("Starting fps loop");
        loop {
        match fps_rx.recv() {
            Ok(dur) => {
                if now_plus_1 < dur {
                    println!("FPS: {:?}", num_frames);
                    num_frames = 0;
                    now_plus_1 = since_the_epoch().as_millis() + 1000;
                } else {    
                    num_frames += 1;
                }
            },
            Err(e) => {
                println!("Receive error: {:?}", e);
            }
        }
    }
    });
    

    let write_thread = thread::spawn(move || {
        info!(r#"write thread: Opening camera"#);
        let mut camera = Camera::new(
            0,                                                             // index
            Some(CameraFormat::new_from(width as u32, height as u32, FrameFormat::MJPEG, 10)), // format
        )
        .unwrap();
        camera.open_stream().unwrap();
        info!("write thread: Starting write loop");
        loop {
            let mut frame = camera.frame().unwrap();
            let mut r_slice: Vec<u8> = vec!();
            let mut g_slice: Vec<u8> = vec!();
            let mut b_slice: Vec<u8> = vec!();
            for pixel in frame.pixels_mut() {
                let (r, g, b) = to_ycbcr(pixel);
                r_slice.push(r);
                g_slice.push(g);
                b_slice.push(b);
            }
            tx.send(vec!(r_slice, g_slice, b_slice)).unwrap();
        }
    });

    let read_thread = thread::spawn(move || {
        let mut ctx: Context<u8> = cfg.new_context().unwrap();
        loop {
            let counter = counter.lock().unwrap();
            if *counter <= 0 {
                break;
            }
            let planes = rx.recv().unwrap();
            info!("read thread: Creating new frame");
            let mut frame = ctx.new_frame();
            let encoding_time = Instant::now();
            for (dst, src) in frame.planes.iter_mut().zip(planes) {
                dst.copy_from_raw_u8(&src, enc.width, 1);
            }

            match ctx.send_frame(frame) {
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
                    warn!("time encoding {:?}",  encoding_time.elapsed());
                    info!("read thread: base64 Encoding packet {}", pkt.input_frameno);
                    let frame_type = if pkt.frame_type == FrameType::KEY {
                        "key"
                    } else {
                        "delta"
                    };
                    let time_serializing = Instant::now();
                    let data = encode(pkt.data);
                    info!("read thread: base64 Encoded packet {}", pkt.input_frameno);
                    let frame = VideoPacket {
                        data: Some(data),
                        frameType: frame_type.to_string(),
                        epochTime: since_the_epoch(),
                    };
                    let json = serde_json::to_string(&frame).unwrap();
                    bus_copy.lock().unwrap().broadcast(json);
                    warn!("time serializing {:?}", time_serializing.elapsed());
                    fps_tx.send(since_the_epoch().as_millis()).unwrap();
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

    let routes = warp::path("ws")
         // The `ws()` filter will prepare the Websocket handshake.
        .and(warp::ws())
        .and(add_bus)
        .and(add_counter)
        .map( |ws: warp::ws::Ws, bus: Arc<Mutex<Bus<String>>>, counter: Arc<Mutex<u16>>| {
            // And then our closure will be called when it completes...
            let reader = bus.lock().unwrap().add_rx();

            ws.on_upgrade(|ws| client_connection(ws, reader))
        });
    warp::serve(routes).run(([0, 0, 0, 0], 8080)).await;
    write_thread.join().unwrap();
    read_thread.join().unwrap();
    fps_thread.join().unwrap();
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


pub async fn client_connection(ws: WebSocket, mut reader: BusReader<String>) {
    println!("establishing client connection... {:?}", ws);
    let (mut client_ws_sender, _client_ws_rcv) = ws.split();

    loop {
       let next = reader.recv().unwrap();
        info!("Forwarding video message");
        let time_serializing = Instant::now();
        client_ws_sender.send(Message::text(next)).await.unwrap();
        warn!("web_socket serializing {:?}", time_serializing.elapsed());
    }
}

pub fn since_the_epoch() -> Duration {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap()
}
