//! Holds off a respawn after a network exit until the API host resolves again. Otherwise rc
//! restarts during a DNS outage, fails the same way, and each start makes a new session.
use std::net::ToSocketAddrs;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::{Duration, Instant};

const HOST: &str = "api.anthropic.com";
/// How often the host is looked up while waiting.
pub const CHECK_EVERY: Duration = Duration::from_secs(10);
/// A lookup that takes longer counts as not resolved. The OS DNS timeout can be much longer.
const LOOKUP_TIMEOUT: Duration = Duration::from_secs(2);
/// Stop is checked this often, like the backoff wait does.
const POLL: Duration = Duration::from_millis(100);
/// Prefixes that the parser puts on network errors in `last_error`.
const NETWORK_ERRORS: [&str; 2] = ["No network:", "Network error:"];

/// Answers whether a host name resolves. A trait so tests can fake DNS.
pub trait Resolver: Send + Sync {
    fn resolves(&self, host: &str) -> bool;
}

/// Uses the system resolver on port 443.
pub struct SystemResolver;

impl Resolver for SystemResolver {
    fn resolves(&self, host: &str) -> bool {
        (host, 443)
            .to_socket_addrs()
            .is_ok_and(|mut addrs| addrs.next().is_some())
    }
}

/// Starts one lookup on a helper thread. The result comes back through the receiver.
fn spawn_lookup(resolver: &Arc<dyn Resolver>) -> mpsc::Receiver<bool> {
    let (tx, rx) = mpsc::channel();
    let resolver = Arc::clone(resolver);
    thread::spawn(move || {
        let _ = tx.send(resolver.resolves(HOST));
    });
    rx
}

enum Awaited {
    Stop,
    /// The lookup returned, or its thread died.
    Done(bool),
    /// Still running after `LOOKUP_TIMEOUT`. The receiver is kept for the next check.
    Pending,
}

/// Waits up to `LOOKUP_TIMEOUT` for a lookup, checking stop every `POLL`.
fn await_lookup(rx: &mpsc::Receiver<bool>, stop: &AtomicBool) -> Awaited {
    let deadline = Instant::now() + LOOKUP_TIMEOUT;
    loop {
        if stop.load(Ordering::SeqCst) {
            return Awaited::Stop;
        }
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Awaited::Pending;
        }
        match rx.recv_timeout(POLL.min(left)) {
            Ok(resolved) => return Awaited::Done(resolved),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return Awaited::Done(false),
        }
    }
}

/// After a network exit, waits until the API host resolves. Returns false when stop arrives
/// first. Any other exit returns true at once and does not look up the host.
///
/// At most one lookup thread exists at a time. A lookup still pending at a check is polled
/// again at the next check, and no new thread starts until it returns.
pub fn wait_after_exit(
    last_error: Option<&str>,
    stop: &AtomicBool,
    resolver: Arc<dyn Resolver>,
    check_every: Duration,
) -> bool {
    if !last_error.is_some_and(|e| NETWORK_ERRORS.iter().any(|p| e.starts_with(p))) {
        return true;
    }
    let mut pending: Option<mpsc::Receiver<bool>> = None;
    let mut next_check = Instant::now();
    loop {
        if stop.load(Ordering::SeqCst) {
            return false;
        }
        if Instant::now() >= next_check {
            let rx = pending.take().unwrap_or_else(|| spawn_lookup(&resolver));
            match await_lookup(&rx, stop) {
                Awaited::Stop => return false,
                Awaited::Done(true) => return true,
                Awaited::Done(false) => {}
                Awaited::Pending => pending = Some(rx),
            }
            next_check = Instant::now() + check_every;
        }
        thread::sleep(POLL.min(check_every));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU32;

    const NETWORK: Option<&str> = Some(
        "No network: DNS lookup failed. Check the internet connection. (Error: getaddrinfo ENOTFOUND api.anthropic.com)",
    );
    const FAST: Duration = Duration::from_millis(1);

    /// Fails until the `succeed_on`-th lookup.
    struct FakeDns {
        calls: AtomicU32,
        succeed_on: u32,
    }

    impl Resolver for FakeDns {
        fn resolves(&self, _host: &str) -> bool {
            let n = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
            n >= self.succeed_on
        }
    }

    /// Never resolves, and sets stop on the first lookup.
    struct StopOnLookup {
        stop: Arc<AtomicBool>,
        calls: AtomicU32,
    }

    impl Resolver for StopOnLookup {
        fn resolves(&self, _host: &str) -> bool {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.stop.store(true, Ordering::SeqCst);
            false
        }
    }

    /// Blocks for `hold` on every lookup, like a DNS server that does not answer.
    struct BlockingDns {
        hold: Duration,
    }

    impl Resolver for BlockingDns {
        fn resolves(&self, _host: &str) -> bool {
            thread::sleep(self.hold);
            false
        }
    }

    #[test]
    fn wait_returns_as_soon_as_dns_resolves() {
        let stop = AtomicBool::new(false);
        let dns = Arc::new(FakeDns {
            calls: AtomicU32::new(0),
            succeed_on: 3,
        });
        assert!(wait_after_exit(NETWORK, &stop, dns.clone(), FAST));
        assert_eq!(dns.calls.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn stop_during_wait_returns_false() {
        let stop = Arc::new(AtomicBool::new(false));
        let dns = Arc::new(StopOnLookup {
            stop: Arc::clone(&stop),
            calls: AtomicU32::new(0),
        });
        assert!(!wait_after_exit(NETWORK, &stop, dns.clone(), FAST));
        assert_eq!(dns.calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn stop_returns_fast_while_a_lookup_blocks() {
        let stop = Arc::new(AtomicBool::new(false));
        let setter = {
            let stop = Arc::clone(&stop);
            thread::spawn(move || {
                thread::sleep(Duration::from_millis(100));
                stop.store(true, Ordering::SeqCst);
            })
        };
        let dns = Arc::new(BlockingDns {
            hold: Duration::from_secs(5),
        });
        let started = Instant::now();
        assert!(!wait_after_exit(NETWORK, &stop, dns, FAST));
        assert!(started.elapsed() < Duration::from_secs(1));
        setter.join().unwrap();
    }

    /// Blocks every lookup until `gate` opens, and counts the calls.
    struct GatedDns {
        gate: Arc<AtomicBool>,
        calls: AtomicU32,
    }

    impl Resolver for GatedDns {
        fn resolves(&self, _host: &str) -> bool {
            self.calls.fetch_add(1, Ordering::SeqCst);
            while !self.gate.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_millis(10));
            }
            false
        }
    }

    #[test]
    fn blocked_lookup_is_not_respawned() {
        let stop = Arc::new(AtomicBool::new(false));
        let gate = Arc::new(AtomicBool::new(false));
        let dns = Arc::new(GatedDns {
            gate: Arc::clone(&gate),
            calls: AtomicU32::new(0),
        });
        let waiter = {
            let (stop, dns) = (Arc::clone(&stop), dns.clone());
            thread::spawn(move || wait_after_exit(NETWORK, &stop, dns, FAST))
        };
        // Several checks pass while the first lookup is still blocked.
        for _ in 0..3 {
            thread::sleep(Duration::from_millis(1500));
            assert_eq!(dns.calls.load(Ordering::SeqCst), 1);
        }
        gate.store(true, Ordering::SeqCst);
        stop.store(true, Ordering::SeqCst);
        assert!(!waiter.join().unwrap());
    }

    #[test]
    fn non_network_exit_does_not_wait() {
        let stop = Arc::new(AtomicBool::new(false));
        let dns = Arc::new(StopOnLookup {
            stop: Arc::clone(&stop),
            calls: AtomicU32::new(0),
        });
        assert!(wait_after_exit(
            Some("Error: bad flag --foo"),
            &stop,
            dns.clone(),
            FAST
        ));
        assert!(wait_after_exit(None, &stop, dns.clone(), FAST));
        assert_eq!(dns.calls.load(Ordering::SeqCst), 0);
    }
}
