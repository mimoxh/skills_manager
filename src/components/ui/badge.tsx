import * as React from "react";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "../../lib/utils";

const badgeVariants = cva(
  "inline-flex min-h-6 items-center rounded-md border px-2 py-0.5 text-xs font-medium transition-colors",
  {
    variants: {
      variant: {
        default: "border-transparent bg-[var(--color-text)] text-white",
        secondary: "border-[var(--color-border)] bg-[var(--color-surface)] text-[var(--color-text-secondary)]",
        success: "border-transparent bg-[var(--color-success-light)] text-[var(--color-success)]",
        warning: "border-transparent bg-[var(--color-warning-light)] text-[var(--color-warning)]",
        danger: "border-transparent bg-[var(--color-danger-light)] text-[var(--color-danger)]",
        outline: "text-[var(--color-text-secondary)]",
      },
    },
    defaultVariants: {
      variant: "default",
    },
  },
);

export interface BadgeProps
  extends React.HTMLAttributes<HTMLDivElement>,
    VariantProps<typeof badgeVariants> {}

function Badge({ className, variant, ...props }: BadgeProps) {
  return <div className={cn(badgeVariants({ variant }), className)} {...props} />;
}

export { Badge, badgeVariants };
