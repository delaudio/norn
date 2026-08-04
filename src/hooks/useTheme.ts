import { useCallback, useEffect, useState } from "react";
import { readMigratedStorageValue } from "@/lib/storageMigration";

type Theme = "light" | "dark";
const STORAGE_KEY = "norn.theme.v1";
const LEGACY_STORAGE_KEY = "lachesi.theme";

function initialTheme(): Theme {
  if (typeof localStorage !== "undefined") {
    const stored = readMigratedStorageValue(
      localStorage,
      STORAGE_KEY,
      LEGACY_STORAGE_KEY,
      (value) => (value === "light" || value === "dark" ? value : null),
    );
    if (stored === "light" || stored === "dark") return stored;
  }
  return "dark";
}

/** Owns the active color theme and reflects it onto `<html data-theme>`. */
export function useTheme() {
  const [theme, setTheme] = useState<Theme>(initialTheme);

  useEffect(() => {
    document.documentElement.setAttribute("data-theme", theme);
    try {
      localStorage.setItem(STORAGE_KEY, theme);
    } catch {
      // ignore storage failures (private mode, etc.)
    }
  }, [theme]);

  const toggle = useCallback(() => {
    setTheme((t) => (t === "dark" ? "light" : "dark"));
  }, []);

  return { theme, toggle, setTheme };
}
