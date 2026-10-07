//! Robin's movement (`docs/re/robin.md`, *Robin's movement*): the
//! controls, walking, walls and the screen's edge.

use crate::controls::Controls;
use crate::game::Game;

/// The control methods, by the address of their code (`0xD152`).
pub const REDEFINED_KEYS: u16 = 0xD06E;
pub const KEMPSTON: u16 = 0xD07F;
pub const SINCLAIR: u16 = 0xD088;

/// The Z80's flag-free rotations through carry, as the original's code
/// uses them.
struct Carry(bool);

impl Carry {
    /// `RRA`.
    fn rra(&mut self, a: u8) -> u8 {
        let out = a & 1 != 0;
        let a = (a >> 1) | if self.0 { 0x80 } else { 0 };
        self.0 = out;
        a
    }
    /// `RLA` and `RL r`.
    fn rl(&mut self, a: u8) -> u8 {
        let out = a & 0x80 != 0;
        let a = (a << 1) | u8::from(self.0);
        self.0 = out;
        a
    }
    /// `RRCA`.
    fn rrca(&mut self, a: u8) -> u8 {
        self.0 = a & 1 != 0;
        a.rotate_right(1)
    }
}

/// Whether the key named by `code` is pressed (`0:D053`): its half-row in
/// the low three bits, its place in it in the rest. Returns the carry as the
/// original leaves it: clear when pressed.
fn key_up(controls: &Controls, code: u8, carry: &mut Carry) {
    let rotations = (code & 7) + 1;
    let shift = 5u8.wrapping_sub(code >> 3);
    carry.0 = (code >> 3) > 5;
    let mut half = 0xFE;
    for _ in 0..rotations {
        half = carry.rrca(half);
    }
    let mut a = controls.read(u16::from(half) << 8 | 0xFE);
    for _ in 0..crate::times(shift) {
        a = carry.rra(a);
    }
}

/// The redefined keys (`0xD06E`): fire, up, down, left and right, each
/// read in turn into bits 4 to 0.
#[must_use]
pub fn read_keys(g: &Game, controls: &Controls) -> u8 {
    let mut e: u8 = 8;
    let mut carry = Carry(false);
    for &code in &g.robin.keys {
        key_up(controls, code, &mut carry);
        carry.0 = !carry.0;
        e = carry.rl(e);
    }
    e
}

/// The Kempston joystick (`0xD07F`): its port's bits, already in order.
#[must_use]
pub fn read_kempston(controls: &Controls) -> u8 {
    controls.read(0x001F) & 0x1F
}

/// The Sinclair joystick (`0xD088`): the keys 0, 9, 8, 6 and 7, as fire,
/// up, down, left and right.
#[must_use]
pub fn read_sinclair(controls: &Controls) -> u8 {
    let mut a = !controls.read(0xEFFE);
    let mut c = Carry(false);
    a = c.rra(a);
    let mut e = c.rl(0);
    a = c.rra(a);
    e = c.rl(e);
    a = c.rra(a);
    e = c.rl(e);
    a = c.rra(a);
    a = c.rra(a);
    e = c.rl(e);
    c.rl(a);
    c.rl(e)
}

/// What the player is pressing, through the chosen method, with the code's
/// own override applied and opposite directions cancelled (`0:D0C6`).
///
/// # Panics
///
/// If the override bytes are other than the two the game writes there:
/// `NOP NOP`, or `LD E,n`.
#[must_use]
pub fn read_controls(g: &Game, controls: &Controls) -> u8 {
    let mut e = match g.robin.method {
        REDEFINED_KEYS => read_keys(g, controls),
        KEMPSTON => read_kempston(controls),
        SINCLAIR => read_sinclair(controls),
        other => panic!("a control method at {other:#06x}, which the game never chooses"),
    };
    match g.robin.override_controls {
        [0, 0] => {}
        [0x1E, n] => e = n,
        other => panic!("override bytes {other:02x?}, which the game never writes"),
    }
    if e & 3 == 3 {
        e &= !3;
    }
    if e & 0x0C == 0x0C {
        e &= !0x0C;
    }
    e
}
