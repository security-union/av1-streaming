// gpio_servo_softpwm.rs - Rotates a servo using software-based PWM.
//
// Calibrate your servo beforehand, and change the values listed below to fall
// within your servo's safe limits to prevent potential damage. Don't power the
// servo directly from the Pi's GPIO header. Current spikes during power-up and
// stalls could otherwise damage your Pi, or cause your Pi to spontaneously
// reboot, corrupting your microSD card. If you're powering the servo using a
// separate power supply, remember to connect the grounds of the Pi and the
// power supply together.
//
// Software-based PWM is inherently inaccurate on a multi-threaded OS due to
// scheduling/preemption. If an accurate or faster PWM signal is required, use
// the hardware PWM peripheral instead. Check out the pwm_servo.rs example to
// learn how to control a servo using hardware PWM.

use std::{error::Error, env};
use futures_util::StreamExt;
use log::{info, debug};
use tokio::sync::mpsc::{channel, Sender, Receiver};
use video_streaming::types::oculus_controller_state::{OculusControllerState, OculusControllerState_Vector2};

use std::thread;
use std::time::Duration;
use warp::{
    ws::{Message, WebSocket},
    Filter,
};

use rppal::gpio::Gpio;

// Gpio uses BCM pin numbering. BCM GPIO 23 is tied to physical pin 16.
const GPIO_PWM: u8 = 23;

// Servo configuration. Change these values based on your servo's verified safe
// minimum and maximum values.
//
const PERIOD_MS: u64 = 250;
const PULSE_MIN_US: u64 = 500;
const PULSE_NEUTRAL_US: u64 = 750;
const PULSE_MAX_US: u64 = 1000;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let port: u16 = env::var("PORT")
    .ok()
    .map(|n| n.parse::<u16>().ok())
    .flatten()
    .unwrap_or(9080u16);
    env_logger::init();
    // Retrieve the GPIO pin and configure it as an output.
    let mut pin = Gpio::new()?.get(GPIO_PWM)?.into_output();

    // Enable software-based PWM with the specified period, and rotate the servo by
    // setting the pulse width to its maximum value.
    pin.set_pwm(
        Duration::from_millis(PERIOD_MS),
        Duration::from_micros(PULSE_NEUTRAL_US),
    )?;

    // Sleep for 500 ms while the servo moves into position.
    thread::sleep(Duration::from_millis(500));

    let (tx, mut rx): (Sender<OculusControllerState>, Receiver<OculusControllerState>) = channel(1);
    let add_tx = warp::any().map(move || tx.clone());

    let receiver = tokio::spawn(async move {
        while let Some(state) = rx.recv().await {
            let stick = state.secondary_thumbstick.get_ref().x;
            let transformed_value =
            (PULSE_MIN_US as f32) + ((stick + 1f32) * 50f32 * ((PULSE_MAX_US - PULSE_MIN_US) as f32) / 100f32);
            info!("original {:?}", state);
            info!("transformed {:?}", transformed_value);
            pin.set_pwm(
                Duration::from_millis(PERIOD_MS),
                Duration::from_micros((transformed_value as u64)),
            );
        }
    });
    let routes = warp::path("ws")
    // The `ws()` filter will prepare the Websocket handshake.
    .and(warp::ws())
    .and(add_tx)
    .map(
        |ws: warp::ws::Ws, tx: Sender<OculusControllerState>| {
            debug!("before creating upgrade");
            // And then our closure will be called when it completes...
            debug!("adding client connection");
            ws.on_upgrade(|ws| client_connection(ws, tx))
        },
    );
    // WebSocker server thread
    warp::serve(routes).run(([0, 0, 0, 0], port)).await;
    receiver.await;

    Ok(())

    // When the pin variable goes out of scope, software-based PWM is automatically disabled.
    // You can manually disable PWM by calling the clear_pwm() method.
}

async fn client_connection(ws: WebSocket, tx: Sender<OculusControllerState>) {
    info!("establishing client connection... {:?}", ws);
    let (_client_ws_sender, mut client_ws_rcv) = ws.split();

    let handle = tokio::task::spawn(async move {
        while let Some(message) = client_ws_rcv.next().await {
            let msg = message
                .ok()
                .map(|msg| {
                    let packet: Result<OculusControllerState, protobuf::ProtobufError> =
                        protobuf::Message::parse_from_bytes(msg.as_bytes());
                    packet.ok()
                })
                .flatten();
            match msg {
                Some(oculus) => {
                    info!("got message {:?}", oculus);
                    tx.send(oculus); 
                },
                None => info!("unable to parse message")
            }
        }
    });
    handle.await;
}