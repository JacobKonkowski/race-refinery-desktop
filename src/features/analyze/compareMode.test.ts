import { describe, expect, it } from "vitest";
import { compareMode } from "./compareMode";
import type { LapSummary } from "../../shared/types";

function lap(id: number): LapSummary {
  return {
    id,
    sessionNum: 1,
    sessionType: "Practice",
    iracingLap: id,
    lapNumber: id,
    lapTimeMs: 90_000,
    deltaBestOk: true,
    deltaSessionBestOk: true,
    onPitRoadStart: false,
    onPitRoadEnd: false,
    lapDistPctMin: 0,
    lapDistPctMax: 1,
    paceEligible: true,
    fuelStart: null,
    fuelUsed: null,
    avgSpeed: null,
    lfTemp: null,
    rfTemp: null,
    lrTemp: null,
    rrTemp: null,
    lfPressure: null,
    rfPressure: null,
    lrPressure: null,
    rrPressure: null,
    hasTraffic: false,
    sectors: [],
    deltaToBestMs: null,
  };
}

describe("compareMode", () => {
  it("is empty without a reference", () => {
    expect(compareMode(null, null)).toBe("empty");
    expect(compareMode(lap(1), null)).toBe("empty");
  });

  it("is solo when only the reference is set", () => {
    expect(compareMode(null, lap(2))).toBe("solo");
  });

  it("is same when candidate equals reference", () => {
    expect(compareMode(lap(2), lap(2))).toBe("same");
  });

  it("is compare when both differ", () => {
    expect(compareMode(lap(1), lap(2))).toBe("compare");
  });
});
