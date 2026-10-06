//! The keys a trace configuration presses, frame by frame: its scripted
//! inputs and its random held keys. Shared by the tracer and by anything
//! else that must play the same game, such as a game's verifier.

use crate::config;
use zx_runtime::Zx;
use zx_runtime::keys::Key;

fn keys(names: &[String]) -> Result<Vec<Key>, String> {
    names
        .iter()
        .map(|n| Key::by_name(n).ok_or_else(|| format!("unknown key name {n:?} in trace config")))
        .collect()
}

/// xorshift64*; deterministic so builds are reproducible.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
}

/// A trace configuration's input, ready to play.
pub struct Script {
    scripted: Vec<(u32, u32, Vec<Key>)>,
    random: Option<(u32, u32, Vec<Key>)>,
    rng: Rng,
    held_random: Vec<Key>,
}

impl Script {
    /// # Errors
    ///
    /// If the configuration names a key the Spectrum does not have.
    pub fn new(cfg: &config::Trace) -> Result<Script, String> {
        let scripted = cfg
            .input
            .iter()
            .map(|e| Ok((e.at, e.hold, keys(&e.keys)?)))
            .collect::<Result<_, String>>()?;
        let random = match &cfg.random {
            Some(r) => Some((r.start, r.every, keys(&r.keys)?)),
            None => None,
        };
        Ok(Script {
            scripted,
            rng: Rng(cfg.random.as_ref().map_or(1, |r| r.seed) | 1),
            random,
            held_random: Vec::new(),
        })
    }

    /// Sets the keys held during `frame`. Called once a frame, in order from
    /// frame 0, before the frame runs.
    pub fn press(&mut self, frame: u32, z: &mut Zx) {
        z.release_all_keys();
        for (at, hold, keys) in &self.scripted {
            if frame >= *at && frame < at.saturating_add(*hold) {
                for &k in keys {
                    z.set_key(k, true);
                }
            }
        }
        if let Some((start, every, choices)) = &self.random
            && frame >= *start
            && !choices.is_empty()
        {
            if (frame - start).is_multiple_of((*every).max(1)) {
                self.held_random.clear();
                for _ in 0..(self.rng.next() % 3) {
                    let k = choices[(self.rng.next() % choices.len() as u64) as usize];
                    self.held_random.push(k);
                }
            }
            for &k in &self.held_random {
                z.set_key(k, true);
            }
        }
    }
}
