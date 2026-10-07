//! What a routine reads from outside the game's memory: the controls, and
//! random numbers.
//!
//! The original takes its random numbers from the refresh register R, which
//! counts the instructions the processor has fetched, so a rewrite cannot
//! reproduce them (`README.md`, *Status*; #37). In the checks, a routine is
//! given the very values the original read, in order. The game itself will
//! draw its own.

use crate::controls::Controls;

/// Random numbers, drawn in order.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Random {
    values: Vec<u8>,
    drawn: usize,
}

impl Random {
    /// The values a routine is to draw, in order: in the checks, those the
    /// original read from R.
    #[must_use]
    pub fn given(values: Vec<u8>) -> Random {
        Random { values, drawn: 0 }
    }

    /// The next value, as the original's `LD A,R` gives it.
    ///
    /// # Panics
    ///
    /// If more are drawn than were given: the rewrite read R where the
    /// original did not.
    pub fn r(&mut self) -> u8 {
        let v = *self.values.get(self.drawn).unwrap_or_else(|| {
            panic!(
                "drew random value {} where the original read {}",
                self.drawn + 1,
                self.values.len()
            )
        });
        self.drawn += 1;
        v
    }

    /// How many were given and not drawn.
    #[must_use]
    pub fn left(&self) -> usize {
        self.values.len() - self.drawn
    }
}

/// Everything a routine reads from outside the game's memory.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Inputs {
    pub controls: Controls,
    pub random: Random,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_are_drawn_in_order_and_no_more() {
        let mut r = Random::given(vec![3, 1]);
        assert_eq!((r.r(), r.left()), (3, 1));
        assert_eq!((r.r(), r.left()), (1, 0));
        assert!(std::panic::catch_unwind(move || r.r()).is_err());
    }
}
