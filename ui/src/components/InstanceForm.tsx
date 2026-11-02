import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { AlertTriangle, FolderOpen } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";
import { api, isTauri, type InstanceConfig } from "@/api";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import {
  configToForm,
  emptyForm,
  formToConfig,
  PERMISSION_MODES,
  type InstanceForm as FormState,
  SPAWN_MODES,
  UNSET,
  validateForm,
} from "@/lib/instanceConfig";

export const BYPASS_WARNING = "Remote sessions will run any command in this folder without asking.";
export const TRUST_HELP = "Remote sessions can read, edit and run files in this folder.";

interface Props {
  /** Config to edit, or null to add a new folder. */
  initial: InstanceConfig | null;
  onClose: () => void;
}

const label = "text-sm font-medium";
const hint = "text-xs text-muted-foreground";

export function InstanceForm({ initial, onClose }: Props) {
  const [form, setForm] = useState<FormState>(() => (initial ? configToForm(initial) : emptyForm()));
  const [error, setError] = useState<{ field: "folder" | "capacity"; message: string } | null>(null);
  const [saving, setSaving] = useState(false);
  const set = <K extends keyof FormState>(key: K, value: FormState[K]) => setForm((f) => ({ ...f, [key]: value }));

  async function pickFolder() {
    try {
      const picked = await openDialog({ directory: true, multiple: false });
      if (typeof picked === "string") set("folder", picked);
    } catch (e) {
      toast.error(String(e));
    }
  }

  async function save() {
    const problem = validateForm(form);
    if (problem) {
      setError({ field: form.folder.trim() === "" ? "folder" : "capacity", message: problem });
      return;
    }
    setError(null);
    setSaving(true);
    try {
      if (initial) {
        await api.updateInstance(formToConfig(form, initial.id));
        toast.success("Folder updated");
      } else {
        await api.addInstance(formToConfig(form, ""));
        toast.success("Folder added");
      }
      setSaving(false);
      onClose();
    } catch (e) {
      setSaving(false);
      toast.error(String(e));
    }
  }

  const describe = (field: "folder" | "capacity") => (error?.field === field ? "f-error" : undefined);

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{initial ? "Edit folder" : "Add folder"}</DialogTitle>
          <DialogDescription>Remote Control keeps running for this folder.</DialogDescription>
        </DialogHeader>

        <form
          className="flex min-h-0 flex-col gap-4 overflow-y-auto pr-1"
          onSubmit={(e) => {
            e.preventDefault();
            void save();
          }}
        >
          <div className="grid gap-1.5">
            <label htmlFor="f-folder" className={label}>
              Folder
            </label>
            <div className="flex gap-2">
              <Input id="f-folder" value={form.folder} onChange={(e) => set("folder", e.target.value)} placeholder="/Users/you/project" aria-invalid={error?.field === "folder"} aria-describedby={describe("folder")} />
              {isTauri && (
                <Button type="button" variant="outline" onClick={() => void pickFolder()}>
                  <FolderOpen />
                  Browse
                </Button>
              )}
            </div>
          </div>

          <div className="grid gap-1.5">
            <label htmlFor="f-name" className={label}>
              Name
            </label>
            <Input id="f-name" value={form.name} onChange={(e) => set("name", e.target.value)} placeholder="Defaults to the folder name" />
          </div>

          <div className="flex items-center justify-between gap-4">
            <div>
              <label htmlFor="f-chrome" className={label}>
                Chrome
              </label>
              <p className={hint}>Pass the Chrome integration flag.</p>
            </div>
            <Switch id="f-chrome" checked={form.chrome} onCheckedChange={(v) => set("chrome", v)} />
          </div>

          <div className="grid gap-1.5">
            <span id="f-perm-label" className={label}>
              Permission mode
            </span>
            <Select value={form.permissionMode} onValueChange={(v) => set("permissionMode", v)}>
              <SelectTrigger aria-labelledby="f-perm-label">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value={UNSET}>Unset</SelectItem>
                {PERMISSION_MODES.map((m) => (
                  <SelectItem key={m} value={m}>
                    {m}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            {form.permissionMode === "bypassPermissions" && (
              <p role="alert" className="flex items-start gap-2 rounded-md bg-warning/15 p-2 text-sm text-warning">
                <AlertTriangle className="mt-0.5 size-4 shrink-0" aria-hidden />
                {BYPASS_WARNING}
              </p>
            )}
          </div>

          <div className="grid gap-1.5">
            <span id="f-spawn-label" className={label}>
              Spawn mode
            </span>
            <Select value={form.spawn} onValueChange={(v) => set("spawn", v)}>
              <SelectTrigger aria-labelledby="f-spawn-label">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value={UNSET}>Unset</SelectItem>
                {SPAWN_MODES.map((m) => (
                  <SelectItem key={m} value={m}>
                    {m}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>

          <div className="grid gap-1.5">
            <label htmlFor="f-capacity" className={label}>
              Capacity
            </label>
            <Input id="f-capacity" type="number" min={1} inputMode="numeric" value={form.capacity} onChange={(e) => set("capacity", e.target.value)} placeholder="Unset" aria-invalid={error?.field === "capacity"} aria-describedby={describe("capacity")} />
          </div>

          <div className="grid gap-1.5">
            <span id="f-cwd-label" className={label}>
              Create session in directory
            </span>
            <Select value={form.createSessionInDir} onValueChange={(v) => set("createSessionInDir", v)}>
              <SelectTrigger aria-labelledby="f-cwd-label">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="unset">Unset</SelectItem>
                <SelectItem value="on">On</SelectItem>
                <SelectItem value="off">Off</SelectItem>
              </SelectContent>
            </Select>
          </div>

          <div className="grid gap-1.5">
            <label htmlFor="f-prefix" className={label}>
              Session name prefix
            </label>
            <Input id="f-prefix" value={form.sessionNamePrefix} onChange={(e) => set("sessionNamePrefix", e.target.value)} />
          </div>

          <div className="grid gap-1.5">
            <label htmlFor="f-args" className={label}>
              Extra arguments
            </label>
            <Input id="f-args" value={form.extraArgs} onChange={(e) => set("extraArgs", e.target.value)} placeholder="--verbose --foo" />
            <p className={hint}>Separated by spaces.</p>
          </div>

          <div className="flex items-start justify-between gap-4">
            <div>
              <label htmlFor="f-trust" className={label}>
                Trust this folder automatically
              </label>
              <p className={hint}>{TRUST_HELP}</p>
            </div>
            <Switch id="f-trust" checked={form.autoTrust} onCheckedChange={(v) => set("autoTrust", v)} />
          </div>

          <div className="flex items-center justify-between gap-4">
            <label htmlFor="f-autostart" className={label}>
              Start when the app opens
            </label>
            <Switch id="f-autostart" checked={form.autoStart} onCheckedChange={(v) => set("autoStart", v)} />
          </div>

          {error && (
            <p id="f-error" role="alert" className="text-sm text-destructive">
              {error.message}
            </p>
          )}

          <div className="flex justify-end gap-2 pt-1">
            <Button type="button" variant="outline" onClick={onClose}>
              Cancel
            </Button>
            <Button type="submit" disabled={saving}>
              Save
            </Button>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  );
}
