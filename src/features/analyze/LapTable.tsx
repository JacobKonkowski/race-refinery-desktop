import { Fragment, useMemo, type CSSProperties } from "react";
import type { LapSummary } from "../../shared/types";
import {
  formatDelta,
  formatLapTime,
  formatLiters,
  formatTemp,
} from "../../shared/format";

interface Props {
  laps: LapSummary[];
  candidateLapId: number | null;
  referenceLapId: number | null;
  onSelectCandidate: (id: number) => void;
  onSelectReference: (id: number) => void;
  /** When true, hide non-pace-eligible laps (candidate/reference always kept). */
  paceOnly?: boolean;
  /** Show tire temperature columns (secondary density). */
  showTireTemps?: boolean;
}

interface Group {
  sessionNum: number;
  sessionType: string;
  laps: LapSummary[];
}

function groupBySubsession(laps: LapSummary[]): Group[] {
  const groups: Group[] = [];
  for (const lap of laps) {
    let group = groups.find((g) => g.sessionNum === lap.sessionNum);
    if (!group) {
      group = { sessionNum: lap.sessionNum, sessionType: lap.sessionType, laps: [] };
      groups.push(group);
    }
    group.laps.push(lap);
  }
  return groups;
}

/** Best pace-eligible sector times within a sub-session, keyed by sectorNum. */
function bestSectors(laps: LapSummary[]): Map<number, number> {
  const best = new Map<number, number>();
  for (const lap of laps) {
    if (!lap.paceEligible) continue;
    for (const s of lap.sectors) {
      const prev = best.get(s.sectorNum);
      if (prev == null || s.timeMs < prev) best.set(s.sectorNum, s.timeMs);
    }
  }
  return best;
}

function sectorClass(
  timeMs: number | undefined,
  bestMs: number | undefined,
  paceEligible: boolean,
): string {
  if (timeMs == null || bestMs == null || !paceEligible) return "num";
  if (Math.abs(timeMs - bestMs) < 1) return "num fast";
  if (timeMs > bestMs) return "num slow";
  return "num";
}

/**
 * Sorted Δ Best values of pace-eligible laps in a sub-session. The gradient is
 * rank-based against these so one slow lap doesn't push everything else to
 * green. Non-eligible laps (out-laps, offs) are excluded from the scale and
 * slot in wherever their delta falls (clamping to full red when slower).
 */
function paceDeltaScale(laps: LapSummary[]): number[] {
  return laps
    .filter((l) => l.paceEligible && l.deltaToBestMs != null)
    .map((l) => l.deltaToBestMs as number)
    .sort((a, b) => a - b);
}

/** Green (fastest) → yellow → red (slowest) text color for a Δ Best cell. */
function deltaGradientStyle(
  deltaMs: number | null,
  scale: number[],
): CSSProperties | undefined {
  if (deltaMs == null) return undefined;
  let t = 0;
  if (scale.length > 1) {
    const faster = scale.filter((d) => d < deltaMs - 0.5).length;
    t = Math.min(faster / (scale.length - 1), 1);
  } else if (scale.length === 1 && deltaMs > scale[0] + 0.5) {
    t = 1;
  }
  return {
    color: `color-mix(in hsl, var(--color-slow) ${(t * 100).toFixed(1)}%, var(--color-fast))`,
  };
}

/** Flag cell reflecting the sim's own pace judgement. */
function OkFlag({ lap }: { lap: LapSummary }) {
  if (lap.paceEligible)
    return (
      <span
        className="flag ok"
        title="Pace-eligible: both _OK flags true, official lap time, and at least 95% of the lap recorded"
      >
        ✓
      </span>
    );
  if (lap.deltaBestOk === null && lap.deltaSessionBestOk === null)
    return <span className="flag unknown" title="_OK channel not in this IBT">?</span>;
  return <span className="flag no" title="Sim marked this lap's delta invalid">✗</span>;
}

function PitCell({ lap }: { lap: LapSummary }) {
  if (!lap.onPitRoadStart && !lap.onPitRoadEnd) return <span className="muted">—</span>;
  return (
    <span style={{ display: "inline-flex", gap: 4 }}>
      {lap.onPitRoadStart ? (
        <span className="pill pit" title="Started on pit road">out</span>
      ) : null}
      {lap.onPitRoadEnd ? (
        <span className="pill pit" title="Ended on pit road">in</span>
      ) : null}
    </span>
  );
}

export function LapTable({
  laps,
  candidateLapId,
  referenceLapId,
  onSelectCandidate,
  onSelectReference,
  paceOnly = false,
  showTireTemps = false,
}: Props) {
  const visibleLaps = useMemo(() => {
    if (!paceOnly) return laps;
    return laps.filter(
      (l) =>
        l.paceEligible || l.id === candidateLapId || l.id === referenceLapId,
    );
  }, [laps, paceOnly, candidateLapId, referenceLapId]);

  const groups = useMemo(() => groupBySubsession(visibleLaps), [visibleLaps]);
  const maxSector = useMemo(
    () => visibleLaps.reduce((m, l) => Math.max(m, ...l.sectors.map((s) => s.sectorNum), 0), 0),
    [visibleLaps],
  );
  const sectorNums = Array.from({ length: maxSector }, (_, i) => i + 1);
  // Lap, Time, Δ Best, sectors…, OK, Pit, Fuel[, temps]
  const colSpan = 3 + sectorNums.length + 2 + 1 + (showTireTemps ? 4 : 0);

  return (
    <div className="table-scroll">
      <table className="data-table">
        <thead>
          <tr>
            <th>Lap</th>
            <th className="num">Time</th>
            <th className="num">Δ Best</th>
            {sectorNums.map((n) => (
              <th key={n} className="num">
                S{n}
              </th>
            ))}
            <th>OK</th>
            <th>Pit</th>
            <th className="num">Fuel</th>
            {showTireTemps ? (
              <>
                <th className="num">LF</th>
                <th className="num">RF</th>
                <th className="num">LR</th>
                <th className="num">RR</th>
              </>
            ) : null}
          </tr>
        </thead>
        <tbody>
          {groups.map((group) => {
            const best = bestSectors(group.laps);
            const deltaScale = paceDeltaScale(group.laps);
            return (
              <Fragment key={group.sessionNum}>
                {groups.length > 1 ? (
                  <tr className="subsession-row">
                    <td colSpan={colSpan}>{group.sessionType}</td>
                  </tr>
                ) : null}
                {group.laps.map((lap) => {
                  const classes = [
                    lap.id === candidateLapId ? "candidate" : "",
                    lap.id === referenceLapId ? "reference" : "",
                  ]
                    .filter(Boolean)
                    .join(" ");
                  const sectorByNum = new Map(lap.sectors.map((s) => [s.sectorNum, s.timeMs]));
                  return (
                    <tr
                      key={lap.id}
                      className={classes}
                      title="Click = candidate · Shift/right-click = reference"
                      onClick={(e) => {
                        if (e.shiftKey) {
                          e.preventDefault();
                          onSelectReference(lap.id);
                          return;
                        }
                        onSelectCandidate(lap.id);
                      }}
                      onContextMenu={(e) => {
                        e.preventDefault();
                        onSelectReference(lap.id);
                      }}
                    >
                      <td>
                        <span className="lap-num-cell">
                          {lap.lapNumber}
                          {lap.id === candidateLapId ? (
                            <span className="pill lap-role cand">Cand</span>
                          ) : null}
                          {lap.id === referenceLapId ? (
                            <span className="pill lap-role ref">Ref</span>
                          ) : null}
                          {lap.hasTraffic ? (
                            <span
                              className="pill traffic"
                              title="Another car within ~1.5% lap distance on this lap"
                            >
                              traffic
                            </span>
                          ) : null}
                        </span>
                      </td>
                      <td className="num">{formatLapTime(lap.lapTimeMs)}</td>
                      <td
                        className="num"
                        style={deltaGradientStyle(lap.deltaToBestMs, deltaScale)}
                      >
                        {formatDelta(lap.deltaToBestMs)}
                      </td>
                      {sectorNums.map((n) => {
                        const t = sectorByNum.get(n);
                        return (
                          <td key={n} className={sectorClass(t, best.get(n), lap.paceEligible)}>
                            {t != null ? (t / 1000).toFixed(3) : "—"}
                          </td>
                        );
                      })}
                      <td>
                        <OkFlag lap={lap} />
                      </td>
                      <td>
                        <PitCell lap={lap} />
                      </td>
                      <td className="num">{formatLiters(lap.fuelUsed)}</td>
                      {showTireTemps ? (
                        <>
                          <td className="num">{formatTemp(lap.lfTemp)}</td>
                          <td className="num">{formatTemp(lap.rfTemp)}</td>
                          <td className="num">{formatTemp(lap.lrTemp)}</td>
                          <td className="num">{formatTemp(lap.rrTemp)}</td>
                        </>
                      ) : null}
                    </tr>
                  );
                })}
              </Fragment>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}
