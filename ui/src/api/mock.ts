// In-memory stand-in for the Tauri backend. Used when __TAURI_INTERNALS__ is absent
// (browser dev, tests). Add `?demo` to the URL to start with two sample folders.
import type { RcApi } from "./index";
import type { InstanceConfig, InstanceStatus, InstanceView, State, UiSettings } from "./types";

type Listener = (views: InstanceView[]) => void;

const views = new Map<string, InstanceView>();
const logs = new Map<string, string[]>();
const listeners = new Set<Listener>();
let settings: UiSettings = { claudeBin: null, autostart: false };
let timer: ReturnType<typeof setInterval> | undefined;
let nextId = 1;

const snapshot = (): InstanceView[] => [...views.values()].map((v) => structuredClone(v));
const emit = () => listeners.forEach((l) => l(snapshot()));

function setStatus(id: string, patch: Partial<InstanceStatus>) {
  const v = views.get(id);
  if (v) views.set(id, { ...v, status: { ...v.status, ...patch } });
}

function log(id: string, line: string) {
  const buf = logs.get(id) ?? [];
  buf.push(`${new Date().toLocaleTimeString()} ${line}`);
  logs.set(id, buf.slice(-500));
}

// Fake run loop: starting -> running after 1 s; running instances accrue uptime.
function later(id: string, ms: number, state: State) {
  setTimeout(() => {
    const cur = views.get(id);
    if (!cur || cur.status.state === "stopped") return;
    setStatus(id, { state, uptimeSecs: state === "running" ? 0 : null, nextRetrySecs: null, pid: state === "running" ? 4242 : null });
    log(id, `state -> ${state}`);
    emit();
  }, ms);
}

function ensureTimer() {
  if (timer) return;
  timer = setInterval(() => {
    if (listeners.size === 0) {
      clearInterval(timer);
      timer = undefined;
      return;
    }
    for (const v of views.values()) {
      if (v.status.state === "running") {
        setStatus(v.config.id, { uptimeSecs: (v.status.uptimeSecs ?? 0) + 1 });
        log(v.config.id, "heartbeat ok");
      }
    }
    emit();
  }, 1000);
}

function newStatus(id: string, state: State): InstanceStatus {
  return { id, state, pid: null, sessions: 0, uptimeSecs: null, restarts: 0, nextRetrySecs: null, lastError: null, sessionUrls: [] };
}

function seed() {
  const base = { chrome: true, permissionMode: null, spawn: null, capacity: null, createSessionInDir: null, sessionNamePrefix: null, extraArgs: [], autoTrust: true, autoStart: true };
  const a = { id: "demo-1", folder: "/Users/you/dev/app", name: "app", ...base };
  const b = { id: "demo-2", folder: "/Users/you/dev/site", name: null, ...base, autoStart: false };
  views.set(a.id, { config: a, status: { ...newStatus(a.id, "running"), pid: 4242, sessions: 2, uptimeSecs: 5400, restarts: 1, sessionUrls: ["https://claude.ai/code/session_demo1"] } });
  views.set(b.id, { config: b, status: { ...newStatus(b.id, "error"), restarts: 3, lastError: "claude exited with code 1" } });
  logs.set(a.id, ["12:00:01 starting", "12:00:02 session ready"]);
  logs.set(b.id, ["12:00:05 exit 1"]);
}

if (typeof window !== "undefined" && window.location.search.includes("demo")) seed();

export const mockApi: RcApi = {
  async listInstances() {
    return snapshot();
  },
  async addInstance(cfg: InstanceConfig) {
    const id = cfg.id || `mock-${nextId++}`;
    const config = { ...cfg, id };
    views.set(id, { config, status: newStatus(id, config.autoStart ? "starting" : "stopped") });
    if (config.autoStart) later(id, 1000, "running");
    emit();
    return id;
  },
  async updateInstance(cfg: InstanceConfig) {
    const v = views.get(cfg.id);
    if (!v) throw new Error(`no instance ${cfg.id}`);
    views.set(cfg.id, { ...v, config: { ...cfg } });
    emit();
  },
  async removeInstance(id: string) {
    views.delete(id);
    logs.delete(id);
    emit();
  },
  async startInstance(id: string) {
    setStatus(id, { state: "starting" });
    log(id, "starting");
    later(id, 1000, "running");
    emit();
  },
  async stopInstance(id: string) {
    setStatus(id, { state: "stopped", pid: null, uptimeSecs: null, nextRetrySecs: null });
    log(id, "stopped");
    emit();
  },
  async restartInstance(id: string) {
    setStatus(id, { state: "restarting", nextRetrySecs: 2, restarts: (views.get(id)?.status.restarts ?? 0) + 1 });
    log(id, "restarting");
    later(id, 2000, "running");
    emit();
  },
  async getLogs(id: string) {
    return [...(logs.get(id) ?? [])];
  },
  async startAll() {
    for (const id of views.keys()) await mockApi.startInstance(id);
  },
  async stopAll() {
    for (const id of views.keys()) await mockApi.stopInstance(id);
  },
  async getSettings() {
    return { ...settings };
  },
  async setSettings(s: UiSettings) {
    settings = { ...s };
  },
  async daemonInfo() {
    return { version: "0.1.0-mock", running: true };
  },
  async stopDaemon() {
    log("mock", "daemon stop requested");
  },
  async detectClaudeBin() {
    return "/usr/local/bin/claude";
  },
  async subscribe(onChange: Listener) {
    listeners.add(onChange);
    ensureTimer();
    return () => {
      listeners.delete(onChange);
    };
  },
};
