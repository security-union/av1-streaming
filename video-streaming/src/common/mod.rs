use crate::types::oculus_controller_state::OculusControllerState;

const DC_MOTOR_MAX_DUTY_CYCLE: f32 = 75f32;

pub fn compute_h_bridge_input_signals(state: OculusControllerState) -> (i16, i16) {
    let y = (state.get_primary_thumbstick().get_y() * DC_MOTOR_MAX_DUTY_CYCLE) as i16; // -1 ... 1
    if y > 0i16 {
        // forward
        (y, 0)
    } else {
        // back
        (0, y.abs())
    }
}
