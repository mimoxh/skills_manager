import { Bot, Gauge, Settings, Sparkles } from "lucide-react";
import { cn } from "../../lib/utils";
import type { View } from "../../App";

const navItems: Array<{ id: View; label: string; icon: React.ReactNode }> = [
  { id: "overview", label: "概览", icon: <Gauge size={16} /> },
  { id: "skills", label: "Skills", icon: <Sparkles size={16} /> },
  { id: "agents", label: "Agents", icon: <Bot size={16} /> },
  { id: "settings", label: "设置", icon: <Settings size={16} /> },
];

function shortPath(path: string) {
  if (!path) return "未设置";
  return path.length > 36 ? `...${path.slice(-33)}` : path;
}

interface SidebarProps {
  view: View;
  repository: string;
  onNavigate: (view: View) => void;
}

export function Sidebar({ view, repository, onNavigate }: SidebarProps) {
  return (
    <aside className="flex w-64 shrink-0 flex-col border-r border-[var(--color-border)] bg-[rgb(247_247_245_/_0.92)] px-3 py-4">
      <nav className="flex flex-col gap-1">
        {navItems.map((item) => (
          <button
            key={item.id}
            className={cn(
              "relative flex min-h-11 items-center gap-3 rounded-md px-3 text-[13px] font-medium transition-colors duration-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[color-mix(in_srgb,var(--color-accent)_28%,transparent)]",
              view === item.id
                ? "bg-[var(--color-surface-raised)] text-[var(--color-text)] shadow-sm"
                : "text-[var(--color-text-secondary)] hover:bg-[var(--color-surface-raised)] hover:text-[var(--color-text)]",
            )}
            onClick={() => onNavigate(item.id)}
            type="button"
          >
            {view === item.id && (
              <div className="absolute left-0 top-1/2 h-5 w-0.5 -translate-y-1/2 rounded-r bg-[var(--color-accent)]" />
            )}
            {item.icon}
            <span>{item.label}</span>
          </button>
        ))}
      </nav>

      <div className="mt-auto rounded-lg border border-[var(--color-border)] bg-[var(--color-surface-raised)] p-3 shadow-sm">
        <span className="text-xs font-medium text-[var(--color-text-secondary)]">主仓库</span>
        <p
          className="mt-1 truncate text-xs leading-relaxed text-[var(--color-text)]"
          title={repository}
        >
          {shortPath(repository)}
        </p>
      </div>
    </aside>
  );
}
