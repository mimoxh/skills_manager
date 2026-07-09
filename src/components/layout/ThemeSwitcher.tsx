import { useState } from "react";
import type { ResolvedTheme, ThemeMode } from "../../hooks/useTheme";

// 按照跟随系统、浅色、深色的顺序排列
const themeOptions: Array<{ mode: ThemeMode; label: string }> = [
  { mode: "system", label: "跟随系统" },
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
  // 收起态：点击展开/收起
  const [expanded, setExpanded] = useState(false);

  if (!collapsed) {
    // 展开态：三个图标横向排列，当前选中高亮
    return (
      <div className="theme-icon-row" role="group" aria-label="主题切换">
        {themeOptions.map((option) => (
          <button
            key={option.mode}
            className={`theme-icon-btn${mode === option.mode ? " active" : ""}`}
            onClick={() => onChange(option.mode)}
            type="button"
            aria-pressed={mode === option.mode}
            title={`${option.label}${option.mode === "system" ? `（当前${resolvedTheme === "dark" ? "深色" : "浅色"}）` : ""}`}
          >
            <ThemeIcon mode={option.mode} resolvedTheme={resolvedTheme} />
          </button>
        ))}
      </div>
    );
  }

  // 收起态：当前图标 + 点击展开其他
  const currentIndex = themeOptions.findIndex((o) => o.mode === mode);
  const aboveOptions = themeOptions.slice(0, currentIndex);
  const belowOptions = themeOptions.slice(currentIndex + 1);

  const handleSelect = (selectedMode: ThemeMode) => {
    if (selectedMode === mode) {
      setExpanded((prev) => !prev);
    } else {
      onChange(selectedMode);
      setExpanded(false);
    }
  };

  return (
    <div className="theme-icon-switcher">
      {expanded &&
        aboveOptions.map((option, i) => (
          <button
            key={option.mode}
            className="theme-icon-option"
            onClick={() => handleSelect(option.mode)}
            type="button"
            title={option.label}
            style={{ animationDelay: `${i * 50}ms` }}
          >
            <ThemeIcon mode={option.mode} resolvedTheme={resolvedTheme} />
          </button>
        ))}

      <button
        className="theme-icon-current"
        onClick={() => handleSelect(mode)}
        type="button"
        title={`${themeOptions[currentIndex].label}，点击${expanded ? "收起" : "展开"}`}
      >
        <ThemeIcon mode={mode} resolvedTheme={resolvedTheme} />
      </button>

      {expanded &&
        belowOptions.map((option, i) => (
          <button
            key={option.mode}
            className="theme-icon-option"
            onClick={() => handleSelect(option.mode)}
            type="button"
            title={option.label}
            style={{ animationDelay: `${(aboveOptions.length + i) * 50}ms` }}
          >
            <ThemeIcon mode={option.mode} resolvedTheme={resolvedTheme} />
          </button>
        ))}
    </div>
  );
}

function ThemeIcon({ mode, resolvedTheme }: { mode: ThemeMode; resolvedTheme: ResolvedTheme }) {
  // 跟随系统 - 显示器图标（半明半暗）
  if (mode === "system") {
    return (
      <svg className="icon" viewBox="0 0 24 24">
        <rect x="2" y="3" width="20" height="14" rx="2" />
        <line x1="8" y1="21" x2="16" y2="21" />
        <line x1="12" y1="17" x2="12" y2="21" />
        <line x1="12" y1="3" x2="12" y2="17" />
        <path d="M2 3h10v14H2" fill="currentColor" opacity="0.15" />
      </svg>
    );
  }

  // 浅色 - 太阳图标
  if (mode === "light") {
    return (
      <svg className="icon" viewBox="0 0 24 24">
        <circle cx="12" cy="12" r="5" />
        <line x1="12" y1="1" x2="12" y2="3" />
        <line x1="12" y1="21" x2="12" y2="23" />
        <line x1="4.22" y1="4.22" x2="5.64" y2="5.64" />
        <line x1="18.36" y1="18.36" x2="19.78" y2="19.78" />
        <line x1="1" y1="12" x2="3" y2="12" />
        <line x1="21" y1="12" x2="23" y2="12" />
        <line x1="4.22" y1="19.78" x2="5.64" y2="18.36" />
        <line x1="18.36" y1="5.64" x2="19.78" y2="4.22" />
      </svg>
    );
  }

  // 深色 - 月亮图标
  return (
    <svg className="icon" viewBox="0 0 24 24">
      <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" />
    </svg>
  );
}
