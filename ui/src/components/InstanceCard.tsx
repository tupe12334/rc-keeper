import { ExternalLink, Pencil, Play, RotateCw, ScrollText, Square, Trash2 } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";
import { api, type InstanceView } from "@/api";
import { StatusBadge } from "@/components/StatusBadge";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { formatUptime, shortenPath, stateHelp } from "@/lib/status";
import { openExternal } from "@/lib/openExternal";

interface Props {
  view: InstanceView;
  onEdit: () => void;
  onLogs: () => void;
  onChanged: () => void;
}

export function InstanceCard({ view, onEdit, onLogs, onChanged }: Props) {
  const { config: cfg, status: st } = view;
  const [confirmRemove, setConfirmRemove] = useState(false);
  const title = cfg.name ?? cfg.folder.split(/[\\/]/).filter(Boolean).pop() ?? cfg.folder;
  const running = st.state === "running" || st.state === "starting" || st.state === "restarting";

  // Runs an API call, shows a toast on failure, and refreshes the list.
  async function act(fn: () => Promise<unknown>) {
    try {
      await fn();
    } catch (e) {
      toast.error(String(e));
    }
    onChanged();
  }

  const help = stateHelp[st.state];

  return (
    <Card role="group" className="flex w-full flex-col gap-3 p-4" aria-label={`Folder ${title}`}>
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <h2 className="truncate text-base font-semibold">{title}</h2>
          <p className="truncate font-mono text-xs text-muted-foreground" title={cfg.folder}>
            {shortenPath(cfg.folder)}
          </p>
        </div>
        <StatusBadge state={st.state} nextRetrySecs={st.nextRetrySecs} />
      </div>

      {help && <p className="text-sm text-muted-foreground">{help}</p>}

      <dl className="grid grid-cols-3 gap-2 text-sm">
        <div>
          <dt className="text-xs text-muted-foreground">Sessions</dt>
          <dd>{st.sessions}</dd>
        </div>
        <div>
          <dt className="text-xs text-muted-foreground">Uptime</dt>
          <dd>{formatUptime(st.uptimeSecs)}</dd>
        </div>
        <div>
          <dt className="text-xs text-muted-foreground">Restarts</dt>
          <dd>{st.restarts}</dd>
        </div>
      </dl>

      {st.lastError && (
        <p className="break-words rounded-md bg-destructive/10 p-2 font-mono text-xs text-destructive">{st.lastError}</p>
      )}

      {st.sessionUrls.length > 0 && (
        <ul className="flex flex-col gap-1 text-sm">
          {st.sessionUrls.map((url) => (
            <li key={url}>
              <button type="button" onClick={() => void openExternal(url).catch((e: unknown) => toast.error(String(e)))} className="inline-flex max-w-full items-center gap-1 truncate text-left underline underline-offset-2">
                <ExternalLink className="size-3.5 shrink-0" aria-hidden />
                <span className="truncate">{url}</span>
              </button>
            </li>
          ))}
        </ul>
      )}

      <div className="flex flex-wrap items-center gap-1 border-t border-border pt-3">
        <IconAction label={running ? "Stop" : "Start"} onClick={() => void act(() => (running ? api.stopInstance(cfg.id) : api.startInstance(cfg.id)))}>
          {running ? <Square /> : <Play />}
        </IconAction>
        <IconAction label="Restart" onClick={() => void act(() => api.restartInstance(cfg.id))}>
          <RotateCw />
        </IconAction>
        <IconAction label="Edit" onClick={onEdit}>
          <Pencil />
        </IconAction>
        <IconAction label="Logs" onClick={onLogs}>
          <ScrollText />
        </IconAction>
        <div className="ml-auto">
          <IconAction label="Remove" destructive onClick={() => setConfirmRemove(true)}>
            <Trash2 />
          </IconAction>
        </div>
      </div>

      <Dialog open={confirmRemove} onOpenChange={setConfirmRemove}>
        <DialogContent className="max-w-sm">
          <DialogHeader>
            <DialogTitle>Remove {title}?</DialogTitle>
            <DialogDescription>The remote control session for this folder stops. The folder itself is not touched.</DialogDescription>
          </DialogHeader>
          <div className="flex justify-end gap-2">
            <Button variant="outline" onClick={() => setConfirmRemove(false)}>
              Cancel
            </Button>
            <Button
              variant="destructive"
              onClick={() => {
                setConfirmRemove(false);
                void act(() => api.removeInstance(cfg.id));
              }}
            >
              Remove
            </Button>
          </div>
        </DialogContent>
      </Dialog>
    </Card>
  );
}

function IconAction({
  label,
  onClick,
  destructive,
  children,
}: {
  label: string;
  onClick: () => void;
  destructive?: boolean;
  children: React.ReactNode;
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button variant="ghost" size="icon" aria-label={label} onClick={onClick} className={destructive ? "text-destructive" : undefined}>
          {children}
        </Button>
      </TooltipTrigger>
      <TooltipContent>{label}</TooltipContent>
    </Tooltip>
  );
}
