import type { ResolvedTheme, ThemeMode } from "../../hooks/useTheme";

const themeOptions: Array<{ mode: ThemeMode; label: string }> = [
  { mode: "system", label: "系统" },
  { mode: "light", label: "浅色" },
  { mode: "dark", label: "深色" },
];

interface ThemeSwitcherProps {
  collapsed: boolean;
  mode: ThemeMode;
  resolvedTheme: ResolvedTheme;
  onChange: (mode: ThemeMode) => void;
}

export function ThemeSwitcher({ collapsed, mode, resolvedTheme, onChange }: ThemeSwitcherProps) {
  if (collapsed) {
    return (
      <button
        className="theme-switcher-icon"
        onClick={() => onChange(nextThemeMode(mode))}
        title={`主题：${themeLabel(mode)}（当前${resolvedTheme === "dark" ? "深色" : "浅色"}），点击切换`}
        type="button"
      >
        <ThemeIcon mode={mode} resolvedTheme={resolvedTheme} />
      </button>
    );
  }

  return (
    <div className="theme-switcher" aria-label="主题切换">
      <div className="theme-switcher-label">主题</div>
      <div className="theme-switcher-options" role="group" aria-label="主题切换">
        {themeOptions.map((option) => (
          <button
            key={option.mode}
            className={`theme-switcher-option${mode === option.mode ? " active" : ""}`}
            onClick={() => onChange(option.mode)}
            type="button"
            aria-pressed={mode === option.mode}
            title={`${option.label}${option.mode === "system" ? `（当前${resolvedTheme === "dark" ? "深色" : "浅色"}）` : ""}`}
          >
            {option.label}
          </button>
        ))}
      </div>
    </div>
  );
}

function nextThemeMode(mode: ThemeMode): ThemeMode {
  if (mode === "system") return "light";
  if (mode === "light") return "dark";
  return "system";
}

function themeLabel(mode: ThemeMode): string {
  if (mode === "system") return "跟随系统";
  if (mode === "light") return "浅色";
  return "深色";
}

function ThemeIcon({ mode, resolvedTheme }: { mode: ThemeMode; resolvedTheme: ResolvedTheme }) {
  if (mode === "system") {
    return (
      <svg className="icon icon-sm" viewBox="0 0 24 24">
        <rect x="3" y="4" width="18" height="12" rx="2" />
        <line x1="8" y1="20" x2="16" y2="20" />
        <line x1="12" y1="16" x2="12" y2="20" />
      </svg>
    );
  }

  if (resolvedTheme === "dark") {
    return (
      <svg className="icon icon-sm" viewBox="0 0 24 24">
        <path d="M21 12.8A8.5 8.5 0 1 1 11.2 3a6.5 6.5 0 0 0 9.8 9.8z" />
      </svg>
    );
  }

  return (
    <svg className="icon icon-sm" viewBox="0 0 24 24">
      <circle cx="12" cy="12" r="4" />
      <line x1="12" y1="2" x2="12" y2="5" />
      <line x1="12" y1="19" x2="12" y2="22" />
      <line x1="2" y1="12" x2="5" y2="12" />
      <line x1="19" y1="12" x2="22" y2="12" />
    </svg>
  );
}
