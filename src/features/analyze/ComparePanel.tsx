import { useEffect, useMemo, useState } from "react";
import {
  CartesianGrid,
  Line,
  LineChart,
  ReferenceArea,
  ReferenceLine,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import { compareLaps } from "../../shared/api";
import type {
  AssistKind,
  AssistSpan,
  CornerDelta,
  LapComparison,
  LapSummary,
} from "../../shared/types";
import { deltaClass, formatDelta, formatLapTime, formatSpeedKph } from "../../shared/format";
import { CornerTable } from "./CornerTable";
import { CAND_COLOR, DELTA_COLOR, REF_COLOR } from "./compareColors";
import { compareMode } from "./compareMode";
import {
  defaultTraceVisibility,
  TRACE_TOGGLES,
  type TraceToggleKey,
} from "./compareTraces";

const SYNC_ID = "compare-dist";
const G = 9.80665;

interface Props {
  laps: LapSummary[];
  candidate: LapSummary | null;
  reference: LapSummary | null;
  onChangeReference: (id: number) => void;
  /** Lap fraction (0..1) under the cursor, or `null` when the cursor leaves. */
  onHoverDistPct?: (pct: number | null) => void;
  /** Corner row click: zoom the track map to that lap fraction. */
  onFocusDistPct?: (pct: number) => void;
  /** Saved sets scatter every complete off-pit lap, not one sub-session. */
  sameSubsession?: boolean;
}

const ASSIST_NAMES: Record<AssistKind, string> = {
  abs: "ABS",
};

/** A shaded x range on a chart, in percent around the lap. */
interface ChartBand {
  x1: number;
  x2: number;
  color: string;
  label: string;
}

function assistBands(spans: AssistSpan[], kind: AssistKind): ChartBand[] {
  return spans
    .filter((s) => s.kind === kind)
    .map((s) => ({
      x1: +(s.startPct * 100).toFixed(2),
      x2: +(s.endPct * 100).toFixed(2),
      color: s.lap === "candidate" ? CAND_COLOR : REF_COLOR,
      label: `${ASSIST_NAMES[kind]} (${s.lap})`,
    }));
}

function lapLabel(lap: LapSummary): string {
  const time = lap.lapTimeMs != null ? ` · ${formatLapTime(lap.lapTimeMs)}` : "";
  const pace = lap.paceEligible ? " · pace" : "";
  const traffic = lap.hasTraffic ? " · traffic" : "";
  return `Lap ${lap.lapNumber}${time}${pace}${traffic}`;
}

export function ComparePanel({
  laps,
  candidate,
  reference,
  onChangeReference,
  onHoverDistPct,
  onFocusDistPct,
  sameSubsession = true,
}: Props) {
  const [comparison, setComparison] = useState<LapComparison | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [visible, setVisible] = useState(defaultTraceVisibility);

  const mode = compareMode(candidate, reference);
  /** Reference set, no compare lap yet — show that lap's graphs/timings alone. */
  const soloReference = mode === "solo";
  const canCompare = mode === "compare";
  const sameLap = mode === "same";
  const showTraces = soloReference || canCompare;

  useEffect(() => {
    if (!showTraces || !reference) {
      setComparison(null);
      return;
    }
    let cancelled = false;
    setLoading(true);
    setError(null);
    // Solo reuses compare_laps(ref, ref) so charts/corners share one pipeline;
    // the UI below drops candidate/delta overlays.
    const candidateId = canCompare && candidate ? candidate.id : reference.id;
    compareLaps(candidateId, reference.id)
      .then((c) => !cancelled && setComparison(c))
      .catch((e) => !cancelled && setError(String(e)))
      .finally(() => !cancelled && setLoading(false));
    return () => {
      cancelled = true;
    };
  }, [showTraces, canCompare, candidate, reference]);

  const chartData = useMemo(() => {
    if (!comparison) return [];
    const toDeg = (rad: number) => +(rad * (180 / Math.PI)).toFixed(1);
    const toG = (ms2: number) => +(ms2 / G).toFixed(2);
    return comparison.series.map((p) => ({
      x: +(p.distPct * 100).toFixed(2),
      deltaMs: p.cumulativeDeltaMs == null ? null : +p.cumulativeDeltaMs.toFixed(1),
      candSpeed: p.candidateSpeed == null ? null : +(p.candidateSpeed * 3.6).toFixed(1),
      refSpeed: p.referenceSpeed == null ? null : +(p.referenceSpeed * 3.6).toFixed(1),
      candThrottle: p.candidateThrottle == null ? null : +(p.candidateThrottle * 100).toFixed(0),
      refThrottle: p.referenceThrottle == null ? null : +(p.referenceThrottle * 100).toFixed(0),
      candBrake: p.candidateBrake == null ? null : +(p.candidateBrake * 100).toFixed(0),
      refBrake: p.referenceBrake == null ? null : +(p.referenceBrake * 100).toFixed(0),
      candGear: p.candidateGear == null ? null : Math.round(p.candidateGear),
      refGear: p.referenceGear == null ? null : Math.round(p.referenceGear),
      candSteering: p.candidateSteering == null ? null : toDeg(p.candidateSteering),
      refSteering: p.referenceSteering == null ? null : toDeg(p.referenceSteering),
      candClutch: p.candidateClutch == null ? null : +(p.candidateClutch * 100).toFixed(0),
      refClutch: p.referenceClutch == null ? null : +(p.referenceClutch * 100).toFixed(0),
      candRpm: p.candidateRpm == null ? null : Math.round(p.candidateRpm),
      refRpm: p.referenceRpm == null ? null : Math.round(p.referenceRpm),
      candLatG: p.candidateLatAccel == null ? null : toG(p.candidateLatAccel),
      refLatG: p.referenceLatAccel == null ? null : toG(p.referenceLatAccel),
      candLongG: p.candidateLongAccel == null ? null : toG(p.candidateLongAccel),
      refLongG: p.referenceLongAccel == null ? null : toG(p.referenceLongAccel),
      candYaw: p.candidateYawRate == null ? null : toDeg(p.candidateYawRate),
      refYaw: p.referenceYawRate == null ? null : toDeg(p.referenceYawRate),
    }));
  }, [comparison]);

  const absBands = useMemo(() => {
    const spans = comparison?.assists ?? [];
    const filtered = soloReference ? spans.filter((s) => s.lap === "reference") : spans;
    return assistBands(filtered, "abs");
  }, [comparison, soloReference]);

  const trafficMarkers = useMemo(() => {
    const refr = (comparison?.referenceTraffic ?? []).map((t) => ({
      pct: t.distPct,
      role: "reference" as const,
    }));
    if (soloReference) return refr;
    const cand = (comparison?.candidateTraffic ?? []).map((t) => ({
      pct: t.distPct,
      role: "candidate" as const,
    }));
    return [...cand, ...refr];
  }, [comparison, soloReference]);

  const show = (key: TraceToggleKey) => visible[key];
  const toggle = (key: TraceToggleKey) =>
    setVisible((v) => ({ ...v, [key]: !v[key] }));

  const channelToggles = soloReference
    ? TRACE_TOGGLES.filter((t) => t.key !== "delta")
    : TRACE_TOGGLES;

  const pairLines = (
    candKey: string,
    refKey: string,
    opts?: { step?: boolean },
  ): ChartLine[] =>
    soloReference
      ? [{ key: refKey, color: REF_COLOR, ...opts }]
      : [
          { key: candKey, color: CAND_COLOR, ...opts },
          { key: refKey, color: REF_COLOR, ...opts },
        ];

  return (
    <div className="panel compare-panel">
      <div className="panel-header">
        <h2>Compare</h2>
        <div className="compare-controls" style={{ marginLeft: "auto" }}>
          <label htmlFor="ref-select">Reference</label>
          <select
            id="ref-select"
            value={reference?.id ?? ""}
            onChange={(e) => onChangeReference(Number(e.target.value))}
          >
            <option value="" disabled>
              Pick a lap…
            </option>
            {laps.map((lap) => (
              <option key={lap.id} value={lap.id}>
                {lapLabel(lap)}
              </option>
            ))}
          </select>
        </div>
      </div>

      <div className="panel-body compare-scroll">
        {!reference ? (
          <p className="muted">
            {candidate
              ? "Pick a reference lap to compare against."
              : "Pick a reference lap to inspect, or select a lap below to compare."}
          </p>
        ) : sameLap ? (
          <p className="muted">Candidate and reference are the same lap. Pick another lap.</p>
        ) : loading ? (
          <p className="muted">{soloReference ? "Loading reference…" : "Comparing…"}</p>
        ) : error ? (
          <p className="slow">{soloReference ? "Failed to load reference: " : "Compare failed: "}
            {error}
          </p>
        ) : comparison && reference ? (
          <>
            <div className="trace-toggles" role="group" aria-label="Chart channels">
              {channelToggles.map((t) => (
                <button
                  key={t.key}
                  type="button"
                  className={`trace-toggle${visible[t.key] ? " on" : ""}`}
                  data-group={t.group}
                  aria-pressed={visible[t.key]}
                  onClick={() => toggle(t.key)}
                >
                  {t.label}
                </button>
              ))}
            </div>

            <div className="legend">
              {candidate && !soloReference ? (
                <span>
                  <span className="swatch" style={{ background: CAND_COLOR }} />
                  Candidate — Lap {candidate.lapNumber}
                  {candidate.hasTraffic ? " · traffic" : ""}
                </span>
              ) : null}
              <span>
                <span className="swatch" style={{ background: REF_COLOR }} />
                Reference — Lap {reference.lapNumber}
                {reference.hasTraffic ? " · traffic" : ""}
              </span>
              {!soloReference ? (
                <span>
                  <span className="swatch" style={{ background: DELTA_COLOR }} />
                  Time gain/loss
                </span>
              ) : (
                <span className="muted">Select a lap below to overlay a comparison.</span>
              )}
            </div>

            <div className="compare-summary">
              {candidate && !soloReference ? (
                <Fact label="Candidate" value={formatLapTime(comparison.candidateTimeMs)} />
              ) : null}
              <Fact label="Reference" value={formatLapTime(comparison.referenceTimeMs)} />
              {!soloReference ? (
                <Fact
                  label="Delta"
                  value={formatDelta(comparison.deltaMs)}
                  className={deltaClass(comparison.deltaMs)}
                />
              ) : null}
            </div>

            {soloReference ? (
              <ReferenceSectorTable comparison={comparison} />
            ) : (
              <SectorDeltaTable comparison={comparison} />
            )}

            {comparison.timing == null ? (
              <p className="muted">
                {soloReference
                  ? "Not enough trace data on this lap to time corners."
                  : "Not enough trace data on these laps to time them against each other."}
              </p>
            ) : soloReference ? (
              <ReferenceCornerTable
                corners={comparison.corners}
                estimated={comparison.timing === "estimated"}
                reference={reference}
                trafficPcts={(comparison.referenceTraffic ?? []).map((t) => t.distPct)}
                onHoverDistPct={onHoverDistPct}
                onFocusDistPct={onFocusDistPct}
              />
            ) : candidate ? (
              <CornerTable
                corners={comparison.corners}
                estimated={comparison.timing === "estimated"}
                laps={laps}
                candidate={candidate}
                reference={reference}
                trafficPcts={(comparison.candidateTraffic ?? []).map((t) => t.distPct)}
                onHoverDistPct={onHoverDistPct}
                onFocusDistPct={onFocusDistPct}
                sameSubsession={sameSubsession}
              />
            ) : null}

            {!soloReference && show("delta") ? (
              <>
                <div className="chart-title">
                  Time gain / loss vs distance
                  {comparison.timing === "estimated" ? " (estimated)" : ""}
                </div>
                <Chart
                  data={chartData}
                  onHoverDistPct={onHoverDistPct}
                  lines={[{ key: "deltaMs", color: DELTA_COLOR }]}
                  yFormatter={(v) => `${v >= 0 ? "+" : ""}${(v / 1000).toFixed(3)}s`}
                  zeroLine
                  corners={comparison.corners}
                  cornerLabels
                  traffic={trafficMarkers}
                />
              </>
            ) : null}

            {show("speed") ? (
              <>
                <div className="chart-title">Speed (km/h)</div>
                <Chart
                  data={chartData}
                  onHoverDistPct={onHoverDistPct}
                  corners={comparison.corners}
                  traffic={trafficMarkers}
                  lines={pairLines("candSpeed", "refSpeed")}
                />
              </>
            ) : null}

            {show("throttle") ? (
              <>
                <div className="chart-title">Throttle (%)</div>
                <Chart
                  data={chartData}
                  onHoverDistPct={onHoverDistPct}
                  corners={comparison.corners}
                  domain={[0, 100]}
                  lines={pairLines("candThrottle", "refThrottle")}
                />
              </>
            ) : null}

            {show("brake") ? (
              <>
                <div className="chart-title">Brake (%)</div>
                {absBands.length > 0 ? (
                  <p className="muted chart-note">
                    Shaded: ABS active
                    {soloReference ? "." : " in each lap's color."}
                  </p>
                ) : null}
                <Chart
                  data={chartData}
                  onHoverDistPct={onHoverDistPct}
                  corners={comparison.corners}
                  domain={[0, 100]}
                  bands={absBands}
                  lines={pairLines("candBrake", "refBrake")}
                />
              </>
            ) : null}

            {show("gear") ? (
              <>
                <div className="chart-title">Gear</div>
                <Chart
                  data={chartData}
                  onHoverDistPct={onHoverDistPct}
                  height={140}
                  lines={pairLines("candGear", "refGear", { step: true })}
                />
              </>
            ) : null}

            {show("rpm") ? (
              <>
                <div className="chart-title">RPM</div>
                <Chart
                  data={chartData}
                  onHoverDistPct={onHoverDistPct}
                  height={140}
                  lines={pairLines("candRpm", "refRpm")}
                />
              </>
            ) : null}

            {show("steering") ? (
              <>
                <div className="chart-title">Steering (°)</div>
                <Chart
                  data={chartData}
                  onHoverDistPct={onHoverDistPct}
                  height={140}
                  lines={pairLines("candSteering", "refSteering")}
                />
              </>
            ) : null}

            {show("clutch") ? (
              <>
                <div className="chart-title">Clutch (%)</div>
                <Chart
                  data={chartData}
                  onHoverDistPct={onHoverDistPct}
                  height={140}
                  domain={[0, 100]}
                  lines={pairLines("candClutch", "refClutch")}
                />
              </>
            ) : null}

            {show("latG") ? (
              <>
                <div className="chart-title">Lat G</div>
                <Chart
                  data={chartData}
                  onHoverDistPct={onHoverDistPct}
                  height={140}
                  zeroLine
                  lines={pairLines("candLatG", "refLatG")}
                />
              </>
            ) : null}

            {show("longG") ? (
              <>
                <div className="chart-title">Long G</div>
                <Chart
                  data={chartData}
                  onHoverDistPct={onHoverDistPct}
                  height={140}
                  zeroLine
                  lines={pairLines("candLongG", "refLongG")}
                />
              </>
            ) : null}

            {show("yaw") ? (
              <>
                <div className="chart-title">Yaw rate (°/s)</div>
                <Chart
                  data={chartData}
                  onHoverDistPct={onHoverDistPct}
                  height={140}
                  zeroLine
                  lines={pairLines("candYaw", "refYaw")}
                />
              </>
            ) : null}
          </>
        ) : null}
      </div>
    </div>
  );
}

function SectorDeltaTable({ comparison }: { comparison: LapComparison }) {
  if (comparison.sectorDeltas.length === 0) {
    return <p className="muted">No sector data for these laps.</p>;
  }
  return (
    <table className="data-table" style={{ maxWidth: 480, margin: "8px 0 16px" }}>
      <thead>
        <tr>
          <th>Sector</th>
          <th className="num">Candidate</th>
          <th className="num">Reference</th>
          <th className="num">Δ</th>
        </tr>
      </thead>
      <tbody>
        {comparison.sectorDeltas.map((s) => (
          <tr key={s.sectorNum} style={{ cursor: "default" }}>
            <td>S{s.sectorNum}</td>
            <td className="num">
              {s.candidateMs == null ? "—" : (s.candidateMs / 1000).toFixed(3)}
            </td>
            <td className="num">
              {s.referenceMs == null ? "—" : (s.referenceMs / 1000).toFixed(3)}
            </td>
            <td className={`num ${deltaClass(s.deltaMs)}`}>{formatDelta(s.deltaMs)}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

/** Sector times for the reference lap alone (no compare candidate yet). */
function ReferenceSectorTable({ comparison }: { comparison: LapComparison }) {
  if (comparison.sectorDeltas.length === 0) {
    return <p className="muted">No sector data for this lap.</p>;
  }
  return (
    <table className="data-table" style={{ maxWidth: 280, margin: "8px 0 16px" }}>
      <thead>
        <tr>
          <th>Sector</th>
          <th className="num">Time</th>
        </tr>
      </thead>
      <tbody>
        {comparison.sectorDeltas.map((s) => (
          <tr key={s.sectorNum} style={{ cursor: "default" }}>
            <td>S{s.sectorNum}</td>
            <td className="num">
              {s.referenceMs == null ? "—" : (s.referenceMs / 1000).toFixed(3)}
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

function formatCornerTime(ms: number | null | undefined): string {
  if (ms == null || !Number.isFinite(ms) || ms < 0) return "—";
  return `${(ms / 1000).toFixed(3)}s`;
}

/** Corner timings for the reference lap alone. */
function ReferenceCornerTable({
  corners,
  estimated,
  reference,
  trafficPcts = [],
  onHoverDistPct,
  onFocusDistPct,
}: {
  corners: CornerDelta[];
  estimated: boolean;
  reference: LapSummary;
  trafficPcts?: number[];
  onHoverDistPct?: (pct: number | null) => void;
  onFocusDistPct?: (pct: number) => void;
}) {
  const trafficCorners = useMemo(() => {
    const set = new Set<number>();
    for (const c of corners) {
      if (trafficPcts.some((p) => p >= c.entryPct && p <= c.exitPct)) {
        set.add(c.number);
      }
    }
    return set;
  }, [corners, trafficPcts]);

  if (corners.length === 0) {
    return <p className="muted">No corners detected on the reference lap.</p>;
  }

  return (
    <div className="corner-analysis">
      <div className="corner-header">
        <div className="chart-title">
          Corners ·{" "}
          <span style={{ color: REF_COLOR }}>Lap {reference.lapNumber}</span>
          {estimated ? " (estimated timing)" : ""}
        </div>
      </div>
      {estimated ? (
        <p className="muted corner-note">
          This lap was imported before Race Refinery stored lap timing, so corner times are
          estimated from speed. Re-import the session for exact numbers.
        </p>
      ) : null}
      <table className="data-table corner-table">
        <thead>
          <tr>
            <th title="Detected from the reference lap's speed; may not match official turn numbers">
              Corner
            </th>
            <th className="num" style={{ color: REF_COLOR }} title="Time through this corner">
              Time
            </th>
            <th className="num" title="Slowest speed through the corner">
              Min km/h
            </th>
          </tr>
        </thead>
        <tbody>
          {corners.map((c) => (
            <tr
              key={c.number}
              className="corner-focusable"
              title="Click to focus this corner on the track map"
              onMouseEnter={() => onHoverDistPct?.(c.apexPct)}
              onMouseLeave={() => onHoverDistPct?.(null)}
              onClick={() => onFocusDistPct?.(c.apexPct)}
            >
              <td>
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
              <td className="num">{formatCornerTime(c.referenceTimeMs)}</td>
              <td className="num">{formatSpeedKph(c.referenceMinSpeed)}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

interface ChartLine {
  key: string;
  color: string;
  /** Use a step curve (gear). */
  step?: boolean;
}

function Chart({
  data,
  lines,
  domain,
  height = 220,
  yFormatter,
  zeroLine,
  onHoverDistPct,
  corners,
  cornerLabels,
  bands,
  traffic,
}: {
  data: Record<string, number | null>[];
  lines: ChartLine[];
  domain?: [number, number];
  height?: number;
  yFormatter?: (v: number) => string;
  zeroLine?: boolean;
  onHoverDistPct?: (pct: number | null) => void;
  /** Draw a faint marker at each corner's slowest point. */
  corners?: CornerDelta[];
  cornerLabels?: boolean;
  /** Shaded x ranges, named in the tooltip when hovered. */
  bands?: ChartBand[];
  traffic?: { pct: number; role: "candidate" | "reference" }[];
}) {
  return (
    <div className="chart-wrap" style={{ height }}>
      <ResponsiveContainer width="100%" height="100%">
        <LineChart
          syncId={SYNC_ID}
          data={data}
          margin={{ top: 6, right: 12, bottom: 6, left: -8 }}
          onMouseMove={(state) => {
            const label = Number(state?.activeLabel);
            onHoverDistPct?.(Number.isFinite(label) ? label / 100 : null);
          }}
          onMouseLeave={() => onHoverDistPct?.(null)}
        >
          <CartesianGrid stroke="#262d3a" strokeDasharray="3 3" />
          <XAxis
            dataKey="x"
            type="number"
            domain={[0, 100]}
            tick={{ fill: "#8b95a5", fontSize: 11 }}
            tickFormatter={(v) => `${v}%`}
          />
          <YAxis
            domain={domain ?? ["auto", "auto"]}
            tick={{ fill: "#8b95a5", fontSize: 11 }}
            width={52}
            allowDecimals={!lines.some((l) => l.step)}
            tickFormatter={yFormatter}
          />
          <Tooltip
            contentStyle={{
              background: "#12161f",
              border: "1px solid #262d3a",
              borderRadius: 6,
              fontSize: 12,
            }}
            labelFormatter={(v) => {
              const active = bands?.filter((b) => v >= b.x1 && v <= b.x2).map((b) => b.label);
              return active?.length
                ? `${v}% around lap · ${active.join(", ")}`
                : `${v}% around lap`;
            }}
            formatter={(value: number, name: string) => {
              if (name === "deltaMs") {
                return [formatDelta(value), "Δ time"];
              }
              return [value, name];
            }}
          />
          {zeroLine ? <ReferenceLine y={0} stroke="#5b6472" strokeDasharray="4 4" /> : null}
          {bands?.map((b) => (
            <ReferenceArea
              key={`${b.label}-${b.x1}`}
              x1={b.x1}
              x2={b.x2}
              fill={b.color}
              fillOpacity={0.15}
              stroke="none"
              ifOverflow="hidden"
            />
          ))}
          {corners?.map((c) => (
            <ReferenceLine
              key={c.number}
              x={+(c.apexPct * 100).toFixed(2)}
              stroke="#3a4352"
              strokeDasharray="2 4"
              label={
                cornerLabels
                  ? { value: `C${c.number}`, position: "insideTop", fill: "#8b95a5", fontSize: 10 }
                  : undefined
              }
            />
          ))}
          {traffic?.map((t, i) => (
            <ReferenceLine
              key={`traffic-${t.role}-${i}-${t.pct}`}
              x={+(t.pct * 100).toFixed(2)}
              stroke={t.role === "candidate" ? CAND_COLOR : REF_COLOR}
              strokeOpacity={0.45}
              strokeDasharray="1 3"
            />
          ))}
          {lines.map((l) => (
            <Line
              key={l.key}
              type={l.step ? "stepAfter" : "monotone"}
              dataKey={l.key}
              stroke={l.color}
              dot={false}
              strokeWidth={1.6}
              connectNulls={false}
              isAnimationActive={false}
            />
          ))}
        </LineChart>
      </ResponsiveContainer>
    </div>
  );
}

function Fact({
  label,
  value,
  className,
}: {
  label: string;
  value: string;
  className?: string;
}) {
  return (
    <div className="fact">
      <span className="fact-label">{label}</span>
      <span className={`fact-value ${className ?? ""}`}>{value}</span>
    </div>
  );
}
