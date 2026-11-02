import { Info } from "lucide-react";
import { toast } from "sonner";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { FIELD_HELP, type FieldKey } from "@/help/fieldHelp";
import { openExternal } from "@/lib/openExternal";

interface Props {
  field: FieldKey;
  /** Currently selected value of a select field. Omit for other inputs and for "Unset". */
  value?: string;
}

/** Info icon after a label. Opens a popover with the field's help and an official docs link. */
export function FieldHelp({ field, value }: Props) {
  const help = FIELD_HELP[field];
  const selected = value === undefined ? undefined : help.values?.[value];
  const docUrl = selected?.docUrl ?? help.docUrl;

  return (
    <Popover>
      <PopoverTrigger asChild>
        <button
          type="button"
          aria-label={`About ${help.title}`}
          className="inline-flex size-5 shrink-0 items-center justify-center rounded-full text-muted-foreground hover:text-foreground focus-visible:outline-2 focus-visible:outline-ring"
        >
          <Info className="size-4" aria-hidden />
        </button>
      </PopoverTrigger>
      <PopoverContent>
        <h3 className="mb-1 font-semibold">{help.title}</h3>
        <p className="text-muted-foreground">{help.text}</p>
        {selected && (
          <p className="mt-2">
            <span className="font-medium">{value}: </span>
            {selected.text}
          </p>
        )}
        {help.cliFlag && (
          <p className="mt-2 text-muted-foreground">
            Flag: <code className="break-all rounded bg-muted px-1 py-0.5 text-xs text-foreground">{help.cliFlag}</code>
          </p>
        )}
        <a
          href={docUrl}
          target="_blank"
          rel="noopener noreferrer"
          className="mt-3 inline-block text-sm font-medium underline underline-offset-2 focus-visible:outline-2 focus-visible:outline-ring"
          onClick={(e) => {
            e.preventDefault();
            openExternal(docUrl).catch((err) => toast.error(String(err)));
          }}
        >
          Official docs ↗
        </a>
      </PopoverContent>
    </Popover>
  );
}
