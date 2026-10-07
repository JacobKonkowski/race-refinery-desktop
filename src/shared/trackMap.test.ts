import { describe, expect, it } from "vitest";
import {
  exaggerateLateral,
  hasRacingLine,
  pedalSegments,
  pedalTone,
  pointAt,
  projectSample,
  startFinishSegment,
  toPathD,
  tracePath,
} from "./trackMap";
import type { OutlinePoint, TrackOutline, TrailSample } from "./types";

/** Unit square walked clockwise, one vertex per quarter lap. */
const square: OutlinePoint[] = [
  { pct: 0, x: 0, y: 0 },
  { pct: 0.25, x: 1, y: 0 },
  { pct: 0.5, x: 1, y: 1 },
  { pct: 0.75, x: 0, y: 1 },
];

/** Identity-ish projection: 1 degree of latitude maps to 111320 m. */
const outline: TrackOutline = {
  track: "Test",
  points: square,
  svgPath: "M 0,0 L 1,0 L 1,1 L 0,1 Z",
  coverage: 1,
  sampleCount: 600,
  projection: {
    originLat: 0,
    originLon: 0,
    minX: 0,
    minY: 0,
    scale: 1 / 111_320,
    offsetX: 0,
    offsetY: 0,
  },
};

function sample(over: Partial<TrailSample> = {}): TrailSample {
  return { distPct: 0, throttle: 0, brake: 0, lat: null, lon: null, ...over };
}

describe("pointAt", () => {
  it("returns vertices exactly", () => {
    expect(pointAt(square, 0.25)).toEqual({ x: 1, y: 0 });
    expect(pointAt(square, 0.5)).toEqual({ x: 1, y: 1 });
  });

  it("interpolates along a segment", () => {
    expect(pointAt(square, 0.125)).toEqual({ x: 0.5, y: 0 });
  });

  it("wraps across the finish line", () => {
    expect(pointAt(square, 0.875)).toEqual({ x: 0, y: 0.5 });
    expect(pointAt(square, 1)).toEqual({ x: 0, y: 0 });
    expect(pointAt(square, 1.25)).toEqual({ x: 1, y: 0 });
    expect(pointAt(square, -0.25)).toEqual({ x: 0, y: 1 });
  });

  it("handles degenerate outlines", () => {
    expect(pointAt([], 0.5)).toBeNull();
    expect(pointAt([{ pct: 0, x: 0.5, y: 0.5 }], 0.9)).toEqual({ x: 0.5, y: 0.5 });
  });
});

describe("projectSample", () => {
  it("maps GPS into the outline box", () => {
    // 1 degree east at the equator is one full box width under this scale.
    expect(projectSample(outline, 0, 1)).toEqual({ x: 1, y: 0 });
    // North is up, so increasing latitude decreases y.
    expect(projectSample(outline, 1, 0)).toEqual({ x: 0, y: -1 });
  });

  it("returns null without a stored projection", () => {
    expect(projectSample({ ...outline, projection: null }, 0, 1)).toBeNull();
  });
});

describe("hasRacingLine", () => {
  it("needs both a projection and GPS samples", () => {
    const withGps = [sample({ lat: 0, lon: 0.5 })];
    expect(hasRacingLine(outline, withGps)).toBe(true);
    expect(hasRacingLine(outline, [sample()])).toBe(false);
    expect(hasRacingLine({ ...outline, projection: null }, withGps)).toBe(false);
    expect(hasRacingLine(null, withGps)).toBe(false);
  });
});

describe("exaggerateLateral", () => {
  it("doubles offset from the centerline", () => {
    expect(exaggerateLateral({ x: 0.6, y: 0 }, { x: 0.5, y: 0 })).toEqual({ x: 0.7, y: 0 });
  });

  it("leaves GPS unchanged without a center", () => {
    expect(exaggerateLateral({ x: 0.6, y: 0.1 }, null)).toEqual({ x: 0.6, y: 0.1 });
  });
});

describe("tracePath", () => {
  it("uses GPS when present (on-centerline stays put)", () => {
    const path = tracePath(outline, [
      sample({ distPct: 0, lat: 0, lon: 0 }),
      sample({ distPct: 0.25, lat: 0, lon: 1 }),
    ]);
    expect(path).toEqual([
      { x: 0, y: 0 },
      { x: 1, y: 0 },
    ]);
  });

  it("exaggerates GPS offset from the outline centerline", () => {
    // At distPct 0 the centerline is (0,0); lon=0.25 projects to (0.25, 0).
    // 2× exaggeration → (0.5, 0).
    const path = tracePath(outline, [sample({ distPct: 0, lat: 0, lon: 0.25 })]);
    expect(path).toEqual([{ x: 0.5, y: 0 }]);
  });

  it("falls back to lap distance without GPS", () => {
    const path = tracePath(outline, [sample({ distPct: 0.25 }), sample({ distPct: 0.5 })]);
    expect(path).toEqual([
      { x: 1, y: 0 },
      { x: 1, y: 1 },
    ]);
  });
});

describe("pedalTone", () => {
  it("prefers brake over throttle", () => {
    expect(pedalTone(0.2, 0.9)).toBe("brake");
    expect(pedalTone(0.9, 0.9)).toBe("brake");
  });

  it("reports throttle and coast", () => {
    expect(pedalTone(0.8, 0)).toBe("throttle");
    expect(pedalTone(0.01, 0.01)).toBe("coast");
  });
});

describe("pedalSegments", () => {
  it("splits runs of constant tone and bridges the seam", () => {
    const segments = pedalSegments(outline, [
      sample({ distPct: 0, throttle: 1 }),
      sample({ distPct: 0.25, throttle: 1 }),
      sample({ distPct: 0.5, brake: 1 }),
      sample({ distPct: 0.75, brake: 1 }),
    ]);

    expect(segments.map((s) => s.tone)).toEqual(["throttle", "brake"]);
    // The brake run starts where the throttle run ended, so strokes connect.
    expect(segments[1].points[0]).toEqual(segments[0].points.at(-1));
  });

  it("colors downshift blips by the driver's pedals", () => {
    // Applied throttle spikes to ~0.5 on the blip while the driver is trail braking.
    const blip = { throttle: 0.52, brake: 0.3 };
    const raw = pedalSegments(outline, [
      sample({ distPct: 0, ...blip, throttleRaw: 0, brakeRaw: 0.3 }),
      sample({ distPct: 0.25, ...blip, throttleRaw: 0, brakeRaw: 0.3 }),
    ]);
    expect(raw.map((s) => s.tone)).toEqual(["brake"]);

    // Without raw channels (pre-v5 sessions) the applied values still decide.
    const applied = pedalSegments(outline, [
      sample({ distPct: 0, ...blip }),
      sample({ distPct: 0.25, ...blip, throttleRaw: null, brakeRaw: null }),
    ]);
    expect(applied.map((s) => s.tone)).toEqual(["throttle"]);
  });

  it("drops single-point runs and empty input", () => {
    expect(pedalSegments(outline, [sample()])).toEqual([]);
    expect(pedalSegments(outline, [])).toEqual([]);
  });
});

describe("startFinishSegment", () => {
  it("crosses the start point perpendicular to the outline", () => {
    // Square runs +x from (0,0); perpendicular is vertical through the origin.
    const seg = startFinishSegment(square, 0.1);
    expect(seg).not.toBeNull();
    expect(seg!.x1).toBeCloseTo(0, 5);
    expect(seg!.x2).toBeCloseTo(0, 5);
    expect(Math.abs(seg!.y2 - seg!.y1)).toBeCloseTo(0.2, 5);
    expect((seg!.y1 + seg!.y2) / 2).toBeCloseTo(0, 5);
  });

  it("returns null for empty outlines", () => {
    expect(startFinishSegment([])).toBeNull();
  });
});

describe("toPathD", () => {
  it("emits an open path", () => {
    expect(toPathD([{ x: 0, y: 0 }, { x: 0.5, y: 1 }])).toBe("M 0.0000,0.0000 L 0.5000,1.0000");
  });
});
