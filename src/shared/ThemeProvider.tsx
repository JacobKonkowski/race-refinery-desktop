import {
  createContext,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import { getSettings, onSettingsChanged } from "./api";
import type { ThemeColors } from "./themeColors";
import {
  applyTheme,
  DEFAULT_THEME_ID,
  onThemeApplied,
  resolveBuiltinTheme,
} from "./themes";

interface ThemeContextValue {
  themeId: string;
  colors: ThemeColors;
}

const ThemeContext = createContext<ThemeContextValue>({
  themeId: DEFAULT_THEME_ID,
  colors: resolveBuiltinTheme(DEFAULT_THEME_ID).colors,
});

/**
 * Loads `themeId` from settings, applies CSS vars on the document, and
 * re-applies when settings change so Analyze charts can re-read the palette.
 */
export function ThemeProvider({ children }: { children: ReactNode }) {
  const [themeId, setThemeId] = useState<string>(DEFAULT_THEME_ID);
  const [colors, setColors] = useState<ThemeColors>(() =>
    resolveBuiltinTheme(DEFAULT_THEME_ID).colors,
  );

  useEffect(() => {
    let cancelled = false;

    const unsubApply = onThemeApplied((id, nextColors) => {
      setThemeId(id);
      setColors(nextColors);
    });

    // Boot already applied the persisted theme; re-sync for StrictMode / late IPC.
    getSettings()
      .then((s) => {
        if (cancelled) return;
        applyTheme(s.themeId || DEFAULT_THEME_ID);
      })
      .catch(() => undefined);

    const unlisten = onSettingsChanged((s) => {
      applyTheme(s.themeId || DEFAULT_THEME_ID);
    });

    return () => {
      cancelled = true;
      unsubApply();
      unlisten.then((fn) => fn()).catch(() => undefined);
    };
  }, []);

  const value = useMemo(() => ({ themeId, colors }), [themeId, colors]);

  return <ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>;
}

/** Active theme id + resolved palette (updates when Appearance changes). */
export function useTheme(): ThemeContextValue {
  return useContext(ThemeContext);
}

/**
 * Palette for Recharts / SVG. Prefer this over {@link getThemeColors} in React
 * so components re-render when the user switches themes.
 */
export function useThemeColors(): ThemeColors {
  return useContext(ThemeContext).colors;
}
