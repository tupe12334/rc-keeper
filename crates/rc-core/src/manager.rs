//! Public entry point. Owns the instance registry, the supervisor threads and status.json.
use crate::config::{self, Loaded};
use crate::locate::find_claude_bin;
use crate::model::{AppConfig, InstanceConfig, InstanceStatus, InstanceView, Settings, State};
use crate::procs;
use crate::supervisor::{self, Job, Shared};
use crate::watch;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
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
    home: Option<PathBuf>,
    reg: Mutex<Registry>,
}

#[derive(Default)]
struct Registry {
    config: AppConfig,
    /// Instance files that do not parse, by id, with the parse error. A known id that is also
    /// here keeps its last good config and its process, and shows the error as `last_error`.
    broken: BTreeMap<String, String>,
    runs: HashMap<String, Arc<Shared>>,
}

impl Manager {
    /// Reads the config dir, migrates a legacy `config.json`, watches the dir for edits,
    /// and starts the instances with `auto_start`.
    pub fn load(config_dir: PathBuf) -> Manager {
        if let Err(e) = fs::create_dir_all(&config_dir) {
            eprintln!("rc-core: cannot create {}: {e}", config_dir.display());
        }
        let home = config::home_dir();
        if let Err(e) = config::migrate_legacy(&config_dir, home.as_deref()) {
            eprintln!("rc-core: {e}");
        }
        if let Err(e) = config::ensure_gitignore(&config_dir) {
            eprintln!("rc-core: {e}");
        }
        let Loaded { config, broken } = config::load(&config_dir, home.as_deref());
        let runs = config
            .instances
            .iter()
            .map(|cfg| (cfg.id.clone(), Arc::new(Shared::new(&cfg.id))))
            .collect();
        let manager = Manager {
            inner: Arc::new(Inner {
                dir: config_dir.clone(),
                home,
                reg: Mutex::new(Registry {
                    config,
                    broken,
                    runs,
                }),
            }),
        };
        let weak = Arc::downgrade(&manager.inner);
        let alive = weak.clone();
        thread::spawn(move || tick(weak));
        let watched = watch::spawn(config_dir, move || alive.strong_count() > 0, {
            let weak = Arc::downgrade(&manager.inner);
            move || {
                if let Some(inner) = weak.upgrade() {
                    Manager { inner }.reload();
                }
            }
        });
        if let Err(e) = watched {
            eprintln!("rc-core: cannot watch the config dir: {e}");
        }
        for id in manager.ids_where(|cfg| cfg.auto_start) {
            let _ = manager.start(&id);
        }
        manager
    }

    pub fn list(&self) -> Vec<InstanceView> {
        let reg = self.reg();
        let mut views: Vec<InstanceView> = reg
            .config
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
                if let Some(message) = reg.broken.get(&cfg.id) {
                    status.last_error = Some(message.clone());
                    let running = live.thread.as_ref().is_some_and(|t| !t.is_finished());
                    if !running {
                        status.state = State::Error;
                    }
                }
                Some(InstanceView {
                    config: cfg.clone(),
                    status,
                })
            })
            .collect();
        views.extend(
            reg.broken
                .iter()
                .filter(|(id, _)| !reg.config.instances.iter().any(|c| c.id == **id))
                .map(|(id, message)| InstanceView {
                    config: InstanceConfig {
                        id: id.clone(),
                        ..Default::default()
                    },
                    status: InstanceStatus {
                        id: id.clone(),
                        state: State::Error,
                        last_error: Some(message.clone()),
                        ..Default::default()
                    },
                }),
        );
        // Sorted by id. Ids are unique, so the order is stable across reloads.
        views.sort_by(|a, b| a.config.id.cmp(&b.config.id));
        views
    }

    /// Writes the instance file and starts it when `auto_start` is set, the same as a
    /// file that appears on disk. An empty id is generated from the folder name.
    pub fn add(&self, cfg: InstanceConfig) -> Result<String, String> {
        let auto_start = cfg.auto_start;
        let id = self.save_new(cfg)?;
        if auto_start {
            // A start failure (no claude binary) shows as the instance's state and last error.
            let _ = self.start(&id);
        }
        Ok(id)
    }

    fn save_new(&self, mut cfg: InstanceConfig) -> Result<String, String> {
        let mut reg = self.reg();
        if cfg.id.is_empty() {
            let base = config::slug_of(&cfg.folder);
            cfg.id = config::unique_id(&base, |id| self.taken(&reg, id));
        } else if self.taken(&reg, &cfg.id) {
            return Err(format!("duplicate id {}", cfg.id));
        }
        config::save_instance(&self.inner.dir, &cfg, self.home())?;
        let id = cfg.id.clone();
        reg.runs.insert(id.clone(), Arc::new(Shared::new(&id)));
        reg.config.instances.push(cfg);
        Ok(id)
    }

    /// Saves the new config. Restarts the instance if it is running.
    pub fn update(&self, cfg: InstanceConfig) -> Result<(), String> {
        let id = cfg.id.clone();
        let running = self.is_running(&id)?;
        {
            let mut reg = self.reg();
            let slot = reg
                .config
                .instances
                .iter()
                .position(|c| c.id == id)
                .ok_or_else(|| unknown(&id))?;
            config::save_instance(&self.inner.dir, &cfg, self.home())?;
            reg.config.instances[slot] = cfg;
        }
        if running { self.restart(&id) } else { Ok(()) }
    }

    /// Stops the instance, then deletes its file. A broken file is deleted without a stop.
    pub fn remove(&self, id: &str) -> Result<(), String> {
        let (known, broken) = {
            let reg = self.reg();
            (
                reg.config.instances.iter().any(|c| c.id == id),
                reg.broken.contains_key(id),
            )
        };
        if !known && !broken {
            return Err(unknown(id));
        }
        if known {
            self.stop(id)?;
        }
        let removed = {
            let mut reg = self.reg();
            config::remove_instance(&self.inner.dir, id)?;
            reg.config.instances.retain(|c| c.id != id);
            reg.broken.remove(id);
            reg.runs.remove(id)
        };
        // A start that slipped in between the stop above and the removal is stopped here.
        if let Some(shared) = removed {
            stop_shared(&shared);
        }
        Ok(())
    }

    /// Starts the instance. Does nothing if it already runs.
    pub fn start(&self, id: &str) -> Result<(), String> {
        let (cfg, shared, settings) = {
            let reg = self.reg();
            if let Some(message) = reg.broken.get(id).filter(|_| !running_in(&reg, id)) {
                return Err(message.clone());
            }
            let shared = reg.runs.get(id).ok_or_else(|| unknown(id))?.clone();
            let cfg = reg
                .config
                .instances
                .iter()
                .find(|c| c.id == id)
                .ok_or_else(|| unknown(id))?
                .clone();
            (cfg, shared, reg.config.settings.clone())
        };
        // Lock order: lifecycle, then live.
        let _lifecycle = shared.lifecycle();
        // Removed while we waited for the lifecycle lock: never spawn for a gone instance.
        if !self
            .reg()
            .runs
            .get(id)
            .is_some_and(|s| Arc::ptr_eq(s, &shared))
        {
            return Err(unknown(id));
        }
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
        stop_shared(&shared);
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
        config::save_settings(&self.inner.dir, &settings)?;
        reg.config.settings = settings;
        Ok(())
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

    fn home(&self) -> Option<&Path> {
        self.inner.home.as_deref()
    }

    /// An id is taken by a loaded instance, a broken file, or any instance file on disk.
    fn taken(&self, reg: &Registry, id: &str) -> bool {
        reg.config.instances.iter().any(|c| c.id == id)
            || reg.broken.contains_key(id)
            || config::instance_exists(&self.inner.dir, id)
    }

    /// Applies the files on disk to the registry. The watcher calls this after a quiet period.
    ///
    /// One lock covers the read, the diff and the apply, so an add or update cannot slip between them.
    /// Stops and starts run after the lock is released, because they lock the registry themselves.
    /// Reload never writes files, so the daemon's own writes cause no further reload.
    fn reload(&self) {
        let mut to_stop: Vec<Arc<Shared>> = Vec::new();
        let mut to_start: Vec<String> = Vec::new();
        {
            let mut reg = self.reg();
            let fresh = config::load(&self.inner.dir, self.home());
            let mut instances = fresh.config.instances.clone();
            for cfg in &reg.config.instances {
                match fresh.config.instances.iter().find(|c| c.id == cfg.id) {
                    Some(new) => {
                        // Changed and running: stop the old process, then start with the new config.
                        if new != cfg && running_in(&reg, &cfg.id) {
                            to_stop.extend(reg.runs.get(&cfg.id).cloned());
                            to_start.push(cfg.id.clone());
                        }
                    }
                    None if fresh.broken.contains_key(&cfg.id) => {
                        // Possibly a read caught mid-write. Keep the last good config and the process.
                        instances.push(cfg.clone());
                    }
                    None => {
                        // The file is gone: the instance is removed.
                        to_stop.extend(reg.runs.get(&cfg.id).cloned());
                    }
                }
            }
            for cfg in &fresh.config.instances {
                let known = reg.config.instances.iter().any(|c| c.id == cfg.id);
                if cfg.auto_start && !known {
                    to_start.push(cfg.id.clone());
                }
            }
            reg.runs
                .retain(|id, _| instances.iter().any(|c| c.id == *id));
            for cfg in &instances {
                reg.runs
                    .entry(cfg.id.clone())
                    .or_insert_with(|| Arc::new(Shared::new(&cfg.id)));
            }
            reg.config.instances = instances;
            reg.config.settings = fresh.config.settings;
            reg.broken = fresh.broken;
        }
        for shared in &to_stop {
            stop_shared(shared);
        }
        for id in &to_start {
            let _ = self.start(id);
        }
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
        if let Err(e) = config::write_status(&self.inner.dir, &self.list()) {
            eprintln!("rc-core: {e}");
        }
    }
}

/// Stops the instance and waits for its thread. Caller must not hold the registry lock.
fn stop_shared(shared: &Shared) {
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

/// Caller holds the registry lock. Lock order: registry, then live.
fn running_in(reg: &Registry, id: &str) -> bool {
    reg.runs.get(id).is_some_and(|shared| {
        shared
            .live()
            .thread
            .as_ref()
            .is_some_and(|t| !t.is_finished())
    })
}

fn unknown(id: &str) -> String {
    format!("unknown instance {id}")
}
