import { describe, expect, it } from "vitest";
import {
  applyTheme,
  BUILTIN_THEMES,
  DEFAULT_THEME_ID,
  resolveBuiltinTheme,
  THEME_CSS_VARS,
} from "./themes";
import { DEFAULT_THEME_COLORS } from "./themeColors";

describe("built-in themes", () => {
  it("ships Default, Colorblind safe, and High contrast", () => {
    expect(BUILTIN_THEMES.map((t) => t.id)).toEqual([
      "default",
      "colorblind-safe",
      "high-contrast",
    ]);
  });

  it("keeps Default identical to tokens.css / DEFAULT_THEME_COLORS", () => {
    expect(resolveBuiltinTheme("default").colors).toEqual(DEFAULT_THEME_COLORS);
  });

  it("falls back to Default for unknown ids", () => {
    expect(resolveBuiltinTheme("nope").id).toBe(DEFAULT_THEME_ID);
    expect(resolveBuiltinTheme(undefined).id).toBe(DEFAULT_THEME_ID);
  });

  it("uses deuteranopia-friendly pace (not red/green) for Colorblind safe", () => {
    const { colors } = resolveBuiltinTheme("colorblind-safe");
    // Strong amber mid vs deep-blue far (not two oranges); teal fast stays apart.
    expect(colors.fast).toBe("#009e73");
    expect(colors.slow).toBe("#0072b2");
    expect(colors.warning).toBe("#ffbf00");
    expect(colors.fast).not.toBe(DEFAULT_THEME_COLORS.fast);
    expect(colors.slow).not.toBe(DEFAULT_THEME_COLORS.slow);
    expect(colors.warning).not.toBe(colors.slow);
    expect(colors.fast).not.toBe(colors.slow);
    expect(colors.fast).not.toBe(colors.warning);
    // Cand/ref/delta distinct from each other and from pace.
    expect(colors.candidate).toBe("#5ec8ff");
    expect(colors.reference).toBe("#ff7eb6");
    expect(colors.delta).toBe("#f0e442");
    expect(colors.candidate).not.toBe(colors.fast);
    expect(colors.reference).not.toBe(colors.slow);
    expect(colors.reference).not.toBe(colors.warning);
    expect(colors.delta).not.toBe(colors.candidate);
    expect(colors.delta).not.toBe(colors.reference);
    // Chart chrome brighter than default dark borders.
    expect(colors.chartGrid).toBe("#5a6578");
    expect(colors.chartTick).toBe("#c5ced9");
  });

  it("raises text/border contrast for High contrast", () => {
    const { colors } = resolveBuiltinTheme("high-contrast");
    expect(colors.text).toBe("#ffffff");
    expect(colors.bg).toBe("#000000");
    expect(colors.borderStrong).not.toBe(DEFAULT_THEME_COLORS.borderStrong);
  });
});

function mockRoot(): HTMLElement {
  const props = new Map<string, string>();
  const dataset: Record<string, string> = {};
  return {
    dataset,
    style: {
      setProperty(name: string, value: string) {
        props.set(name, value);
      },
      getPropertyValue(name: string) {
        return props.get(name) ?? "";
      },
      removeProperty(name: string) {
        props.delete(name);
      },
    },
  } as unknown as HTMLElement;
}

describe("applyTheme", () => {
  it("sets data-theme and CSS vars on the root", () => {
    const root = mockRoot();
    const colors = applyTheme("colorblind-safe", root);
    expect(root.dataset.theme).toBe("colorblind-safe");
    expect(root.style.getPropertyValue("--color-fast")).toBe("#009e73");
    expect(root.style.getPropertyValue("--color-warning")).toBe("#ffbf00");
    expect(root.style.getPropertyValue("--color-slow")).toBe("#0072b2");
    expect(root.style.getPropertyValue("--pw-pedal-brake")).toBe("#ff8c1a");
    expect(root.style.getPropertyValue("--pw-pedal-throttle")).toBe("#00e5ff");
    expect(root.style.getPropertyValue(THEME_CSS_VARS.fast)).toBe("#009e73");
    expect(colors.fast).toBe("#009e73");
    expect(colors.pedalBrake).not.toBe(colors.pedalThrottle);
  });

  it("does not set overlay Iron Man HUD vars on the root", () => {
    const root = mockRoot();
    applyTheme("high-contrast", root);
    expect(root.style.getPropertyValue("--pw-glow")).toBe("");
    expect(root.style.getPropertyValue("--pw-hero")).toBe("");
    expect(root.style.getPropertyValue("--pw-fast")).toBe("");
  });
});
