import { FolderPlus, Settings as SettingsIcon } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";
import { api, type InstanceView } from "@/api";
import { EmptyState } from "@/components/EmptyState";
import { InstanceCard } from "@/components/InstanceCard";
import { InstanceForm } from "@/components/InstanceForm";
import { LogsDialog } from "@/components/LogsDialog";
import { SettingsDialog } from "@/components/SettingsDialog";
import { Button } from "@/components/ui/button";
import { Toaster } from "@/components/ui/sonner";
import { TooltipProvider } from "@/components/ui/tooltip";
import { useInstances } from "@/hooks/useInstances";
import { overallTone } from "@/lib/status";

const dot = { green: "bg-green-500", yellow: "bg-yellow-500", red: "bg-red-500", gray: "bg-zinc-400" } as const;
const overallLabel = { green: "All running", yellow: "Partly running", red: "Needs attention", gray: "Stopped" } as const;

type Dialog = { kind: "form"; config: InstanceView["config"] | null } | { kind: "logs"; view: InstanceView } | { kind: "settings" } | null;

export default function App() {
  const { views, loaded, error, refresh } = useInstances();
  const [dialog, setDialog] = useState<Dialog>(null);
  const tone = overallTone(views);

  async function bulk(fn: () => Promise<void>, ok: string) {
    try {
      await fn();
      toast.success(ok);
    } catch (e) {
      toast.error(String(e));
    }
    void refresh();
  }

  return (
    <TooltipProvider delayDuration={300}>
      <div className="mx-auto flex min-h-dvh max-w-5xl flex-col gap-4 px-4 py-4">
        <header className="flex flex-wrap items-center gap-3">
          <div className="flex items-center gap-2" role="status" aria-live="polite">
            <span aria-hidden className={`size-3 rounded-full ${dot[tone]}`} />
            <h1 className="text-xl font-semibold">rc-keeper</h1>
            <span className="text-sm text-muted-foreground">{overallLabel[tone]}</span>
          </div>
          <div className="ml-auto flex flex-wrap items-center gap-2">
            <Button variant="outline" size="sm" disabled={views.length === 0} onClick={() => void bulk(api.startAll, "Started all folders")}>
              Start all
            </Button>
            <Button variant="outline" size="sm" disabled={views.length === 0} onClick={() => void bulk(api.stopAll, "Stopped all folders")}>
              Stop all
            </Button>
            <Button variant="ghost" size="icon" aria-label="Settings" onClick={() => setDialog({ kind: "settings" })}>
              <SettingsIcon />
            </Button>
          </div>
        </header>

        {error && (
          <p role="alert" className="text-sm text-destructive">
            {error}
          </p>
        )}

        <main>
          {loaded && views.length === 0 && <EmptyState onAdd={() => setDialog({ kind: "form", config: null })} />}

          {views.length > 0 && (
            <>
              <div className="mb-3 flex justify-end">
                <Button size="sm" onClick={() => setDialog({ kind: "form", config: null })}>
                  <FolderPlus />
                  Add folder
                </Button>
              </div>
              <ul className="grid gap-3 md:grid-cols-2">
                {views.map((v) => (
                  <li key={v.config.id} className="flex">
                    <InstanceCard
                      view={v}
                      onChanged={() => void refresh()}
                      onEdit={() => setDialog({ kind: "form", config: v.config })}
                      onLogs={() => setDialog({ kind: "logs", view: v })}
                    />
                  </li>
                ))}
              </ul>
            </>
          )}
        </main>

        {dialog?.kind === "form" && (
          <InstanceForm
            initial={dialog.config}
            onClose={() => {
              setDialog(null);
              void refresh();
            }}
          />
        )}
        {dialog?.kind === "logs" && <LogsDialog view={dialog.view} onClose={() => setDialog(null)} />}
        {dialog?.kind === "settings" && <SettingsDialog onClose={() => setDialog(null)} />}
      </div>
      <Toaster />
    </TooltipProvider>
  );
}
