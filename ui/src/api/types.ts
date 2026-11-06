// IPC wire types come from the generated schema types. Do not redefine them here.
export type { InstanceConfig, InstanceStatus, InstanceView, State, TakeoverResult } from "./generated";

/** Result of the GUI command `get_settings`. Not an IPC type: the GUI owns autostart. */
export interface UiSettings {
  claudeBin: string | null;
  autostart: boolean;
  /** macOS only: hide the Dock icon, keep the menu bar icon. */
  hideDock: boolean;
}

/** Result of the GUI command `update_info`. Not an IPC type. */
export interface UpdateInfo {
  current: string;
  latest: string;
  updateAvailable: boolean;
  /** True while an update runs (marker written by `rc-keeper update`). */
  updating: boolean;
  updatingTo: string | null;
}

/** Result of the GUI command `daemon_info`. Not an IPC type. */
export interface DaemonInfo {
  version: string;
  running: boolean;
}
