import { FolderPlus } from "lucide-react";
import { Button } from "@/components/ui/button";

export function EmptyState({ onAdd }: { onAdd: () => void }) {
  return (
    <div className="mx-auto flex max-w-md flex-col items-center gap-3 px-4 py-16 text-center">
      <h2 className="text-lg font-semibold">No folders yet</h2>
      <p className="text-sm text-muted-foreground">Add a folder to keep Remote Control running for it.</p>
      <Button onClick={onAdd}>
        <FolderPlus />
        Add folder
      </Button>
      <p className="mt-4 text-xs text-muted-foreground">
        Tip: on servers use the <code className="font-mono">rc-keeper</code> CLI.
      </p>
    </div>
  );
}
