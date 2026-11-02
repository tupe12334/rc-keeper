import type { InstanceConfig } from "@/api";

export const PERMISSION_MODES = ["default", "acceptEdits", "auto", "bypassPermissions", "dontAsk", "plan"] as const;
export const SPAWN_MODES = ["same-dir", "worktree", "session"] as const;
export const UNSET = "__unset__";

export interface InstanceForm {
  folder: string;
  name: string;
  chrome: boolean;
  permissionMode: string; // UNSET or a PERMISSION_MODES value
  spawn: string; // UNSET or a SPAWN_MODES value
  capacity: string; // "" = unset
  createSessionInDir: string; // "unset" | "on" | "off"
  sessionNamePrefix: string;
  extraArgs: string;
  autoTrust: boolean;
  autoStart: boolean;
  forceBoot: boolean;
}

export function emptyForm(): InstanceForm {
  return {
    folder: "",
    name: "",
    chrome: true,
    permissionMode: UNSET,
    spawn: UNSET,
    capacity: "",
    createSessionInDir: "unset",
    sessionNamePrefix: "",
    extraArgs: "",
    autoTrust: true,
    autoStart: true,
    forceBoot: false,
  };
}

/** Splits on whitespace; "double" and 'single' quoted spans stay one arg. */
export function parseExtraArgs(text: string): string[] {
  return [...text.matchAll(/"([^"]*)"|'([^']*)'|(\S+)/g)].map((m) => m[1] ?? m[2] ?? m[3]);
}

/** Inverse of parseExtraArgs: quotes args that contain whitespace. */
export function formatExtraArgs(args: string[]): string {
  return args.map((a) => (/\s/.test(a) ? `"${a}"` : a)).join(" ");
}

export const CAPACITY_MAX = 4294967295; // u32::MAX, the Rust side type

const orNull = (s: string) => (s.trim() === "" ? null : s.trim());
const pick = (s: string) => (s === UNSET ? null : s);

export function formToConfig(form: InstanceForm, id: string): InstanceConfig {
  const capacity = form.capacity.trim();
  return {
    id,
    folder: form.folder.trim(),
    name: orNull(form.name),
    chrome: form.chrome,
    permissionMode: pick(form.permissionMode),
    spawn: pick(form.spawn),
    capacity: capacity === "" ? null : Number(capacity),
    createSessionInDir: form.createSessionInDir === "unset" ? null : form.createSessionInDir === "on",
    sessionNamePrefix: orNull(form.sessionNamePrefix),
    extraArgs: parseExtraArgs(form.extraArgs),
    autoTrust: form.autoTrust,
    autoStart: form.autoStart,
    forceBoot: form.forceBoot,
  };
}

export function configToForm(cfg: InstanceConfig): InstanceForm {
  return {
    folder: cfg.folder,
    name: cfg.name ?? "",
    chrome: cfg.chrome,
    permissionMode: cfg.permissionMode ?? UNSET,
    spawn: cfg.spawn ?? UNSET,
    capacity: cfg.capacity == null ? "" : String(cfg.capacity),
    createSessionInDir: cfg.createSessionInDir == null ? "unset" : cfg.createSessionInDir ? "on" : "off",
    sessionNamePrefix: cfg.sessionNamePrefix ?? "",
    extraArgs: formatExtraArgs(cfg.extraArgs),
    autoTrust: cfg.autoTrust,
    autoStart: cfg.autoStart,
    forceBoot: cfg.forceBoot,
  };
}

/** Returns an error message, or null when the form can be saved. */
export function validateForm(form: InstanceForm): string | null {
  if (form.folder.trim() === "") return "Folder is required.";
  const cap = form.capacity.trim();
  if (cap !== "" && !(Number.isInteger(Number(cap)) && Number(cap) > 0 && Number(cap) <= CAPACITY_MAX)) {
    return `Capacity must be a whole number from 1 to ${CAPACITY_MAX}.`;
  }
  return null;
}
