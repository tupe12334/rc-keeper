//! Takes a folder away from a foreign `claude remote-control` (CONTRACT.md v4). Scans with
//! `foreign`, then stops the victims with `procs::terminate`.
use std::collections::VecDeque;
use std::path::Path;
use std::time::{Duration, Instant};

use crate::foreign::{self, Victim};
use crate::procs;

/// How long a foreign process gets to exit after SIGTERM before SIGKILL.
pub const GRACE: Duration = Duration::from_secs(5);
/// Most takeovers one supervisor makes inside `WINDOW`. Past that, the folder keeps coming back.
pub const MAX_TAKEOVERS: usize = 3;
pub const WINDOW: Duration = Duration::from_secs(10 * 60);

/// The result of a preview or a takeover.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Takeover {
    /// Pids that were stopped, or that a takeover would stop.
    pub stopped: Vec<u32>,
    /// The supervisor that may start the stopped process again, if one was found.
    pub respawn_risk: Option<String>,
}

/// Counts takeovers inside a sliding window, so a respawned rc cannot be killed forever.
#[derive(Debug, Default)]
pub struct Budget {
    taken: VecDeque<Instant>,
}

impl Budget {
    /// Records a takeover at `now` and returns true. Returns false when the window is full.
    pub fn allow(&mut self, now: Instant) -> bool {
        while self
            .taken
            .front()
            .is_some_and(|&t| now.saturating_duration_since(t) > WINDOW)
        {
            self.taken.pop_front();
        }
        if self.taken.len() >= MAX_TAKEOVERS {
            return false;
        }
        self.taken.push_back(now);
        true
    }
}

/// Lists what `take_over` would stop. Stops nothing.
pub fn preview(folder: &Path) -> Takeover {
    scan(folder).0
}

/// Stops the foreign rc processes in `folder` and their session children. Each pid is
/// re-verified against a fresh process table right before any signal.
pub fn take_over(folder: &Path) -> Takeover {
    let (taken, victims) = scan(folder);
    procs::terminate(&victims, GRACE, |victim| {
        foreign::same_process(&foreign::snapshot(), victim)
    });
    taken
}

fn scan(folder: &Path) -> (Takeover, Vec<Victim>) {
    let plan = foreign::plan(&foreign::snapshot(), folder, std::process::id());
    let taken = Takeover {
        stopped: plan.victims.iter().map(|v| v.pid).collect(),
        respawn_risk: plan.respawn_risk,
    };
    (taken, plan.victims)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn respawn_loop_stops_after_the_limit() {
        let mut budget = Budget::default();
        let t0 = Instant::now();
        assert!(budget.allow(t0));
        assert!(budget.allow(t0 + Duration::from_secs(1)));
        assert!(budget.allow(t0 + Duration::from_secs(2)));
        // The fourth takeover inside the window is refused.
        assert!(!budget.allow(t0 + Duration::from_secs(3)));
        // Once the oldest falls out of the window, a takeover is allowed again.
        assert!(budget.allow(t0 + WINDOW + Duration::from_secs(1)));
    }
}
