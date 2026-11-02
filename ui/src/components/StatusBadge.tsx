import type { State } from "@/api";
import { Badge } from "@/components/ui/badge";
import { stateLabel, stateTone } from "@/lib/status";

const dot = { green: "bg-green-500", yellow: "bg-yellow-500", red: "bg-red-500", gray: "bg-zinc-400" } as const;

export function StatusBadge({ state, nextRetrySecs = null }: { state: State; nextRetrySecs?: number | null }) {
  const tone = stateTone(state);
  return (
    <Badge tone={tone} data-state={state}>
      <span aria-hidden className={`size-2 rounded-full ${dot[tone]}`} />
      {stateLabel(state, nextRetrySecs)}
    </Badge>
  );
}
