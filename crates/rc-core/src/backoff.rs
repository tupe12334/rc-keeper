//! Restart delays: 2 s, doubling, capped at 300 s. A run of 10 min or more resets it.
use std::time::Duration;

pub const INITIAL: Duration = Duration::from_secs(2);
pub const MAX: Duration = Duration::from_secs(300);
pub const HEALTHY_AFTER: Duration = Duration::from_secs(600);

#[derive(Debug)]
pub struct Backoff {
    next: Duration,
}

impl Default for Backoff {
    fn default() -> Self {
        Self { next: INITIAL }
    }
}

impl Backoff {
    /// Returns the delay before the next start. `ran_for` is how long the last run lasted.
    pub fn next_delay(&mut self, ran_for: Duration) -> Duration {
        if ran_for >= HEALTHY_AFTER {
            self.next = INITIAL;
        }
        let delay = self.next;
        self.next = (self.next * 2).min(MAX);
        delay
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doubles_then_caps() {
        let mut b = Backoff::default();
        let quick = Duration::from_secs(1);
        let delays: Vec<u64> = (0..9).map(|_| b.next_delay(quick).as_secs()).collect();
        assert_eq!(delays, [2, 4, 8, 16, 32, 64, 128, 256, 300]);
    }

    #[test]
    fn healthy_run_resets_to_initial() {
        let mut b = Backoff::default();
        b.next_delay(Duration::from_secs(1));
        b.next_delay(Duration::from_secs(1));
        assert_eq!(b.next_delay(HEALTHY_AFTER), INITIAL);
        assert_eq!(b.next_delay(Duration::ZERO), INITIAL * 2);
    }
}
