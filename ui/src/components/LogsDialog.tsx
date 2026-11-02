import { useEffect, useRef, useState } from "react";
import { api, type InstanceView } from "@/api";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { ScrollArea } from "@/components/ui/scroll-area";

const REFRESH_MS = 1000;

export function LogsDialog({ view, onClose }: { view: InstanceView; onClose: () => void }) {
  const [lines, setLines] = useState<string[]>([]);
  const endRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let alive = true;
    const pull = () =>
      api
        .getLogs(view.config.id)
        .then((l) => alive && setLines(l))
        .catch(() => undefined);
    void pull();
    const timer = setInterval(() => void pull(), REFRESH_MS);
    return () => {
      alive = false;
      clearInterval(timer);
    };
  }, [view.config.id]);

  // Auto-scroll to the newest line on every update.
  useEffect(() => {
    endRef.current?.scrollIntoView({ block: "end" });
  }, [lines]);

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle>Logs: {view.config.name ?? view.config.folder}</DialogTitle>
          <DialogDescription>Last 500 lines. Refreshes every second.</DialogDescription>
        </DialogHeader>
        <ScrollArea className="h-[60vh] min-h-48 rounded-md border border-border bg-muted">
          <pre role="region" tabIndex={0} className="whitespace-pre-wrap break-all p-3 font-mono text-xs leading-relaxed" aria-label="Log output">
            {lines.length === 0 ? "No output yet." : lines.join("\n")}
          </pre>
          <div ref={endRef} />
        </ScrollArea>
      </DialogContent>
    </Dialog>
  );
}
