import { Fragment, useEffect, useMemo, useState } from "react";
import type { CornerConsistency, CornerDelta, LapSummary } from "../../shared/types";
import { cornerConsistency } from "../../shared/api";
import { deltaClass, formatDelta, formatSpeedKph } from "../../shared/format";
import { CornerDetail, type ConsistencyState } from "./CornerDetail";
import { isCleanLap, matchConsistency } from "./cornerConsistency";
import { CAND_COLOR, REF_COLOR } from "./compareColors";

/** Corner · Cand · Ref · Δ · bar · Entry Δ · Exit Δ · Brake · Min · Throttle */
const COLUMNS = 10;

interface Props {
  corners: CornerDelta[];
  estimated: boolean;
  /** Session laps; the clean ones from the reference's sub-session feed consistency. */
  laps: LapSummary[];
  candidate: LapSummary;
  reference: LapSummary;
  onHoverDistPct?: (pct: number | null) => void;
  /** Row click: zoom the track map to this corner's apex. */
  onFocusDistPct?: (pct: number) => void;
  /** Candidate traffic hits (lap fraction) for tagging affected corners. */
  trafficPcts?: number[];
}

type Loaded =
  | { referenceLapId: number; data: CornerConsistency[] }
  | { referenceLapId: number; error: string };

/** Absolute corner duration, e.g. `1.820s`. */
function formatCornerTime(ms: number | null | undefined): string {
  if (ms == null || !Number.isFinite(ms) || ms < 0) return "—";
  return `${(ms / 1000).toFixed(3)}s`;
}

/** Signed delta with unit, e.g. `+0.184s`. */
function formatDeltaSeconds(ms: number | null | undefined): string {
  const d = formatDelta(ms);
  return d === "—" ? d : `${d}s`;
}

/** "12 m later than ref" / "8 m earlier than ref" / "same"; `null` when unknown. */
function formatMetres(m: number | null, later: string, earlier: string): string {
  if (m == null) return "—";
  const r = Math.round(m);
  if (Math.abs(r) < 2) return "same";
  return `${Math.abs(r)} m ${r > 0 ? later : earlier}`;
}

export function CornerTable({
  corners,
  estimated,
  laps,
  candidate,
  reference,
  onHoverDistPct,
  onFocusDistPct,
  trafficPcts = [],
}: Props) {
  const [sortByLoss, setSortByLoss] = useState(false);
  const [expanded, setExpanded] = useState<number | null>(null);
  const [loaded, setLoaded] = useState<Loaded | null>(null);

  const trafficCorners = useMemo(() => {
    const set = new Set<number>();
    for (const c of corners) {
      if (trafficPcts.some((p) => p >= c.entryPct && p <= c.exitPct)) {
        set.add(c.number);
      }
    }
    return set;
  }, [corners, trafficPcts]);

  const cleanLapIds = useMemo(
    () => laps.filter((l) => isCleanLap(l, reference.sessionNum)).map((l) => l.id),
    [laps, reference.sessionNum],
  );
  const needsFetch = expanded != null && loaded?.referenceLapId !== reference.id;

  useEffect(() => {
    if (!needsFetch) return;
    let cancelled = false;
    const referenceLapId = reference.id;
    cornerConsistency(referenceLapId, cleanLapIds)
      .then((data) => !cancelled && setLoaded({ referenceLapId, data }))
      .catch((e) => !cancelled && setLoaded({ referenceLapId, error: String(e) }));
    return () => {
      cancelled = true;
    };
  }, [needsFetch, reference.id, cleanLapIds]);

  const consistencyFor = (c: CornerDelta): ConsistencyState => {
    if (!loaded || loaded.referenceLapId !== reference.id) return { status: "loading" };
    if ("error" in loaded) return { status: "error", message: loaded.error };
    return { status: "ready", corner: matchConsistency(c, loaded.data) };
  };

  const rows = useMemo(
    () =>
      sortByLoss ? [...corners].sort((a, b) => b.timeDeltaMs - a.timeDeltaMs) : corners,
    [corners, sortByLoss],
  );
  const maxAbs = useMemo(
    () => Math.max(1, ...corners.map((c) => Math.abs(c.timeDeltaMs))),
    [corners],
  );

  if (corners.length === 0) {
    return <p className="muted">No corners detected on the reference lap.</p>;
  }

  return (
    <div className="corner-analysis">
      <div className="corner-header">
        <div className="chart-title">
          Corners ·{" "}
          <span style={{ color: CAND_COLOR }}>Lap {candidate.lapNumber}</span>
          {" vs "}
          <span style={{ color: REF_COLOR }}>Lap {reference.lapNumber}</span>
          {estimated ? " (estimated timing)" : ""}
        </div>
        <label className="corner-sort muted">
          <input
            type="checkbox"
            checked={sortByLoss}
            onChange={(e) => setSortByLoss(e.target.checked)}
          />
          Biggest loss first
        </label>
      </div>
      <p className="muted corner-caption">
        + = slower than ref
      </p>
      {estimated ? (
        <p className="muted corner-note">
          One of these laps was imported before Race Refinery stored lap timing, so corner
          times are estimated from speed. Re-import the session for exact numbers.
        </p>
      ) : null}
      <table className="data-table corner-table">
        <thead>
          <tr>
            <th title="Detected from the reference lap's speed; may not match official turn numbers">
              Corner
            </th>
            <th
              className="num"
              style={{ color: CAND_COLOR }}
              title="Time through this corner on the candidate lap"
            >
              Lap {candidate.lapNumber}
            </th>
            <th
              className="num"
              style={{ color: REF_COLOR }}
              title="Time through this corner on the reference lap"
            >
              Lap {reference.lapNumber}
            </th>
            <th
              className="num"
              title="Candidate − reference through this corner. + = slower than ref."
            >
              Δ
            </th>
            <th className="corner-bar-col" />
            <th
              className="num"
              title="Time gap from segment start to the reference’s slowest point"
            >
              Entry Δ
            </th>
            <th
              className="num"
              title="Time gap from the reference’s slowest point to segment end"
            >
              Exit Δ
            </th>
            <th
              className="num"
              title="How many metres later/earlier the candidate braked vs the reference"
            >
              Brake vs ref
            </th>
            <th className="num" title="Slowest speed through the corner, candidate / reference">
              Min km/h
            </th>
            <th
              className="num"
              title="Metres later/earlier the candidate reached full throttle vs the reference"
            >
              Throttle vs ref
            </th>
          </tr>
        </thead>
        <tbody>
          {rows.map((c) => (
            <Fragment key={c.number}>
              <tr
                className={[
                  "corner-focusable",
                  expanded === c.number ? "corner-expanded" : "",
                ]
                  .filter(Boolean)
                  .join(" ")}
                title="Click for corner detail and lap consistency"
                aria-expanded={expanded === c.number}
                onMouseEnter={() => onHoverDistPct?.(c.apexPct)}
                onMouseLeave={() => onHoverDistPct?.(null)}
                onClick={() => {
                  setExpanded((n) => (n === c.number ? null : c.number));
                  onFocusDistPct?.(c.apexPct);
                }}
              >
                <td>
                  <span
                    className={`corner-chevron${expanded === c.number ? " open" : ""}`}
                    aria-hidden
                  >
                    ›
                  </span>
                  C{c.number}
                  <span className="muted corner-pos"> {(c.apexPct * 100).toFixed(0)}%</span>
                  {trafficCorners.has(c.number) ? (
                    <span
                      className="pill traffic"
                      title="Another car within ~1.5% lap distance in this corner"
                    >
                      traffic
                    </span>
                  ) : null}
                </td>
                <td className="num">{formatCornerTime(c.candidateTimeMs)}</td>
                <td className="num muted">{formatCornerTime(c.referenceTimeMs)}</td>
                <td className={`num ${deltaClass(c.timeDeltaMs)}`}>
                  {formatDeltaSeconds(c.timeDeltaMs)}
                </td>
                <td className="corner-bar-col">
                  <DeltaBar ms={c.timeDeltaMs} maxAbs={maxAbs} />
                </td>
                <td className={`num ${deltaClass(c.entryDeltaMs)}`}>
                  {formatDeltaSeconds(c.entryDeltaMs)}
                </td>
                <td className={`num ${deltaClass(c.exitDeltaMs)}`}>
                  {formatDeltaSeconds(c.exitDeltaMs)}
                </td>
                <td className="num">
                  {formatMetres(c.brakePointDeltaM, "later than ref", "earlier than ref")}
                </td>
                <td className="num">
                  {formatSpeedKph(c.candidateMinSpeed)} / {formatSpeedKph(c.referenceMinSpeed)}
                </td>
                <td className="num">
                  {formatMetres(c.throttlePointDeltaM, "later than ref", "earlier than ref")}
                </td>
              </tr>
              {expanded === c.number ? (
                <tr className="corner-detail-row">
                  <td colSpan={COLUMNS}>
                    <CornerDetail
                      corner={c}
                      consistency={consistencyFor(c)}
                      laps={laps}
                      candidateLapId={candidate.id}
                      referenceLapId={reference.id}
                    />
                  </td>
                </tr>
              ) : null}
            </Fragment>
          ))}
        </tbody>
      </table>
    </div>
  );
}

/** Centred bar: losses grow right (red), gains grow left (green). */
function DeltaBar({ ms, maxAbs }: { ms: number; maxAbs: number }) {
  const width = `${(Math.abs(ms) / maxAbs) * 50}%`;
  return (
    <div className="corner-bar">
      <div
        className={`corner-bar-fill ${ms >= 0 ? "slow" : "fast"}`}
        style={ms >= 0 ? { left: "50%", width } : { right: "50%", width }}
      />
    </div>
  );
}
