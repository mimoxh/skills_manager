import { RefreshCw, Search } from "lucide-react";
import { Button } from "../ui/button";

interface CommandBarProps {
  query: string;
  onQueryChange: (query: string) => void;
  busy: boolean;
  onRefresh: () => void;
}

export function CommandBar({
  query,
  onQueryChange,
  busy,
  onRefresh,
}: CommandBarProps) {
  return (
    <div className="flex shrink-0 items-center gap-3">
      <div className="flex h-11 flex-1 items-center gap-3 rounded-lg border border-[var(--color-border)] bg-[var(--color-surface-raised)] px-3 shadow-sm transition-[border-color,box-shadow] focus-within:border-[var(--color-accent)] focus-within:ring-2 focus-within:ring-[color-mix(in_srgb,var(--color-accent)_18%,transparent)]">
        <Search size={15} className="text-[var(--color-text-tertiary)] shrink-0" />
        <input
          value={query}
          onChange={(e) => onQueryChange(e.target.value)}
          placeholder="搜索 skill、agent 或路径"
          className="h-full flex-1 text-sm text-[var(--color-text)] placeholder:text-[var(--color-text-tertiary)]"
        />
      </div>
      <Button variant="ghost" size="sm" onClick={onRefresh} disabled={busy}>
        <RefreshCw size={14} />
        刷新
      </Button>
    </div>
  );
}
