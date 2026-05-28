import * as React from "react";
import { Slot } from "@radix-ui/react-slot";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "../../lib/utils";

const buttonVariants = cva(
  "inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-md text-sm font-medium transition-[background,color,border-color,box-shadow,transform] duration-150 disabled:pointer-events-none disabled:opacity-45 active:translate-y-px focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[color-mix(in_srgb,var(--color-accent)_42%,transparent)] focus-visible:ring-offset-2 focus-visible:ring-offset-[var(--color-page)]",
  {
    variants: {
      variant: {
        default: "bg-[var(--color-text)] text-white shadow-sm hover:bg-[var(--color-text)]/90",
        primary: "bg-[var(--color-accent)] text-white shadow-sm hover:bg-[var(--color-accent-hover)]",
        secondary: "border border-[var(--color-border)] bg-[var(--color-surface-raised)] text-[var(--color-text)] shadow-sm hover:border-[var(--color-border-hover)] hover:bg-[var(--color-surface)]",
        ghost: "text-[var(--color-text-secondary)] hover:bg-[var(--color-surface)] hover:text-[var(--color-text)]",
        destructive: "bg-[var(--color-danger)] text-white hover:bg-[var(--color-danger)]/90",
        link: "text-[var(--color-accent)] underline-offset-4 hover:underline",
      },
      size: {
        default: "h-9 px-4 text-[13px]",
        sm: "h-8 rounded-md px-3 text-[12.5px]",
        lg: "h-10 rounded-md px-6 text-sm",
        icon: "h-8 w-8",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "default",
    },
  },
);

export interface ButtonProps
  extends React.ButtonHTMLAttributes<HTMLButtonElement>,
    VariantProps<typeof buttonVariants> {
  asChild?: boolean;
}

const Button = React.forwardRef<HTMLButtonElement, ButtonProps>(
  ({ className, variant, size, asChild = false, ...props }, ref) => {
    const Comp = asChild ? Slot : "button";
    return (
      <Comp
        className={cn(buttonVariants({ variant, size, className }))}
        ref={ref}
        {...props}
      />
    );
  },
);
Button.displayName = "Button";

export { Button, buttonVariants };
