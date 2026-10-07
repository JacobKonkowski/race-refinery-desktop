import { describe, expect, it } from "vitest";
import { isCleanLap, matchConsistency, plottablePoints } from "./cornerConsistency";
import type {
  ConsistencyPoint,
  CornerConsistency,
  CornerDelta,
  LapSummary,
} from "../../shared/types";

function lap(over: Partial<LapSummary> = {}): LapSummary {
  return {
    id: 1,
    sessionNum: 2,
    sessionType: "Practice",
    iracingLap: 3,
    lapNumber: 3,
    lapTimeMs: 90_000,
    deltaBestOk: true,
    deltaSessionBestOk: true,
    onPitRoadStart: false,
    onPitRoadEnd: false,
    lapDistPctMin: 0.001,
    lapDistPctMax: 0.999,
    paceEligible: true,
    fuelStart: null,
    fuelUsed: null,
    avgSpeed: null,
    lfTemp: null,
    rfTemp: null,
    lrTemp: null,
    rrTemp: null,
    sectors: [],
    deltaToBestMs: null,
    ...over,
  };
}

describe("isCleanLap", () => {
  it("accepts complete laps from the same sub-session, timed or not", () => {
    expect(isCleanLap(lap(), 2)).toBe(true);
    expect(isCleanLap(lap({ lapTimeMs: null, paceEligible: false }), 2)).toBe(true);
  });

  it("rejects pit, partial, and other-session laps", () => {
    expect(isCleanLap(lap({ onPitRoadStart: true }), 2)).toBe(false);
    expect(isCleanLap(lap({ onPitRoadEnd: true }), 2)).toBe(false);
    expect(isCleanLap(lap({ lapDistPctMin: 0.3 }), 2)).toBe(false);
    expect(isCleanLap(lap({ lapDistPctMax: 0.8 }), 2)).toBe(false);
    expect(isCleanLap(lap({ lapDistPctMax: null }), 2)).toBe(false);
    expect(isCleanLap(lap(), 1)).toBe(false);
  });
});

describe("plottablePoints", () => {
  const point = (lapId: number, brakeOffsetM: number | null, timeDeltaMs: number) =>
    ({ lapId, brakeOffsetM, timeDeltaMs, minSpeed: null }) satisfies ConsistencyPoint;

  it("hides spins and laps without a brake point", () => {
    const { shown, offScale } = plottablePoints([
      point(1, 0, 0),
      point(2, -9, 450),
      point(3, 4, 14_000),
      point(4, null, 100),
    ]);
    expect(shown.map((p) => p.lapId)).toEqual([1, 2]);
    expect(offScale).toBe(1);
  });
});

describe("matchConsistency", () => {
  const corner = { apexPct: 0.5 } as CornerDelta;
  const entry = (number: number, apexPct: number): CornerConsistency => ({
    number,
    apexPct,
    brakeSpreadM: null,
    points: [],
  });

  it("picks the nearest apex regardless of numbering", () => {
    const all = [entry(1, 0.2), entry(2, 0.494), entry(3, 0.503)];
    expect(matchConsistency(corner, all)?.number).toBe(3);
  });

  it("returns null when no apex is close", () => {
    expect(matchConsistency(corner, [entry(1, 0.52)])).toBeNull();
    expect(matchConsistency(corner, [])).toBeNull();
  });
});
