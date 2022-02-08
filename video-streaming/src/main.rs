use nokhwa::{Camera, CameraFormat, FrameFormat};
use rav1e::config::SpeedSettings;
use rav1e::*;

fn main() {
    let mut enc = EncoderConfig::default();

    enc.width = 64;
    enc.height = 96;

    enc.speed_settings = SpeedSettings::from_preset(9);

    let cfg = Config::new().with_encoder_config(enc);

    let mut ctx: Context<u16> = cfg.new_context().unwrap();

    // set up the Camera
    let mut camera = Camera::new(
        0,                                                             // index
        Some(CameraFormat::new_from(640, 480, FrameFormat::YUYV, 30)), // format
    )
    .unwrap();
    // open stream
    camera.open_stream().unwrap();
    loop {
        let mut encoding_frame = ctx.new_frame();
        let frame = camera.frame().unwrap();
        let flat_samples = frame.as_flat_samples();
        for p in &mut encoding_frame.planes {
            let stride = (enc.width + p.cfg.xdec) >> p.cfg.xdec;
            p.copy_from_raw_u8(flat_samples.samples, stride, 1);
        }
        match ctx.send_frame(encoding_frame.clone()) {
            Ok(_) => {}
            Err(e) => match e {
                EncoderStatus::EnoughData => {
                    println!("Unable to append frame to the internal queue");
                }
                _ => {
                    panic!("Unable to send frame");
                }
            },
        }

        match ctx.receive_packet() {
            Ok(pkt) => {
                println!("Packet {}", pkt.input_frameno);
            }
            Err(e) => match e {
                EncoderStatus::LimitReached => {
                    println!("Limit reached");
                    break;
                }
                EncoderStatus::Encoded => println!("  Encoded"),
                EncoderStatus::NeedMoreData => println!("  Need more data"),
                _ => {
                    panic!("Unable to receive packet");
                }
            },
        }
    }
}
