// Mirrors CONTRACT.md. Keep in sync with rc-core (serde camelCase).

export type State =
  | "stopped"
  | "starting"
  | "running"
  | "restarting"
  | "folder-busy"
  | "needs-trust"
  | "error";

export interface InstanceConfig {
  id: string;
  folder: string;
  name: string | null;
  chrome: boolean;
  permissionMode: string | null;
  spawn: string | null;
  capacity: number | null;
  createSessionInDir: boolean | null;
  sessionNamePrefix: string | null;
  extraArgs: string[];
  autoTrust: boolean;
  autoStart: boolean;
}

export interface InstanceStatus {
  id: string;
  state: State;
  pid: number | null;
  sessions: number;
  uptimeSecs: number | null;
  restarts: number;
  nextRetrySecs: number | null;
  lastError: string | null;
  sessionUrls: string[];
}

export interface InstanceView {
  config: InstanceConfig;
  status: InstanceStatus;
}

export interface UiSettings {
  claudeBin: string | null;
  autostart: boolean;
}

export interface DaemonInfo {
  version: string;
  running: boolean;
}
