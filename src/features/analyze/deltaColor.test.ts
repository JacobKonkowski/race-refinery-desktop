import { describe, expect, it } from "vitest";
import {
  BASE_GREEN_PCT,
  BASE_YELLOW_PCT,
  bestLapMsFromDelta,
  classifyDeltaBand,
  classifySessionKind,
  deltaBestStyle,
  deltaColorBands,
  deltaPctOfBest,
} from "./deltaColor";

describe("classifySessionKind", () => {
  it("maps practice / warmup / test", () => {
    expect(classifySessionKind("Practice")).toBe("practice");
    expect(classifySessionKind("Open Practice")).toBe("practice");
    expect(classifySessionKind("Warmup")).toBe("practice");
    expect(classifySessionKind("Test")).toBe("practice");
  });

  it("maps qualifying variants", () => {
    expect(classifySessionKind("Qualify")).toBe("qualy");
    expect(classifySessionKind("Qualifying")).toBe("qualy");
    expect(classifySessionKind("Lone Qualifying")).toBe("qualy");
    expect(classifySessionKind("Open Qualify")).toBe("qualy");
  });

  it("maps time trial / TT", () => {
    expect(classifySessionKind("Time Trial")).toBe("tt");
    expect(classifySessionKind("TT")).toBe("tt");
    expect(classifySessionKind("tt session")).toBe("tt");
  });

  it("maps race", () => {
    expect(classifySessionKind("Race")).toBe("race");
    expect(classifySessionKind("Feature Race")).toBe("race");
  });

  it("falls back to other", () => {
    expect(classifySessionKind("")).toBe("other");
    expect(classifySessionKind("Unknown")).toBe("other");
  });
});

describe("deltaColorBands", () => {
  it("uses base cutoffs for practice / qualy / tt / other", () => {
    for (const kind of ["practice", "qualy", "tt", "other"] as const) {
      expect(deltaColorBands(kind)).toEqual({
        greenPct: BASE_GREEN_PCT,
        yellowPct: BASE_YELLOW_PCT,
      });
    }
  });

  it("doubles cutoffs for race", () => {
    expect(deltaColorBands("race")).toEqual({
      greenPct: BASE_GREEN_PCT * 2,
      yellowPct: BASE_YELLOW_PCT * 2,
    });
  });
});

describe("classifyDeltaBand / deltaBestStyle", () => {
  // 90s best lap → 0.2% = 180ms, 0.5% = 450ms; race 2× → 360ms / 900ms
  const best = 90_000;

  it("derives best from lap time and delta", () => {
    expect(bestLapMsFromDelta(90_180, 180)).toBe(90_000);
    expect(bestLapMsFromDelta(null, 180)).toBeNull();
    expect(bestLapMsFromDelta(90_000, null)).toBeNull();
  });

  it("computes percent of best", () => {
    expect(deltaPctOfBest(0, best)).toBe(0);
    expect(deltaPctOfBest(180, best)).toBeCloseTo(0.2);
    expect(deltaPctOfBest(450, best)).toBeCloseTo(0.5);
  });

  it("bands practice at 0.2% / 0.5%", () => {
    expect(classifyDeltaBand(0, best, "Practice")).toBe("green");
    expect(classifyDeltaBand(180, best, "Practice")).toBe("green");
    expect(classifyDeltaBand(181, best, "Practice")).toBe("yellow");
    expect(classifyDeltaBand(450, best, "Practice")).toBe("yellow");
    expect(classifyDeltaBand(451, best, "Practice")).toBe("red");
  });

  it("bands race at 2× cutoffs", () => {
    expect(classifyDeltaBand(360, best, "Race")).toBe("green");
    expect(classifyDeltaBand(361, best, "Race")).toBe("yellow");
    expect(classifyDeltaBand(900, best, "Race")).toBe("yellow");
    expect(classifyDeltaBand(901, best, "Race")).toBe("red");
  });

  it("returns null without delta or best", () => {
    expect(classifyDeltaBand(null, best, "Practice")).toBeNull();
    expect(classifyDeltaBand(100, null, "Practice")).toBeNull();
    expect(deltaBestStyle(null, best, "Practice")).toBeUndefined();
  });

  it("maps bands to theme CSS variables", () => {
    expect(deltaBestStyle(0, best, "Practice")).toEqual({
      color: "var(--color-fast)",
    });
    expect(deltaBestStyle(300, best, "Practice")).toEqual({
      color: "var(--color-warning)",
    });
    expect(deltaBestStyle(1000, best, "Practice")).toEqual({
      color: "var(--color-slow)",
    });
  });
});
