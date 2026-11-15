import {
  DEFAULT_THEME_COLORS,
  getThemeColors,
} from "../../shared/themeColors";
import { useThemeColors } from "../../shared/ThemeProvider";

export { getThemeColors, useThemeColors };
export type { ThemeColors } from "../../shared/themeColors";

/**
 * Default-theme compare hex (matches `tokens.css`).
 * Prefer {@link useThemeColors} in React UI so Recharts tracks theme switches.
 */
export const CAND_COLOR = DEFAULT_THEME_COLORS.candidate;
export const REF_COLOR = DEFAULT_THEME_COLORS.reference;
export const DELTA_COLOR = DEFAULT_THEME_COLORS.delta;
