import { useEffect, useState } from "react";
import { toast } from "sonner";
import { api, type DaemonInfo, type UiSettings } from "@/api";
import { FieldHelp } from "@/components/FieldHelp";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";

export function SettingsDialog({ onClose }: { onClose: () => void }) {
  const [settings, setSettings] = useState<UiSettings>({ claudeBin: null, autostart: false });
  const [bin, setBin] = useState("");
  const [daemon, setDaemon] = useState<DaemonInfo | null>(null);

  useEffect(() => {
    let alive = true;
    void api.getSettings().then((s) => {
      if (!alive) return;
      setSettings(s);
      setBin(s.claudeBin ?? "");
    });
    void api
      .daemonInfo()
      .then((d) => {
        if (alive) setDaemon(d);
      })
      .catch(() => {
        if (alive) setDaemon(null);
      });
    return () => {
      alive = false;
    };
  }, []);

  async function detect() {
    const found = await api.detectClaudeBin();
    if (found) {
      setBin(found);
      toast.success(`Found ${found}`);
    } else {
      toast.error("Claude binary not found. Enter the path yourself.");
    }
  }

  async function save() {
    try {
      await api.setSettings({ claudeBin: bin.trim() === "" ? null : bin.trim(), autostart: settings.autostart });
      toast.success("Settings saved");
      onClose();
    } catch (e) {
      toast.error(String(e));
    }
  }

  async function stopDaemon() {
    try {
      await api.stopDaemon();
      setDaemon((d) => (d ? { ...d, running: false } : d));
      toast.success("Daemon stopped");
    } catch (e) {
      toast.error(String(e));
    }
  }

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Settings</DialogTitle>
          <DialogDescription>Where to find Claude, and how the app starts.</DialogDescription>
        </DialogHeader>

        <div className="flex flex-col gap-4 overflow-y-auto">
          <div className="grid gap-1.5">
            <div className="flex items-center gap-1.5">
              <label htmlFor="s-bin" className="text-sm font-medium">
                Claude binary path
              </label>
              <FieldHelp field="settings.claudeBin" />
            </div>
            <div className="flex gap-2">
              <Input id="s-bin" value={bin} onChange={(e) => setBin(e.target.value)} placeholder="Found on PATH when empty" />
              <Button variant="outline" onClick={() => void detect()}>
                Detect
              </Button>
            </div>
          </div>

          <div className="flex items-center justify-between gap-4">
            <div className="flex items-center gap-1.5">
              <label htmlFor="s-autostart" className="text-sm font-medium">
                Launch at login
              </label>
              <FieldHelp field="settings.autostart" />
            </div>
            <Switch id="s-autostart" checked={settings.autostart} onCheckedChange={(v) => setSettings((s) => ({ ...s, autostart: v }))} />
          </div>

          <section aria-labelledby="s-daemon" className="grid gap-2 border-t border-border pt-4">
            <h3 id="s-daemon" className="text-sm font-medium">
              Daemon
            </h3>
            <p className="text-sm text-muted-foreground" role="status">
              {daemon ? `Version ${daemon.version}, ${daemon.running ? "running" : "stopped"}` : "Daemon info unavailable"}
            </p>
            <div>
              <Button variant="outline" onClick={() => void stopDaemon()} disabled={!daemon?.running}>
                Stop daemon
              </Button>
            </div>
          </section>

          <div className="flex justify-end gap-2">
            <Button variant="outline" onClick={onClose}>
              Cancel
            </Button>
            <Button onClick={() => void save()}>Save</Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
