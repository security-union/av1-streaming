use video_streaming::{types::oculus_controller_state::OculusControllerState, common::compute_h_bridge_input_signals};



#[test]
fn test_primary_controller_y_min() {
    let mut state = OculusControllerState::default();
    state.mut_primary_thumbstick().set_y(-1f32);
    let (in1, in2) = compute_h_bridge_input_signals(state);
    assert_eq!((in1, in2), (0, 75))
}

#[test]
fn test_primary_controller_y_max() {
    let mut state = OculusControllerState::default();
    state.mut_primary_thumbstick().set_y(1f32);
    let (in1, in2) = compute_h_bridge_input_signals(state);
    assert_eq!((in1, in2), (75, 0))
}

#[test]
fn test_primary_controller_y_zero() {
    let mut state = OculusControllerState::default();
    state.mut_primary_thumbstick().set_y(0f32);
    let (in1, in2) = compute_h_bridge_input_signals(state);
    assert_eq!((in1, in2), (0, 0))
}