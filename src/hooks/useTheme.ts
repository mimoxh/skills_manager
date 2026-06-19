import { useEffect, useMemo, useState } from "react";

export type ThemeMode = "system" | "light" | "dark";
export type ResolvedTheme = "light" | "dark";

const THEME_STORAGE_KEY = "skills-manager.theme";
const DARK_QUERY = "(prefers-color-scheme: dark)";

function isThemeMode(value: string | null): value is ThemeMode {
  return value === "system" || value === "light" || value === "dark";
}

function readStoredTheme(): ThemeMode {
  if (typeof window === "undefined") return "system";
  return isThemeMode(window.localStorage.getItem(THEME_STORAGE_KEY))
    ? window.localStorage.getItem(THEME_STORAGE_KEY) as ThemeMode
    : "system";
}

function systemTheme(): ResolvedTheme {
  if (typeof window === "undefined") return "light";
  return window.matchMedia(DARK_QUERY).matches ? "dark" : "light";
}

export function useTheme() {
  const [themeMode, setThemeModeState] = useState<ThemeMode>(readStoredTheme);
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
  }, [resolvedTheme, themeMode]);

  function setThemeMode(nextMode: ThemeMode) {
    setThemeModeState(nextMode);
    window.localStorage.setItem(THEME_STORAGE_KEY, nextMode);
  }

  return { themeMode, resolvedTheme, setThemeMode };
}
