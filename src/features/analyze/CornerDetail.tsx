import { useMemo } from "react";
import {
  CartesianGrid,
  ReferenceLine,
  ResponsiveContainer,
  Scatter,
  ScatterChart,
  Tooltip,
  XAxis,
  YAxis,
  type TooltipProps,
} from "recharts";
import type {
  CornerConsistency,
  CornerDelta,
  CornerTechnique,
  LapSummary,
} from "../../shared/types";
import { formatDelta } from "../../shared/format";
import { CAND_COLOR, REF_COLOR } from "./compareColors";
import { OFF_SCALE_MS, plottablePoints } from "./cornerConsistency";

export type ConsistencyState =
  | { status: "loading" }
  | { status: "error"; message: string }
  | { status: "ready"; corner: CornerConsistency | null };

interface Props {
  corner: CornerDelta;
  consistency: ConsistencyState;
  laps: LapSummary[];
  candidateLapId: number;
  referenceLapId: number;
}

interface ScatterDatum {
  x: number;
  y: number;
  lapNumber: number | null;
}

function seconds(ms: number | null): string {
  return ms == null ? "—" : `${(ms / 1000).toFixed(2)}s`;
}

const METRICS: { label: string; title: string; value: (t: CornerTechnique) => string }[] = [
  {
    label: "ABS active",
    title: "Time ABS was reducing brake pressure",
    value: (t) => seconds(t.absMs),
  },
  {
    label: "Peak brake",
    title: "Highest brake pedal before the reference apex",
    value: (t) => (t.peakBrake == null ? "—" : `${Math.round(t.peakBrake * 100)}%`),
  },
  {
    label: "Trail braking",
    title: "From the last moment near peak brake to below 10%",
    value: (t) => seconds(t.trailBrakeMs),
  },
  {
    label: "Coasting",
    title: "Time with neither pedal pressed",
    value: (t) => seconds(t.coastMs),
  },
  {
    label: "Apex → full throttle",
    title: "From this lap's slowest point to full throttle",
    value: (t) => seconds(t.apexToThrottleMs),
  },
];

/** Expanded corner row: how each lap drove it, and every clean lap's brake point. */
export function CornerDetail({
  corner,
  consistency,
  laps,
  candidateLapId,
  referenceLapId,
}: Props) {
  const missingAssists =
    corner.candidate.absMs == null || corner.reference.absMs == null;

  return (
    <div className="corner-detail">
      <div className="corner-detail-body">
        <table className="data-table corner-technique">
          <thead>
            <tr>
              <th />
              <th className="num" style={{ color: CAND_COLOR }}>
                Candidate
              </th>
              <th className="num" style={{ color: REF_COLOR }}>
                Reference
              </th>
            </tr>
          </thead>
          <tbody>
            {METRICS.map((m) => (
              <tr key={m.label}>
                <td title={m.title}>{m.label}</td>
                <td className="num">{m.value(corner.candidate)}</td>
                <td className="num">{m.value(corner.reference)}</td>
              </tr>
            ))}
          </tbody>
        </table>
        {missingAssists ? (
          <p className="muted corner-note">
            ABS needs a lap imported with `BrakeABSactive` (schema v6); re-import older sessions
            for assist data.
          </p>
        ) : null}

        <div className="corner-consistency">
          <ConsistencyScatter
            state={consistency}
            laps={laps}
            candidateLapId={candidateLapId}
            referenceLapId={referenceLapId}
          />
        </div>
      </div>
    </div>
  );
}

function ConsistencyScatter({
  state,
  laps,
  candidateLapId,
  referenceLapId,
}: {
  state: ConsistencyState;
  laps: LapSummary[];
  candidateLapId: number;
  referenceLapId: number;
}) {
  const series = useMemo(() => {
    if (state.status !== "ready" || !state.corner) return null;
    const lapNumber = new Map(laps.map((l) => [l.id, l.lapNumber]));
    const { shown, offScale } = plottablePoints(state.corner.points);
    const split: Record<"others" | "cand" | "ref", ScatterDatum[]> = {
      others: [],
      cand: [],
      ref: [],
    };
    for (const p of shown) {
      const datum = {
        x: Math.round(p.brakeOffsetM ?? 0),
        y: +p.timeDeltaMs.toFixed(0),
        lapNumber: lapNumber.get(p.lapId) ?? null,
      };
      if (p.lapId === candidateLapId) split.cand.push(datum);
      else if (p.lapId === referenceLapId) split.ref.push(datum);
      else split.others.push(datum);
    }
    return { ...split, offScale };
  }, [state, laps, candidateLapId, referenceLapId]);

  if (state.status === "loading") return <p className="muted">Loading lap consistency…</p>;
  if (state.status === "error") {
    return <p className="slow">Consistency failed: {state.message}</p>;
  }
  if (!state.corner || !series) {
    return <p className="muted">No consistency data for this corner.</p>;
  }
  const plotted = series.others.length + series.cand.length + series.ref.length;
  if (plotted < 2) {
    return <p className="muted">Not enough clean laps braking here to compare.</p>;
  }

  const spread = state.corner.brakeSpreadM;
  return (
    <>
      <div className="chart-title">
        Brake point vs corner time — {plotted} clean laps
        {spread != null ? `, spread ±${Math.round(spread)} m` : ""}
      </div>
      <div className="chart-wrap" style={{ height: 200 }}>
        <ResponsiveContainer width="100%" height="100%">
          <ScatterChart margin={{ top: 6, right: 12, bottom: 6, left: -8 }}>
            <CartesianGrid stroke="#262d3a" strokeDasharray="3 3" />
            <XAxis
              type="number"
              dataKey="x"
              tick={{ fill: "#8b95a5", fontSize: 11 }}
              tickFormatter={(v) => `${v > 0 ? "+" : ""}${v} m`}
            />
            <YAxis
              type="number"
              dataKey="y"
              width={52}
              tick={{ fill: "#8b95a5", fontSize: 11 }}
              tickFormatter={(v) => formatDelta(v)}
            />
            <ReferenceLine x={0} stroke="#5b6472" strokeDasharray="4 4" />
            <ReferenceLine y={0} stroke="#5b6472" strokeDasharray="4 4" />
            <Tooltip content={ScatterTip} />
            <Scatter data={series.others} fill="#8b95a5" isAnimationActive={false} />
            <Scatter data={series.ref} fill={REF_COLOR} isAnimationActive={false} />
            <Scatter data={series.cand} fill={CAND_COLOR} isAnimationActive={false} />
          </ScatterChart>
        </ResponsiveContainer>
      </div>
      <p className="muted corner-note">
        Right = braked later than the reference; up = slower through the corner.
        {series.offScale > 0
          ? ` ${series.offScale} lap${series.offScale === 1 ? "" : "s"} hidden (over ${
              OFF_SCALE_MS / 1000
            }s lost: a spin or off-track).`
          : ""}
      </p>
    </>
  );
}

function ScatterTip({ active, payload }: TooltipProps<number, string>) {
  const d = payload?.[0]?.payload as ScatterDatum | undefined;
  if (!active || !d) return null;
  const brake =
    d.x === 0 ? "same brake point" : `braked ${Math.abs(d.x)} m ${d.x > 0 ? "later" : "earlier"}`;
  return (
    <div className="scatter-tip">
      <div>{d.lapNumber != null ? `Lap ${d.lapNumber}` : "Lap"}</div>
      <div className="muted">
        {brake}, {formatDelta(d.y)}s
      </div>
    </div>
  );
}
