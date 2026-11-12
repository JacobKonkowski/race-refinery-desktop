import type { CSSProperties } from "react";

/** How session type maps onto Δ Best color cutoffs. */
export type SessionKind = "practice" | "qualy" | "tt" | "race" | "other";

export type DeltaBand = "green" | "yellow" | "red";

/** Non-race green / yellow ceilings as percent of the best lap. Race uses 2×. */
export const BASE_GREEN_PCT = 0.2;
export const BASE_YELLOW_PCT = 0.5;

/**
 * Classify an iRacing session type string into a band-policy kind.
 * Practice / Qualy / TT / Other share the base cutoffs; Race doubles them.
 */
export function classifySessionKind(sessionType: string): SessionKind {
  const lower = sessionType.trim().toLowerCase();
  if (!lower) return "other";
  if (lower.includes("qual")) return "qualy";
  if (lower.includes("race")) return "race";
  if (
    lower.includes("time trial") ||
    lower.includes("timetrial") ||
    /(^|[^a-z])tt([^a-z]|$)/.test(lower)
  ) {
    return "tt";
  }
  if (
    lower.includes("practice") ||
    lower.includes("warmup") ||
    lower.includes("warm-up") ||
    lower.includes("test")
  ) {
    return "practice";
  }
  return "other";
}

/** Green / yellow cutoffs (% of best lap time) for a session kind. */
export function deltaColorBands(kind: SessionKind): {
  greenPct: number;
  yellowPct: number;
} {
  const mult = kind === "race" ? 2 : 1;
  return {
    greenPct: BASE_GREEN_PCT * mult,
    yellowPct: BASE_YELLOW_PCT * mult,
  };
}

/**
 * Infer the best-lap time this delta is relative to.
 * `best = lapTime − deltaToBest` when both are present.
 */
export function bestLapMsFromDelta(
  lapTimeMs: number | null | undefined,
  deltaToBestMs: number | null | undefined,
): number | null {
  if (lapTimeMs == null || deltaToBestMs == null) return null;
  const best = lapTimeMs - deltaToBestMs;
  return best > 0 ? best : null;
}

/** Percent of best represented by a positive delta (0 when on/under best). */
export function deltaPctOfBest(
  deltaMs: number,
  bestLapMs: number,
): number {
  if (!(bestLapMs > 0)) return Number.POSITIVE_INFINITY;
  if (deltaMs <= 0) return 0;
  return (deltaMs / bestLapMs) * 100;
}

/**
 * Discrete band for a Δ Best value: green ≤ cutoff, yellow ≤ next, else red.
 * Returns null when delta or best is unknown.
 */
export function classifyDeltaBand(
  deltaMs: number | null | undefined,
  bestLapMs: number | null | undefined,
  sessionType: string,
): DeltaBand | null {
  if (deltaMs == null || bestLapMs == null || !(bestLapMs > 0)) return null;
  const { greenPct, yellowPct } = deltaColorBands(
    classifySessionKind(sessionType),
  );
  const pct = deltaPctOfBest(deltaMs, bestLapMs);
  if (pct <= greenPct) return "green";
  if (pct <= yellowPct) return "yellow";
  return "red";
}

const BAND_COLOR: Record<DeltaBand, string> = {
  green: "var(--color-fast)",
  yellow: "var(--color-warning)",
  red: "var(--color-slow)",
};

/** Inline text color for a Δ Best cell from % of best × session type. */
export function deltaBestStyle(
  deltaMs: number | null | undefined,
  bestLapMs: number | null | undefined,
  sessionType: string,
): CSSProperties | undefined {
  const band = classifyDeltaBand(deltaMs, bestLapMs, sessionType);
  if (!band) return undefined;
  return { color: BAND_COLOR[band] };
}
