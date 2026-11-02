//! Public entry point. Owns the instance registry, the supervisor threads and status.json.
use crate::config::{CONFIG_FILE, read_config, write_config, write_status};
use crate::locate::find_claude_bin;
use crate::model::{AppConfig, InstanceConfig, InstanceView, Settings, State};
use crate::procs;
use crate::supervisor::{self, Job, Shared};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::thread;
use std::time::Duration;

const TICK: Duration = Duration::from_secs(2);

/// Thread-safe handle to all instances. Cheap to clone.
#[derive(Clone)]
pub struct Manager {
    inner: Arc<Inner>,
}

struct Inner {
    dir: PathBuf,
    reg: Mutex<Registry>,
}

#[derive(Default)]
struct Registry {
    config: AppConfig,
    runs: HashMap<String, Arc<Shared>>,
}

impl Manager {
    /// Reads `config.json` from `config_dir` and starts the instances with `auto_start`.
    pub fn load(config_dir: PathBuf) -> Manager {
        if let Err(e) = fs::create_dir_all(&config_dir) {
            eprintln!("rc-core: cannot create {}: {e}", config_dir.display());
        }
        let mut config = read_config(&config_dir).unwrap_or_else(|e| {
            eprintln!("rc-core: {e}; moved to config.json.bak");
            let _ = fs::rename(
                config_dir.join(CONFIG_FILE),
                config_dir.join("config.json.bak"),
            );
            AppConfig::default()
        });
        let mut generated = false;
        for cfg in &mut config.instances {
            if cfg.id.is_empty() {
                cfg.id = new_id();
                generated = true;
            }
        }
        if generated {
            // Clients hold these ids, so they must survive a restart.
            if let Err(e) = write_config(&config_dir, &config) {
                eprintln!("rc-core: {e}");
            }
        }
        let runs = config
            .instances
            .iter()
            .map(|cfg| (cfg.id.clone(), Arc::new(Shared::new(&cfg.id))))
            .collect();
        let manager = Manager {
            inner: Arc::new(Inner {
                dir: config_dir,
                reg: Mutex::new(Registry { config, runs }),
            }),
        };
        let weak = Arc::downgrade(&manager.inner);
        thread::spawn(move || tick(weak));
        for id in manager.ids_where(|cfg| cfg.auto_start) {
            let _ = manager.start(&id);
        }
        manager
    }

    pub fn list(&self) -> Vec<InstanceView> {
        let reg = self.reg();
        reg.config
            .instances
            .iter()
            .filter_map(|cfg| {
                let shared = reg.runs.get(&cfg.id)?;
                let live = shared.live();
                let mut status = live.status.clone();
                status.uptime_secs = live
                    .started_at
                    .filter(|_| status.pid.is_some())
                    .map(|t| t.elapsed().as_secs());
                Some(InstanceView {
                    config: cfg.clone(),
                    status,
                })
            })
            .collect()
    }

    pub fn add(&self, mut cfg: InstanceConfig) -> Result<String, String> {
        if cfg.id.is_empty() {
            cfg.id = new_id();
        }
        let id = cfg.id.clone();
        let mut reg = self.reg();
        if reg.config.instances.iter().any(|c| c.id == id) {
            return Err(format!("duplicate id {id}"));
        }
        self.edit_config(&mut reg, |config| {
            config.instances.push(cfg);
            Ok(())
        })?;
        reg.runs.insert(id.clone(), Arc::new(Shared::new(&id)));
        Ok(id)
    }

    /// Saves the new config. Restarts the instance if it is running.
    pub fn update(&self, cfg: InstanceConfig) -> Result<(), String> {
        let id = cfg.id.clone();
        let running = self.is_running(&id)?;
        {
            let mut reg = self.reg();
            self.edit_config(&mut reg, |config| {
                let slot = config
                    .instances
                    .iter_mut()
                    .find(|c| c.id == id)
                    .ok_or_else(|| unknown(&id))?;
                *slot = cfg;
                Ok(())
            })?;
        }
        if running { self.restart(&id) } else { Ok(()) }
    }

    /// Stops the instance, then deletes it from the config.
    pub fn remove(&self, id: &str) -> Result<(), String> {
        self.stop(id)?;
        let mut reg = self.reg();
        self.edit_config(&mut reg, |config| {
            config.instances.retain(|c| c.id != id);
            Ok(())
        })?;
        reg.runs.remove(id);
        Ok(())
    }

    /// Starts the instance. Does nothing if it already runs.
    pub fn start(&self, id: &str) -> Result<(), String> {
        let (cfg, shared, settings) = {
            let reg = self.reg();
            let cfg = reg
                .config
                .instances
                .iter()
                .find(|c| c.id == id)
                .ok_or_else(|| unknown(id))?
                .clone();
            let shared = reg.runs.get(id).ok_or_else(|| unknown(id))?.clone();
            (cfg, shared, reg.config.settings.clone())
        };
        // Lock order: lifecycle, then live.
        let _lifecycle = shared.lifecycle();
        let Some(bin) = find_claude_bin(&settings) else {
            let message = "claude binary not found".to_owned();
            let mut live = shared.live();
            live.status.state = State::Error;
            live.status.last_error = Some(message.clone());
            return Err(message);
        };
        let mut live = shared.live();
        if live.thread.as_ref().is_some_and(|t| !t.is_finished()) {
            return Ok(());
        }
        shared.stop.store(false, Ordering::SeqCst);
        live.status.state = State::Starting;
        live.status.session_urls.clear();
        let worker = Arc::clone(&shared);
        live.thread = Some(thread::spawn(move || {
            supervisor::run(worker, Job { cfg, bin })
        }));
        Ok(())
    }

    /// Stops the instance and waits until its child is gone.
    pub fn stop(&self, id: &str) -> Result<(), String> {
        let shared = self.shared(id)?;
        // Held for the whole call, so a concurrent start cannot clear the stop flag early.
        let _lifecycle = shared.lifecycle();
        shared.stop.store(true, Ordering::SeqCst);
        // Take the handle out first, so the live lock is free while the thread finishes.
        let worker = shared.live().thread.take();
        if let Some(worker) = worker {
            let _ = worker.join();
        }
        let mut live = shared.live();
        live.status.state = State::Stopped;
        live.status.pid = None;
        live.status.sessions = 0;
        live.status.next_retry_secs = None;
        live.started_at = None;
        Ok(())
    }

    pub fn restart(&self, id: &str) -> Result<(), String> {
        self.stop(id)?;
        self.start(id)
    }

    /// Last 500 log lines, ANSI stripped. Empty for an unknown id.
    pub fn logs(&self, id: &str) -> Vec<String> {
        self.shared(id)
            .map(|s| s.live().log.iter().cloned().collect())
            .unwrap_or_default()
    }

    pub fn settings(&self) -> Settings {
        self.reg().config.settings.clone()
    }

    pub fn set_settings(&self, settings: Settings) -> Result<(), String> {
        let mut reg = self.reg();
        self.edit_config(&mut reg, |config| {
            config.settings = settings;
            Ok(())
        })
    }

    pub fn start_all(&self) {
        for id in self.ids_where(|_| true) {
            let _ = self.start(&id);
        }
    }

    pub fn stop_all(&self) {
        for id in self.ids_where(|_| true) {
            let _ = self.stop(&id);
        }
    }

    /// Stops every instance and waits for all children to exit.
    pub fn shutdown(&self) {
        self.stop_all();
    }

    fn reg(&self) -> MutexGuard<'_, Registry> {
        self.inner.reg.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn shared(&self, id: &str) -> Result<Arc<Shared>, String> {
        self.reg().runs.get(id).cloned().ok_or_else(|| unknown(id))
    }

    fn is_running(&self, id: &str) -> Result<bool, String> {
        let shared = self.shared(id)?;
        let live = shared.live();
        Ok(live.thread.as_ref().is_some_and(|t| !t.is_finished()))
    }

    fn ids_where(&self, keep: impl Fn(&InstanceConfig) -> bool) -> Vec<String> {
        self.reg()
            .config
            .instances
            .iter()
            .filter(|c| keep(c))
            .map(|c| c.id.clone())
            .collect()
    }

    /// Applies `edit` to a copy of the config, saves it, then swaps it in.
    /// The copy keeps memory and disk in step when the save fails.
    fn edit_config(
        &self,
        reg: &mut Registry,
        edit: impl FnOnce(&mut AppConfig) -> Result<(), String>,
    ) -> Result<(), String> {
        let mut next = reg.config.clone();
        edit(&mut next)?;
        write_config(&self.inner.dir, &next)?;
        reg.config = next;
        Ok(())
    }

    /// Refreshes the session counts and rewrites status.json.
    fn publish(&self, sys: &sysinfo::System) {
        let shared: Vec<Arc<Shared>> = self.reg().runs.values().cloned().collect();
        for shared in shared {
            // Read the pid under the lock, scan with the lock released, then write back.
            let pid = shared.live().status.pid;
            if let Some(pid) = pid {
                let sessions = procs::sdk_sessions(sys, pid);
                shared.live().status.sessions = sessions;
            }
        }
        if let Err(e) = write_status(&self.inner.dir, &self.list()) {
            eprintln!("rc-core: {e}");
        }
    }
}

/// Refreshes process data and publishes status every two seconds. Ends with the last Manager.
fn tick(weak: Weak<Inner>) {
    let Ok(mut sys) = sysinfo::System::new() else {
        eprintln!("rc-core: cannot read process table; session counts stay at 0");
        return;
    };
    while let Some(inner) = weak.upgrade() {
        procs::refresh(&mut sys);
        let manager = Manager { inner };
        manager.publish(&sys);
        drop(manager);
        thread::sleep(TICK);
    }
}

fn unknown(id: &str) -> String {
    format!("unknown instance {id}")
}

fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}
