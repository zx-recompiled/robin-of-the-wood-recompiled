//! `Zx::step_in_frame` runs frames exactly as `Zx::run_frame` does, so a
//! caller can stop between any two instructions without changing what runs.

use zx_runtime::{Misses, Zx, loader::power_on_128k, no_code};

/// A 128K with no ROM running a program that takes IM 2 interrupts and
/// halts: the main loop counts in HL and stores it, then halts; the handler
/// counts in A' and returns.
fn machine() -> Zx {
    let mut z = power_on_128k(&[0u8; 0x8000]);
    z.memory.page_128k(0x10);
    let mut poke = |at: u16, bytes: &[u8]| {
        for (i, &b) in bytes.iter().enumerate() {
            z.memory.poke(at + i as u16, b);
        }
    };
    // The vector at I * 0x100 + 0xFF points to the handler at 0x9000.
    poke(0x80FF, &[0x00, 0x90]);
    // EX AF,AF' ; INC A ; EX AF,AF' ; EI ; RETI
    poke(0x9000, &[0x08, 0x3C, 0x08, 0xFB, 0xED, 0x4D]);
    // EI ; loop: INC HL ; LD (0x9100),HL ; LD B,200 ; DJNZ $ ; HALT ; JR loop
    poke(
        0x8200,
        &[
            0xFB, 0x23, 0x22, 0x00, 0x91, 0x06, 0xC8, 0x10, 0xFE, 0x76, 0x18, 0xF5,
        ],
    );
    (z.pc, z.sp, z.i, z.im) = (0x8200, 0xBF00, 0x80, 2);
    z
}

fn state(z: &Zx) -> (u16, u16, u16, u8, u32, u64, u16) {
    (z.pc, z.sp, z.hl(), z.a_, z.t, z.frame, z.read16(0x9100))
}

#[test]
fn stepping_a_frame_at_a_time_is_running_it() {
    let (mut framed, mut stepped) = (machine(), machine());
    // Both at the start of a frame, which is when the interrupt is raised.
    stepped.int_pending = true;
    let mut misses = Misses::default();
    for f in 1..=20u64 {
        framed.run_frame(no_code, &mut misses);
        // Step until the frame is over: the next step would begin frame f.
        while !(stepped.frame == f - 1 && stepped.t >= stepped.timing.frame) {
            stepped.step_in_frame();
        }
        stepped.t -= stepped.timing.frame;
        stepped.frame += 1;
        assert_eq!(state(&stepped), state(&framed), "after frame {f}");
        // Hand the rollover back, as step_in_frame would do it itself.
        stepped.t += stepped.timing.frame;
        stepped.frame -= 1;
    }
    assert!(
        framed.a_ >= 19,
        "the handler ran every frame: {}",
        framed.a_
    );
    assert!(framed.read16(0x9100) >= 19);
}
