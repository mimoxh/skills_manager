import * as React from "react";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "../../lib/utils";

const badgeVariants = cva(
  "inline-flex min-h-6 items-center rounded-[var(--radius-full)] border px-2.5 py-0.5 text-[11.5px] font-medium transition-colors",
  {
    variants: {
      variant: {
        default: "border-transparent bg-[var(--color-text)] text-white",
        secondary: "border-[var(--color-border)] bg-[var(--color-surface-raised)] text-[var(--color-text-secondary)]",
        success: "border-[rgba(107,140,66,0.15)] bg-[var(--color-success-light)] text-[var(--color-success)]",
        warning: "border-[rgba(196,125,46,0.15)] bg-[var(--color-warning-light)] text-[var(--color-warning)]",
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
