#[macro_use]
extern crate log;

use anyhow::Result;
use base64::encode;
use bus::{Bus, BusReader};
use futures_util::{SinkExt, StreamExt};
use image::codecs;
use image::ImageBuffer;
use image::Rgb;
use ndarray::prelude::*;
use nokhwa::{Camera, CameraFormat, CaptureAPIBackend, FrameFormat};
use rav1e::prelude::ChromaSampling;
use rav1e::*;
use rav1e::{config::SpeedSettings, prelude::FrameType};
use serde::{Deserialize, Serialize};
use serde_json;
use tensorflow::Graph;
use tensorflow::ImportGraphDefOptions;
use tensorflow::Operation;
use tensorflow::Session;
use tensorflow::SessionOptions;
use tensorflow::SessionRunArgs;
use tensorflow::Tensor;
use std::collections::HashMap;
use std::str::FromStr;
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

static THRESHOLD_MILLIS: u128 = 1000;

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

    let bus: Arc<Mutex<Bus<String>>> = Arc::new(Mutex::new(bus::Bus::new(10)));
    let bus_copy = bus.clone();
    let add_bus = warp::any().map(move || bus.clone());

    let client_counter: Arc<Mutex<u16>> = Arc::new(Mutex::new(0));
    let web_socket_counter = client_counter.clone();

    // Add counter to warp so that we can access it when we add/remove connections
    let add_counter = warp::any().map(move || web_socket_counter.clone());

    let cfg = Config::new().with_encoder_config(enc).with_threads(4);

    let (fps_tx, fps_rx): (Sender<u128>, Receiver<u128>) = mpsc::channel();
    let (cam_tx, cam_rx): (
        Sender<(ImageBuffer<Rgb<u8>, Vec<u8>>, u128)>,
        Receiver<(ImageBuffer<Rgb<u8>, Vec<u8>>, u128)>,
    ) = mpsc::channel();

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
            let fps_tx_copy = fps_tx.clone();
            let mut ctx: Context<u8> = cfg.new_context().unwrap();
            loop {
                let (mut frame, age) = cam_rx.recv().unwrap();
                // If age older than threshold, throw it away.
                let frame_age = since_the_epoch().as_millis() - age;
                debug!("frame age {}", frame_age);
                if frame_age > THRESHOLD_MILLIS {
                    debug!("throwing away old frame with age {} ms", frame_age);
                    continue;
                }
                if encoder == Encoder::MJPEG {
                    let mut buf: Vec<u8> = Vec::new();
                    let mut jpeg_encoder =
                        codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, 80);
                    jpeg_encoder
                        .encode_image(&frame)
                        .map_err(|e| error!("{:?}", e));
                    let frame = VideoPacket {
                        data: Some(encode(&buf)),
                        frameType: None,
                        epochTime: since_the_epoch(),
                        encoding: encoder.clone(),
                    };
                    let json = serde_json::to_string(&frame).unwrap();
                    bus_copy.lock().unwrap().broadcast(json);
                    fps_tx_copy.send(since_the_epoch().as_millis()).unwrap();
                    continue;
                }
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
                            frameType: Some(frame_type.to_string()),
                            epochTime: since_the_epoch(),
                            encoding: encoder.clone(),
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
    // WebSocker server thread
    warp::serve(routes).run(([0, 0, 0, 0], 8080)).await;
    encoder_thread.join().unwrap();
    fps_thread.join().unwrap();
    camera_thread.join().unwrap();
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

pub struct ObjectDetector {
    graph: Graph,
    session: Session,
    image: Box<dyn DetectionImage + Send>
}

impl ObjectDetector {
    ///Initialize the tensorflow session with the open images ssd mobilenet v2
    pub fn new() -> Self {
        let mut graph = Graph::new();
        let proto = include_bytes!("../models/openimages_v4_ssd_mobilenet_v2_1/model.pb");
        graph.import_graph_def(proto, &ImportGraphDefOptions::new()).unwrap();
        let session = Session::new(&SessionOptions::new(), &graph).unwrap();
        ObjectDetector {
            graph,
            session,
            image: Box::new(GenericImage::default())
        }
    }

    ///Pass in the input image to be inferenced on. Input must implement the DetectionImage trait
    pub fn input<I: DetectionImage + 'static>(&mut self, image: I)
    where
        I: std::marker::Send,
    {
        self.image = Box::new(image);
    }

    fn input_transform(&self) -> Result<(Operation, Tensor<u8>), Box<dyn std::error::Error>> {
        let image_dimension = self.image.dimension();
        let image_array = Array::from_shape_vec(
            (
                image_dimension.height as usize,
                image_dimension.width as usize,
                3,
            ),
            self.image.pixel_buffer().to_vec(),
        )?;
        let image_array_expanded = image_array.insert_axis(Axis(0));

        let image_tensor_op = self.graph.operation_by_name_required("image_tensor")?;
        let input_image_tensor = Tensor::new(&[
            1,
            u64::from(image_dimension.height),
            u64::from(image_dimension.width),
            3,
        ])
        .with_values(image_array_expanded.as_slice().unwrap())?;

        Ok((image_tensor_op, input_image_tensor))
    }

    ///Run the inference on the inputted image transforming the image to the shape of the image input tensor,
    ///performing the inferencing, and mapping the output tensors into the returned HashMap.
    pub fn run(&mut self) -> Result<HashMap<&str, Tensor<f32>>, Box<dyn std::error::Error>> {
        let (image_tensor_op, input_image_tensor) = self.input_transform()?;

        let mut session_args = SessionRunArgs::new();
        session_args.add_feed(&image_tensor_op, 0, &input_image_tensor);

        let num_detections = self.graph.operation_by_name_required("num_detections")?;
        let num_detections_token = session_args.request_fetch(&num_detections, 0);

        let classes = self.graph.operation_by_name_required("detection_classes")?;
        let classes_token = session_args.request_fetch(&classes, 0);

        let boxes = self.graph.operation_by_name_required("detection_boxes")?;
        let boxes_token = session_args.request_fetch(&boxes, 0);

        let scores = self.graph.operation_by_name_required("detection_scores")?;
        let scores_token = session_args.request_fetch(&scores, 0);

        self.session.run(&mut session_args)?;

        let num_detections_tensor = session_args.fetch::<f32>(num_detections_token)?;
        let classes_tensor = session_args.fetch::<f32>(classes_token)?;
        let boxes_tensor = session_args.fetch::<f32>(boxes_token)?;
        let scores_tensor = session_args.fetch::<f32>(scores_token)?;

        let mut tensor_map = HashMap::new();
        tensor_map.insert("num_detections", num_detections_tensor);
        tensor_map.insert("detection_classes", classes_tensor);
        tensor_map.insert("detection_boxes", boxes_tensor);
        tensor_map.insert("detection_scores", scores_tensor);

        Ok(tensor_map)
    }
}

use image::GenericImageView;

#[derive(Default, Clone)]
pub struct GenericImage {
    image_dimension: ImageDimension,
    pixel_buffer: Vec<u8>,
}

impl GenericImage {
    pub fn new(width: u32, height: u32, pixel_buffer: Vec<u8>) -> Self {
        GenericImage {
            image_dimension: ImageDimension { width, height },
            pixel_buffer,
        }
    }
}

#[derive(Clone, Copy, Default)]
pub struct ImageDimension {
    pub width: u32,
    pub height: u32,
}

pub trait DetectionImage {
    fn dimension(&self) -> ImageDimension;
    fn pixel_buffer(&self) -> Vec<u8>;
}

impl DetectionImage for GenericImage {
    fn dimension(&self) -> ImageDimension {
        self.image_dimension
    }

    fn pixel_buffer(&self) -> Vec<u8> {
        self.pixel_buffer.to_vec()
    }
}

impl DetectionImage for image::DynamicImage {
    fn dimension(&self) -> ImageDimension {
        let (width, height) = self.dimensions();
        ImageDimension { width, height }
    }

    fn pixel_buffer(&self) -> Vec<u8> {
        self.as_bytes().to_vec()
    }
}
