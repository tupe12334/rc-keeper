import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { RcApi } from "./index";
import type { DaemonInfo, InstanceConfig, InstanceView, UiSettings } from "./types";

export const tauriApi: RcApi = {
  listInstances: () => invoke<InstanceView[]>("list_instances"),
  addInstance: (cfg: InstanceConfig) => invoke<string>("add_instance", { cfg }),
  updateInstance: (cfg: InstanceConfig) => invoke<void>("update_instance", { cfg }),
  removeInstance: (id: string) => invoke<void>("remove_instance", { id }),
  startInstance: (id: string) => invoke<void>("start_instance", { id }),
  stopInstance: (id: string) => invoke<void>("stop_instance", { id }),
  restartInstance: (id: string) => invoke<void>("restart_instance", { id }),
  getLogs: (id: string) => invoke<string[]>("get_logs", { id }),
  startAll: () => invoke<void>("start_all"),
  stopAll: () => invoke<void>("stop_all"),
  getSettings: () => invoke<UiSettings>("get_settings"),
  setSettings: (s: UiSettings) => invoke<void>("set_settings", { claudeBin: s.claudeBin, autostart: s.autostart }),
  detectClaudeBin: () => invoke<string | null>("detect_claude_bin"),
  daemonInfo: () => invoke<DaemonInfo>("daemon_info"),
  stopDaemon: () => invoke<void>("stop_daemon"),
  subscribe: (onChange) => listen<InstanceView[]>("instances", (e) => onChange(e.payload)),
};
