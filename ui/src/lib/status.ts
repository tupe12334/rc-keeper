import type { InstanceView, State } from "@/api";

export type Tone = "green" | "yellow" | "red" | "gray";

export function stateLabel(state: State, nextRetrySecs: number | null = null): string {
  switch (state) {
    case "stopped":
      return "Stopped";
    case "starting":
      return "Starting";
    case "running":
      return "Running";
    case "restarting":
      return nextRetrySecs != null ? `Restarting in ${nextRetrySecs}s` : "Restarting";
    case "folder-busy":
      return "Folder busy";
    case "needs-trust":
      return "Needs trust";
    case "error":
      return "Error";
  }
}

export const stateHelp: Partial<Record<State, string>> = {
  "folder-busy": "Another claude rc already serves this folder.",
  "needs-trust": "Claude asks you to trust this folder. Enable \"Trust this folder automatically\" or trust it in Claude.",
};

export function stateTone(state: State): Tone {
  switch (state) {
    case "running":
      return "green";
    case "starting":
    case "restarting":
    case "folder-busy":
      return "yellow";
    case "needs-trust":
    case "error":
      return "red";
    case "stopped":
      return "gray";
  }
}

/** Overall header tone: any error or trust need is red; all running is green; all stopped or no folders is gray; otherwise yellow. */
export function overallTone(views: InstanceView[]): Tone {
  if (views.length === 0) return "gray";
  const states = views.map((v) => v.status.state);
  if (states.some((s) => s === "error" || s === "needs-trust")) return "red";
  if (states.every((s) => s === "stopped")) return "gray";
  if (states.every((s) => s === "running")) return "green";
  return "yellow";
}

/** Turns /Users/<name>/... into ~/... and keeps only the last 3 segments when long. */
export function shortenPath(folder: string): string {
  const home = folder.replace(/^\/(Users|home)\/[^/]+/, "~").replace(/^C:\\Users\\[^\\]+/i, "~");
  const parts = home.split(/[\\/]/);
  if (parts.length <= 4) return home;
  return "…/" + parts.slice(-3).join("/");
}

export function formatUptime(secs: number | null): string {
  if (secs == null) return "—";
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const s = secs % 60;
  if (h > 0) return `${h}h ${m}m`;
  if (m > 0) return `${m}m ${s}s`;
  return `${s}s`;
}
