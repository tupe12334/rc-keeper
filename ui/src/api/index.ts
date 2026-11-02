import { mockApi } from "./mock";
import { tauriApi } from "./tauri";
import type { DaemonInfo, InstanceConfig, InstanceView, TakeoverResult, UiSettings } from "./types";

export type { DaemonInfo, InstanceConfig, InstanceView, InstanceStatus, State, TakeoverResult, UiSettings } from "./types";

export interface RcApi {
  listInstances(): Promise<InstanceView[]>;
  addInstance(cfg: InstanceConfig): Promise<string>;
  updateInstance(cfg: InstanceConfig): Promise<void>;
  removeInstance(id: string): Promise<void>;
  startInstance(id: string): Promise<void>;
  stopInstance(id: string): Promise<void>;
  restartInstance(id: string): Promise<void>;
  /** Stops the other claude rc in the folder, then restarts the instance. */
  forceStart(id: string): Promise<TakeoverResult>;
  /** Lists the pids that `forceStart` would stop. Stops nothing. */
  previewForceStart(id: string): Promise<TakeoverResult>;
  getLogs(id: string): Promise<string[]>;
  startAll(): Promise<void>;
  stopAll(): Promise<void>;
  getSettings(): Promise<UiSettings>;
  setSettings(s: UiSettings): Promise<void>;
  detectClaudeBin(): Promise<string | null>;
  daemonInfo(): Promise<DaemonInfo>;
  stopDaemon(): Promise<void>;
  /** Calls `onChange` with every `instances` event. Returns an unsubscribe fn. */
  subscribe(onChange: (views: InstanceView[]) => void): Promise<() => void>;
}

const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export const api: RcApi = inTauri ? tauriApi : mockApi;
export const isTauri = inTauri;
