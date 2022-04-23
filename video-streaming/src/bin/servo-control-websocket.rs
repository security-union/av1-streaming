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

use std::error::Error;
use log::info;
use tokio::sync::mpsc::channel;
use video_streaming::types::oculus_controller_state::{OculusControllerState, OculusControllerState_Vector2};
use std::thread;
use std::time::Duration;
use tokio::time::{sleep};

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

    let (tx, mut rx) = channel(1);

    let sender = tokio::spawn(async move {
        loop {
            for pulse in (-10..=10).step_by(1) {
                let mut state: OculusControllerState = OculusControllerState::default();
                state.mut_secondary_thumbstick().set_x((pulse as f32) / 10f32);
                tx.send(state).await;
                tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
            }
        }
    });

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

    sender.await;
    receiver.await;



    // let routes = warp::path("ws")
    // // The `ws()` filter will prepare the Websocket handshake.
    // .and(warp::ws())
    // .and(add_bus)
    // .and(add_counter)
    // .map(
    //     |ws: warp::ws::Ws, bus: Arc<Mutex<Bus<Vec<u8>>>>, counter: Arc<Mutex<u16>>| {
    //         debug!("before creating upgrade");
    //         // And then our closure will be called when it completes...
    //         let reader = bus.lock().unwrap().add_rx();
    //         let counter_copy = counter.clone();
    //         debug!("adding client connection");
    //         ws.on_upgrade(|ws| client_connection(ws, reader, counter_copy))
    //     },
    // );
    // // WebSocker server thread
    // warp::serve(routes).run(([0, 0, 0, 0], port)).await;

    Ok(())

    // When the pin variable goes out of scope, software-based PWM is automatically disabled.
    // You can manually disable PWM by calling the clear_pwm() method.
}