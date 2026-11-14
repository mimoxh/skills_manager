import { useEffect, useRef, useState } from "react";
import type { ResolvedTheme, ThemeMode } from "../../hooks/useTheme";
import { THEME_GROUPS, THEME_OPTIONS, themeOption, type Palette, type ThemeOption } from "../../lib/themes";

export function ThemeSwatch({ option, theme }: { option: ThemeOption; theme: ResolvedTheme }) {
  const colors = option.swatch[theme];
  return (
    <span className="theme-swatch" aria-hidden="true">
      {colors.map((color, index) => <i key={index} style={{ background: color }} />)}
    </span>
  );
}

const modeOptions: Array<{ mode: ThemeMode; label: string }> = [
  { mode: "system", label: "跟随系统" },
  { mode: "light", label: "浅色" },
  { mode: "dark", label: "深色" },
];

/** 标题栏上的快捷主题菜单：随时切换主题与明暗模式。 */
export function ThemeSwitcher({
  palette,
  themeMode,
  resolvedTheme,
  onPaletteChange,
  onThemeChange,
}: {
  palette: Palette;
  themeMode: ThemeMode;
  resolvedTheme: ResolvedTheme;
  onPaletteChange: (palette: Palette) => void;
  onThemeChange: (mode: ThemeMode) => void;
}) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  const current = themeOption(palette);

  useEffect(() => {
    if (!open) return;
    function handlePointerDown(event: PointerEvent) {
      if (rootRef.current && !rootRef.current.contains(event.target as Node)) setOpen(false);
    }
    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") setOpen(false);
    }
    document.addEventListener("pointerdown", handlePointerDown);
    document.addEventListener("keydown", handleKeyDown);
    return () => {
      document.removeEventListener("pointerdown", handlePointerDown);
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [open]);

  return (
    <div className="theme-switcher" ref={rootRef}>
      <button
        className="titlebar-btn theme-switcher-trigger"
        type="button"
        title="切换主题"
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={() => setOpen((value) => !value)}
      >
        <span className="theme-switcher-dot" style={{ background: current.swatch[resolvedTheme][1] }} />
        {current.label}
      </button>
      {open && (
        <div className="theme-menu" role="menu">
          {THEME_GROUPS.map((group) => (
            <div key={group}>
              <div className="theme-menu-label">{group}</div>
              {THEME_OPTIONS.filter((option) => option.group === group).map((option) => {
                const active = option.value === palette;
                return (
                  <button
                    key={option.value}
                    type="button"
                    role="menuitemradio"
                    aria-checked={active}
                    className={`theme-menu-item${active ? " active" : ""}`}
                    onClick={() => onPaletteChange(option.value)}
                  >
                    <ThemeSwatch option={option} theme={resolvedTheme} />
                    <span>{option.label}</span>
                    {active && (
                      <svg className="icon theme-menu-check" viewBox="0 0 24 24"><polyline points="20 6 9 17 4 12" /></svg>
                    )}
                  </button>
                );
              })}
            </div>
          ))}
          <div className="theme-menu-sep" />
          <div className="theme-menu-label">明暗模式</div>
          <div className="theme-menu-modes">
            {modeOptions.map((option) => (
              <button
                key={option.mode}
                type="button"
                className={`theme-menu-mode${themeMode === option.mode ? " active" : ""}`}
                aria-pressed={themeMode === option.mode}
                onClick={() => onThemeChange(option.mode)}
              >
                {option.label}
              </button>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
