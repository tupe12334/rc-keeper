import { openUrl } from "@tauri-apps/plugin-opener";
import { isTauri } from "@/api";

export function openExternal(url: string): Promise<void> {
  if (isTauri) return openUrl(url);
  window.open(url, "_blank", "noopener,noreferrer");
  return Promise.resolve();
}
