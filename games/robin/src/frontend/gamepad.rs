//! A gamepad, read as a joystick in the Kempston's bit order, as
//! starquake-recompiled reads one (`REUSED.md`). In play it goes through
//! whichever method the menu chose (#84). The D-pad and the left stick
//! move, and any face button fires.
//! Start presses 0, which starts a game from the menu. A pad paired over
//! Bluetooth arrives as any other, and one plugged in turns up at the next
//! poll.

/// How far a stick must move before it counts as a direction.
const DEADZONE: f32 = 0.5;

/// What the pads are asking for this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Pad {
    /// The Kempston bits: 0 right, 1 left, 2 down, 3 up, 4 fire.
    pub kempston: u8,
    /// Start is held.
    pub start: bool,
}

pub struct Gamepad {
    gilrs: Option<gilrs::Gilrs>,
}

impl Gamepad {
    pub fn new() -> Gamepad {
        match gilrs::Gilrs::new() {
            Ok(gilrs) => Gamepad { gilrs: Some(gilrs) },
            Err(e) => {
                eprintln!("no gamepad support: {e}");
                Gamepad { gilrs: None }
            }
        }
    }

    /// What every connected pad together is asking for.
    pub fn poll(&mut self) -> Pad {
        let Some(gilrs) = &mut self.gilrs else {
            return Pad::default();
        };
        // The events feed the state, so they're drained first; this is also
        // where a pad just plugged in arrives.
        while gilrs.next_event().is_some() {}
        let mut pad = Pad::default();
        for (_, p) in gilrs.gamepads() {
            use gilrs::{Axis, Button};
            let (x, y) = (p.value(Axis::LeftStickX), p.value(Axis::LeftStickY));
            pad.kempston |= bits(
                p.is_pressed(Button::DPadRight) || x > DEADZONE,
                p.is_pressed(Button::DPadLeft) || x < -DEADZONE,
                p.is_pressed(Button::DPadDown) || y < -DEADZONE,
                p.is_pressed(Button::DPadUp) || y > DEADZONE,
                [Button::South, Button::East, Button::West, Button::North]
                    .into_iter()
                    .any(|b| p.is_pressed(b)),
            );
            pad.start |= p.is_pressed(Button::Start);
        }
        pad
    }
}

/// The Kempston port's bits for the directions and fire.
fn bits(right: bool, left: bool, down: bool, up: bool, fire: bool) -> u8 {
    u8::from(right)
        | u8::from(left) << 1
        | u8::from(down) << 2
        | u8::from(up) << 3
        | u8::from(fire) << 4
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directions_and_fire_land_on_the_kempston_bits() {
        assert_eq!(bits(true, false, false, false, false), 0x01);
        assert_eq!(bits(false, true, false, true, false), 0x0A);
        assert_eq!(bits(false, false, true, false, true), 0x14);
    }
}
