import {
  DEFAULT_THEME_COLORS,
  getThemeColors,
} from "../../shared/themeColors";

export { getThemeColors };
export type { ThemeColors } from "../../shared/themeColors";

/**
 * Default-theme compare hex (matches `tokens.css`).
 * Prefer {@link getThemeColors} in UI so Recharts tracks the active palette.
 */
export const CAND_COLOR = DEFAULT_THEME_COLORS.candidate;
export const REF_COLOR = DEFAULT_THEME_COLORS.reference;
export const DELTA_COLOR = DEFAULT_THEME_COLORS.delta;
