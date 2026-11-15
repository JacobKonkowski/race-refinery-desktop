/**
 * Theme palette for Recharts / SVG and other JS color props.
 * CSS custom properties are the source of truth (`tokens.css`); this helper
 * resolves them from computed styles so chart libs get concrete color strings.
 */

export type ThemeColors = {
  bg: string;
  surface: string;
  surfaceAlt: string;
  border: string;
  borderStrong: string;
  text: string;
  textMuted: string;
  textFaint: string;
  accent: string;
  accentStrong: string;
  fast: string;
  slow: string;
  warning: string;
  candidate: string;
  reference: string;
  delta: string;
  chartGrid: string;
  chartTick: string;
  chartZero: string;
  tooltipBg: string;
  tooltipBorder: string;
  tireLf: string;
  tireRf: string;
  tireLr: string;
  tireRr: string;
  sessionRace: string;
  sessionQualy: string;
  sessionPractice: string;
  sessionTt: string;
  mapCandidate: string;
  mapReference: string;
  pedalBrake: string;
  pedalThrottle: string;
  pedalCoast: string;
};

/** Fallback hex matching `:root` in `tokens.css` (Default dark theme). */
export const DEFAULT_THEME_COLORS: ThemeColors = {
  bg: "#0b0e14",
  surface: "#12161f",
  surfaceAlt: "#171c28",
  border: "#262d3a",
  borderStrong: "#333c4d",
  text: "#e6edf3",
  textMuted: "#8b95a5",
  textFaint: "#5b6472",
  accent: "#4aa3ff",
  accentStrong: "#1f6feb",
  fast: "#3fb950",
  slow: "#f85149",
  warning: "#d29922",
  candidate: "#4aa3ff",
  reference: "#d29922",
  delta: "#a371f7",
  chartGrid: "#262d3a",
  chartTick: "#8b95a5",
  chartZero: "#5b6472",
  tooltipBg: "#12161f",
  tooltipBorder: "#262d3a",
  tireLf: "#f85149",
  tireRf: "#4aa3ff",
  tireLr: "#d29922",
  tireRr: "#3fb950",
  sessionRace: "#e5484d",
  sessionQualy: "#a855f7",
  sessionPractice: "#46a758",
  sessionTt: "#e6b800",
  mapCandidate: "#4aa3ff",
  mapReference: "#d29922",
  pedalBrake: "#ff6b6b",
  pedalThrottle: "#5dffa8",
  pedalCoast: "#ffb347",
};

const CSS_VARS: Record<keyof ThemeColors, string> = {
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

/**
 * Read the active theme palette from computed CSS variables.
 * Falls back to {@link DEFAULT_THEME_COLORS} when DOM styles are unavailable
 * (unit tests) or a property is missing.
 */
export function getThemeColors(el?: Element | null): ThemeColors {
  const root =
    el ??
    (typeof document !== "undefined" ? document.documentElement : null);
  if (!root || typeof getComputedStyle === "undefined") {
    return { ...DEFAULT_THEME_COLORS };
  }
  const style = getComputedStyle(root);
  const out = { ...DEFAULT_THEME_COLORS };
  for (const key of Object.keys(CSS_VARS) as (keyof ThemeColors)[]) {
    const value = style.getPropertyValue(CSS_VARS[key]).trim();
    if (value) out[key] = value;
  }
  return out;
}
