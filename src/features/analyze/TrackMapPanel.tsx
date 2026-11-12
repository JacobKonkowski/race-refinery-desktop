import { useEffect, useRef, useState } from "react";
import { getLapTraces } from "../../shared/api";
import { hasRacingLine } from "../../shared/trackMap";
import type { LapSummary, TracePoint, TrackOutline } from "../../shared/types";
import { TrackMapWidget } from "../../widgets";
import type { TrackMapMode } from "../../widgets";

interface Props {
  outline: TrackOutline | null;
  track: string;
  candidate: LapSummary | null;
  reference: LapSummary | null;
  /** Lap fraction hovered on the compare charts. */
  highlightPct: number | null;
  /** Corner the map should zoom to; `seq` changes on every request. */
  focus: { pct: number; seq: number } | null;
}

/**
 * Track map for the selected session: pedal zones for the candidate lap, or the
 * two racing lines overlaid. Prefers Lines when both laps have GPS unless the
 * user pins a mode. Owns the trace fetch so the widget stays presentational.
 */
export function TrackMapPanel({
  outline,
  track,
  candidate,
  reference,
  highlightPct,
  focus,
}: Props) {
  const [mode, setMode] = useState<TrackMapMode>("pedal");
  const [modePinned, setModePinned] = useState(false);
  const [traces, setTraces] = useState<Record<number, TracePoint[]>>({});
  const panelRef = useRef<HTMLDivElement>(null);

  // The corner table sits below the map; bring the map into view on focus.
  useEffect(() => {
    if (focus) panelRef.current?.scrollIntoView({ behavior: "smooth", block: "nearest" });
  }, [focus]);

  // New track/session: allow auto Lines again.
  useEffect(() => {
    setModePinned(false);
  }, [track]);

  const candidateId = candidate?.id ?? null;
  const referenceId = reference?.id ?? null;

  useEffect(() => {
    const ids = [candidateId, referenceId].filter((id): id is number => id != null);
    if (ids.length === 0) {
      setTraces({});
      return;
    }
    let active = true;
    getLapTraces(ids)
      .then((loaded) => {
        if (active) {
          setTraces(Object.fromEntries(loaded.map((t) => [t.lapId, t.points])));
        }
      })
      .catch(() => {
        if (active) setTraces({});
      });
    return () => {
      active = false;
    };
  }, [candidateId, referenceId]);

  const candidateTraces = candidateId == null ? null : traces[candidateId] ?? null;
  const referenceTraces = referenceId == null ? null : traces[referenceId] ?? null;
  const linesAvailable =
    hasRacingLine(outline, candidateTraces) && hasRacingLine(outline, referenceTraces);

  useEffect(() => {
    if (modePinned) return;
    setMode(linesAvailable ? "compare" : "pedal");
  }, [linesAvailable, modePinned]);

  const pickMode = (next: TrackMapMode) => {
    setModePinned(true);
    setMode(next);
  };

  return (
    <div className="panel" ref={panelRef}>
      <div className="panel-header">
        <h2>Track Map</h2>
        <span className="muted">{track}</span>
        <div className="btn-row" style={{ marginLeft: "auto" }}>
          <button
            type="button"
            className={`btn btn-sm${mode === "pedal" ? " btn-primary" : ""}`}
            onClick={() => pickMode("pedal")}
          >
            Pedals
          </button>
          <button
            type="button"
            className={`btn btn-sm${mode === "compare" ? " btn-primary" : ""}`}
            onClick={() => pickMode("compare")}
            disabled={!linesAvailable}
            title={
              linesAvailable
                ? "Overlay candidate and reference racing lines"
                : "Needs two laps with GPS — re-import this session's IBT"
            }
          >
            Lines
          </button>
        </div>
      </div>
      <div className="panel-body">
        <div className="analyze-trackmap">
          <TrackMapWidget
            outline={outline}
            candidateTraces={candidateTraces}
            referenceTraces={referenceTraces}
            mode={mode}
            highlightPct={highlightPct}
            interactive
            focusPct={focus?.pct ?? null}
            focusSeq={focus?.seq}
          />
        </div>
        <p className="muted analyze-trackmap-legend">
          {mode === "pedal" ? (
            <>
              <span className="swatch" style={{ background: "#ff6b6b" }} /> Brake
              <span className="swatch" style={{ background: "#5dffa8" }} /> Throttle
              <span className="swatch" style={{ background: "#ffb347" }} /> Coast
              {candidate ? ` — L ${candidate.lapNumber}` : " — select a lap"}
            </>
          ) : (
            <>
              <span className="swatch" style={{ background: "#4aa3ff" }} /> Candidate
              <span className="swatch" style={{ background: "#d29922" }} /> Reference
            </>
          )}
          <span className="analyze-trackmap-help">
            Scroll to zoom · drag to pan · double-click to reset
          </span>
        </p>
      </div>
    </div>
  );
}