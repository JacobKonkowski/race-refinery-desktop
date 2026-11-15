/**
 * Built-in UI themes and runtime apply helpers.
 *
 * Themes are semantic CSS custom-property maps applied to `documentElement`.
 * Overlay Iron Man HUD vars (`--pw-glow*`, `--pw-hero`, `--pw-fast/slow/warn`
 * on `.pw-widget`) are intentionally excluded — Analyze chrome + map pedals only.
 */

import {
  DEFAULT_THEME_COLORS,
  type ThemeColors,
} from "./themeColors";

export type BuiltinThemeId = "default" | "colorblind-safe" | "high-contrast";

export interface BuiltinTheme {
  id: BuiltinThemeId;
  name: string;
  /** Short blurb for the Settings Appearance picker. */
  description: string;
  colors: ThemeColors;
}

/** CSS custom properties written when a theme is applied (Analyze + pedals). */
export const THEME_CSS_VARS: Record<keyof ThemeColors, string> = {
  bg: "--color-bg",
  surface: "--color-surface",
  surfaceAlt: "--color-surface-alt",
  border: "--color-border",
  borderStrong: "--color-border-strong",
  text: "--color-text",
  textMuted: "--color-text-muted",
  textFaint: "--color-text-faint",
  accent: "--color-accent",
  accentStrong: "--color-accent-strong",
  fast: "--color-fast",
  slow: "--color-slow",
  warning: "--color-warning",
  candidate: "--color-candidate",
  reference: "--color-reference",
  delta: "--color-delta",
  chartGrid: "--color-chart-grid",
  chartTick: "--color-chart-tick",
  chartZero: "--color-chart-zero",
  tooltipBg: "--color-tooltip-bg",
  tooltipBorder: "--color-tooltip-border",
  tireLf: "--color-tire-lf",
  tireRf: "--color-tire-rf",
  tireLr: "--color-tire-lr",
  tireRr: "--color-tire-rr",
  sessionRace: "--color-session-race",
  sessionQualy: "--color-session-qualy",
  sessionPractice: "--color-session-practice",
  sessionTt: "--color-session-tt",
  mapCandidate: "--pw-map-candidate",
  mapReference: "--pw-map-reference",
  pedalBrake: "--pw-pedal-brake",
  pedalThrottle: "--pw-pedal-throttle",
  pedalCoast: "--pw-pedal-coast",
};

/** Extra chrome tokens not exposed via {@link ThemeColors} / charts. */
const SURFACE_HOVER = "--color-surface-hover";

const DEFAULT_SURFACE_HOVER = "#1e2432";

/**
 * Deuteranopia-first palette (Okabe–Ito inspired): strong amber mid-band vs
 * deep-blue far band — not two oranges. Fast is teal so all three pace hues
 * stay separable; cand/ref stay cyan/magenta.
 */
const COLORBLIND_SAFE_COLORS: ThemeColors = {
  bg: "#0b0e14",
  surface: "#12161f",
  surfaceAlt: "#171c28",
  border: "#262d3a",
  borderStrong: "#333c4d",
  text: "#e6edf3",
  textMuted: "#8b95a5",
  textFaint: "#5b6472",
  accent: "#5ec8ff",
  accentStrong: "#1aa3e0",
  // Pace: teal (fast) / strong amber (mid) / deep blue (slow) — high L+hue gaps.
  fast: "#009e73",
  slow: "#0072b2",
  warning: "#ffbf00",
  // Compare series: bright sky / hot pink / yellow — strong vs each other and dark bg.
  candidate: "#5ec8ff",
  reference: "#ff7eb6",
  delta: "#f0e442",
  // Chart chrome: lighter grid/ticks so traces don't disappear into the panel.
  chartGrid: "#5a6578",
  chartTick: "#c5ced9",
  chartZero: "#9aa3b2",
  tooltipBg: "#1a2230",
  tooltipBorder: "#7a8699",
  tireLf: "#0072b2",
  tireRf: "#009e73",
  tireLr: "#ff7eb6",
  tireRr: "#5ec8ff",
  sessionRace: "#0072b2",
  sessionQualy: "#ff7eb6",
  sessionPractice: "#009e73",
  sessionTt: "#ffbf00",
  mapCandidate: "#5ec8ff",
  mapReference: "#ff7eb6",
  pedalBrake: "#0072b2",
  pedalThrottle: "#009e73",
  pedalCoast: "#ffbf00",
};

const COLORBLIND_SURFACE_HOVER = "#1e2432";

/** Stronger text/border contrast; punchy pace colors. */
const HIGH_CONTRAST_COLORS: ThemeColors = {
  bg: "#000000",
  surface: "#0d0d0d",
  surfaceAlt: "#161616",
  border: "#6a6a6a",
  borderStrong: "#9a9a9a",
  text: "#ffffff",
  textMuted: "#d0d0d0",
  textFaint: "#b0b0b0",
  accent: "#66b3ff",
  accentStrong: "#3d8bfd",
  fast: "#00e676",
  slow: "#ff1744",
  warning: "#ffc400",
  candidate: "#66b3ff",
  reference: "#ffc400",
  delta: "#e040fb",
  chartGrid: "#6a6a6a",
  chartTick: "#d0d0d0",
  chartZero: "#b0b0b0",
  tooltipBg: "#0d0d0d",
  tooltipBorder: "#6a6a6a",
  tireLf: "#ff1744",
  tireRf: "#66b3ff",
  tireLr: "#ffc400",
  tireRr: "#00e676",
  sessionRace: "#ff5252",
  sessionQualy: "#e040fb",
  sessionPractice: "#69f0ae",
  sessionTt: "#ffd740",
  mapCandidate: "#66b3ff",
  mapReference: "#ffc400",
  pedalBrake: "#ff5252",
  pedalThrottle: "#69f0ae",
  pedalCoast: "#ffd740",
};

const HIGH_CONTRAST_SURFACE_HOVER = "#2a2a2a";

export const BUILTIN_THEMES: readonly BuiltinTheme[] = [
  {
    id: "default",
    name: "Default",
    description: "Current dark telemetry palette.",
    colors: { ...DEFAULT_THEME_COLORS },
  },
  {
    id: "colorblind-safe",
    name: "Colorblind safe",
    description:
      "Optimized for red–green deficiency; Δ Best uses teal / strong amber / deep blue.",
    colors: COLORBLIND_SAFE_COLORS,
  },
  {
    id: "high-contrast",
    name: "High contrast",
    description: "Stronger text and border contrast for bright or dim displays.",
    colors: HIGH_CONTRAST_COLORS,
  },
] as const;

const BY_ID = new Map(BUILTIN_THEMES.map((t) => [t.id, t]));

export const DEFAULT_THEME_ID: BuiltinThemeId = "default";

/** Resolve a settings theme id to a built-in; unknown ids fall back to Default. */
export function resolveBuiltinTheme(themeId: string | null | undefined): BuiltinTheme {
  if (themeId && BY_ID.has(themeId as BuiltinThemeId)) {
    return BY_ID.get(themeId as BuiltinThemeId)!;
  }
  return BY_ID.get(DEFAULT_THEME_ID)!;
}

function surfaceHoverFor(themeId: BuiltinThemeId): string {
  if (themeId === "colorblind-safe") return COLORBLIND_SURFACE_HOVER;
  if (themeId === "high-contrast") return HIGH_CONTRAST_SURFACE_HOVER;
  return DEFAULT_SURFACE_HOVER;
}

type ThemeListener = (themeId: string, colors: ThemeColors) => void;

const listeners = new Set<ThemeListener>();

/** Subscribe to {@link applyTheme} (used by ThemeProvider for live chart updates). */
export function onThemeApplied(listener: ThemeListener): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

/**
 * Apply a built-in theme to `document.documentElement` via `dataset.theme`
 * and inline CSS variables. Does not touch overlay `.pw-widget` HUD tokens.
 */
export function applyTheme(themeId: string | null | undefined, root?: HTMLElement | null): ThemeColors {
  const theme = resolveBuiltinTheme(themeId);
  const colors = { ...theme.colors };
  const el =
    root ??
    (typeof document !== "undefined" ? document.documentElement : null);
  if (el) {
    el.dataset.theme = theme.id;

    for (const key of Object.keys(THEME_CSS_VARS) as (keyof ThemeColors)[]) {
      el.style.setProperty(THEME_CSS_VARS[key], theme.colors[key]);
    }
    el.style.setProperty(SURFACE_HOVER, surfaceHoverFor(theme.id));
  }

  for (const listener of listeners) listener(theme.id, colors);
  return colors;
}
