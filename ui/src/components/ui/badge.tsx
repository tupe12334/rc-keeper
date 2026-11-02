import { cva, type VariantProps } from "class-variance-authority";
import * as React from "react";
import { cn } from "@/lib/utils";

const badgeVariants = cva("inline-flex items-center gap-1.5 rounded-full px-2 py-0.5 text-xs font-medium", {
  variants: {
    tone: {
      green: "bg-green-500/15 text-green-700 dark:text-green-400",
      yellow: "bg-yellow-500/15 text-yellow-800 dark:text-yellow-300",
      red: "bg-red-500/15 text-red-700 dark:text-red-400",
      gray: "bg-muted text-muted-foreground",
    },
  },
  defaultVariants: { tone: "gray" },
});

export interface BadgeProps extends React.HTMLAttributes<HTMLSpanElement>, VariantProps<typeof badgeVariants> {}

export const Badge = ({ className, tone, ...props }: BadgeProps) => (
  <span className={cn(badgeVariants({ tone }), className)} {...props} />
);
