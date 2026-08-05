import { useEffect, useMemo, useState } from "react";

export type ThemeMode = "system" | "light" | "dark";
export type ResolvedTheme = "light" | "dark";
export type Palette = "modern" | "parchment";

const THEME_STORAGE_KEY = "skills-manager.theme";
const PALETTE_STORAGE_KEY = "skills-manager.palette";
const DARK_QUERY = "(prefers-color-scheme: dark)";

function isThemeMode(value: string | null): value is ThemeMode {
  return value === "system" || value === "light" || value === "dark";
}

function isPalette(value: string | null): value is Palette {
  return value === "modern" || value === "parchment";
}

function readStoredTheme(): ThemeMode {
  if (typeof window === "undefined") return "system";
  const stored = window.localStorage.getItem(THEME_STORAGE_KEY);
  return isThemeMode(stored) ? stored : "system";
}

function readStoredPalette(): Palette {
  if (typeof window === "undefined") return "modern";
  const stored = window.localStorage.getItem(PALETTE_STORAGE_KEY);
  return isPalette(stored) ? stored : "modern";
}

function systemTheme(): ResolvedTheme {
  if (typeof window === "undefined") return "light";
  return window.matchMedia(DARK_QUERY).matches ? "dark" : "light";
}

export function useTheme() {
  const [themeMode, setThemeModeState] = useState<ThemeMode>(readStoredTheme);
  const [palette, setPaletteState] = useState<Palette>(readStoredPalette);
  const [systemResolvedTheme, setSystemResolvedTheme] = useState<ResolvedTheme>(systemTheme);

  const resolvedTheme = useMemo<ResolvedTheme>(() => {
    return themeMode === "system" ? systemResolvedTheme : themeMode;
  }, [systemResolvedTheme, themeMode]);

  useEffect(() => {
    const media = window.matchMedia(DARK_QUERY);
    const updateSystemTheme = () => setSystemResolvedTheme(media.matches ? "dark" : "light");

    updateSystemTheme();
    media.addEventListener("change", updateSystemTheme);
    return () => media.removeEventListener("change", updateSystemTheme);
  }, []);

  useEffect(() => {
    document.documentElement.dataset.theme = resolvedTheme;
    document.documentElement.dataset.themeMode = themeMode;
    document.documentElement.dataset.palette = palette;
  }, [resolvedTheme, themeMode, palette]);

  function setThemeMode(nextMode: ThemeMode) {
    setThemeModeState(nextMode);
    window.localStorage.setItem(THEME_STORAGE_KEY, nextMode);
  }

  function setPalette(nextPalette: Palette) {
    setPaletteState(nextPalette);
    window.localStorage.setItem(PALETTE_STORAGE_KEY, nextPalette);
  }

  return { themeMode, resolvedTheme, palette, setThemeMode, setPalette };
}
