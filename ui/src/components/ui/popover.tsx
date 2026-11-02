import { Popover as PopoverPrimitive } from "radix-ui";
import { cn } from "@/lib/utils";

export const Popover = PopoverPrimitive.Root;
export const PopoverTrigger = PopoverPrimitive.Trigger;

export const PopoverContent = ({ className, align = "start", sideOffset = 6, ...props }: PopoverPrimitive.PopoverContentProps) => (
  <PopoverPrimitive.Portal>
    <PopoverPrimitive.Content
      align={align}
      sideOffset={sideOffset}
      collisionPadding={8}
      className={cn("z-[70] w-72 max-w-[calc(100vw-2rem)] rounded-md border border-border bg-background p-3 text-sm text-foreground shadow-md focus:outline-none", className)}
      {...props}
    />
  </PopoverPrimitive.Portal>
);
