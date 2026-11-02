//! Takes a folder away from a foreign `claude remote-control` (CONTRACT.md v4). Scans with
//! `foreign`, then stops the victims with `procs::terminate`.
use std::path::Path;
use std::time::Duration;

use crate::foreign;
use crate::procs;

/// How long a foreign process gets to exit after SIGTERM before SIGKILL.
pub const GRACE: Duration = Duration::from_secs(5);

/// The result of a preview or a takeover.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Takeover {
    /// Pids that were stopped, or that a takeover would stop.
    pub stopped: Vec<u32>,
    /// The supervisor that may start the stopped process again, if one was found.
    pub respawn_risk: Option<String>,
}

/// Lists what `take_over` would stop. Stops nothing.
pub fn preview(folder: &Path) -> Takeover {
    scan(folder).0
}

/// Stops the foreign rc processes in `folder` and their session children.
pub fn take_over(folder: &Path) -> Takeover {
    let (taken, victims) = scan(folder);
    procs::terminate(&victims, GRACE);
    taken
}

fn scan(folder: &Path) -> (Takeover, Vec<u32>) {
    let plan = foreign::plan(&foreign::snapshot(), folder, std::process::id());
    let taken = Takeover {
        stopped: plan.victims.clone(),
        respawn_risk: plan.respawn_risk,
    };
    (taken, plan.victims)
}
