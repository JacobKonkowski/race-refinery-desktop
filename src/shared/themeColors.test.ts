import { describe, expect, it, vi } from "vitest";
import {
  DEFAULT_THEME_COLORS,
  getThemeColors,
} from "./themeColors";

describe("getThemeColors", () => {
  it("returns Default-theme fallbacks when document is unavailable", () => {
    const colors = getThemeColors(null);
    expect(colors).toEqual(DEFAULT_THEME_COLORS);
    expect(colors.candidate).toBe("#4aa3ff");
    expect(colors.delta).toBe("#a371f7");
    expect(colors.chartGrid).toBe("#262d3a");
    expect(colors.tireLf).toBe("#f85149");
    expect(colors.pedalBrake).toBe("#ff6b6b");
  });

  it("reads overridden CSS variables from a style source", () => {
    const el = {} as Element;
    const getPropertyValue = vi.fn((name: string) => {
      if (name === "--color-candidate") return "  #112233  ";
      if (name === "--color-chart-grid") return "#abcdef";
      if (name === "--pw-pedal-brake") return "#ff0000";
      return "";
    });

    const previous = globalThis.getComputedStyle;
    Object.defineProperty(globalThis, "getComputedStyle", {
      configurable: true,
      writable: true,
      value: () => ({ getPropertyValue }),
    });

    try {
      const colors = getThemeColors(el);
      expect(colors.candidate).toBe("#112233");
      expect(colors.chartGrid).toBe("#abcdef");
      expect(colors.pedalBrake).toBe("#ff0000");
      expect(colors.reference).toBe(DEFAULT_THEME_COLORS.reference);
    } finally {
      Object.defineProperty(globalThis, "getComputedStyle", {
        configurable: true,
        writable: true,
        value: previous,
      });
    }
  });
});
