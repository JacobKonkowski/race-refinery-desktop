import type {
  ConsistencyPoint,
  CornerConsistency,
  CornerDelta,
  LapSummary,
} from "../../shared/types";

/** A lap this far off the reference through one corner was off track or spun. */
export const OFF_SCALE_MS = 3000;

/** A consistency corner further than this from a comparison corner's apex is a different corner. */
const APEX_MATCH_PCT = 0.01;
const MIN_START_PCT = 0.05;
const MIN_END_PCT = 0.95;

/** A complete, non-pit lap from the given sub-session: fair to compare brake points on. */
export function isCleanLap(lap: LapSummary, sessionNum: number): boolean {
  return (
    lap.sessionNum === sessionNum &&
    !lap.onPitRoadStart &&
    !lap.onPitRoadEnd &&
    lap.lapDistPctMin != null &&
    lap.lapDistPctMin <= MIN_START_PCT &&
    lap.lapDistPctMax != null &&
    lap.lapDistPctMax >= MIN_END_PCT
  );
}

/**
 * Points worth plotting vs. laps so far off (a spin, an off) that they would
 * squash every other point into a line. Points without a brake point can't be
 * plotted either way.
 */
export function plottablePoints(points: ConsistencyPoint[]): {
  shown: ConsistencyPoint[];
  offScale: number;
} {
  const braked = points.filter((p) => p.brakeOffsetM != null);
  const shown = braked.filter((p) => Math.abs(p.timeDeltaMs) <= OFF_SCALE_MS);
  return { shown, offScale: braked.length - shown.length };
}

/**
 * The consistency entry for a comparison corner. Matched by apex rather than
 * number: a comparison covers only the distance both laps share, so its
 * numbering can shift against the reference lap's full range.
 */
export function matchConsistency(
  corner: CornerDelta,
  all: CornerConsistency[],
): CornerConsistency | null {
  let best: CornerConsistency | null = null;
  for (const c of all) {
    const d = Math.abs(c.apexPct - corner.apexPct);
    if (d <= APEX_MATCH_PCT && (!best || d < Math.abs(best.apexPct - corner.apexPct))) {
      best = c;
    }
  }
  return best;
}
