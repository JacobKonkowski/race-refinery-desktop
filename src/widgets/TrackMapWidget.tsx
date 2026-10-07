import { useEffect, useMemo, useRef, useState } from "react";
import type { CSSProperties, PointerEvent as ReactPointerEvent, RefObject } from "react";
import {
  FIT_CAMERA,
  clientToMap,
  focusOn,
  isFit,
  panByPixels,
  viewBoxFromCamera,
  zoomAt,
} from "../shared/mapCamera";
import type { MapCamera } from "../shared/mapCamera";
import {
  hasRacingLine,
  pedalSegments,
  pointAt,
  startFinishSegment,
  toPathD,
  tracePath,
} from "../shared/trackMap";
import type { LiveSnapshot, TrackOutline, TrailSample } from "../shared/types";

/** Zoom multiplier per wheel notch (~100 px of `deltaY`). */
const WHEEL_ZOOM_PER_NOTCH = 1.25;

/** `pedal` colors one lap by throttle/brake; `compare` overlays two racing lines. */
export type TrackMapMode = "pedal" | "compare";

interface Props {
  /** Cached outline for the current track; `null` until one is generated. */
  outline: TrackOutline | null;
  /** Live field to plot. Omit for a bare circuit (post-session view). */
  snap?: LiveSnapshot | null;
  /** Lap being studied: the Analyze candidate, or the live current-lap trail. */
  candidateTraces?: TrailSample[] | null;
  /** Lap to compare against (Analyze only). */
  referenceTraces?: TrailSample[] | null;
  mode?: TrackMapMode;
  /** Lap fraction to mark, synced from chart hover. */
  highlightPct?: number | null;
  /** Scroll to zoom, drag to pan, double-click to reset (Analyze only). */
  interactive?: boolean;
  /** Lap fraction to centre and zoom on; re-applied whenever `focusSeq` changes. */
  focusPct?: number | null;
  focusSeq?: number;
}

/**
 * Camera for an interactive map. Wheel is bound natively because React's
 * `onWheel` is passive and can't stop the page from scrolling.
 */
function useMapCamera(
  svgRef: RefObject<SVGSVGElement | null>,
  enabled: boolean,
  outline: TrackOutline | null,
  focusPct: number | null | undefined,
  focusSeq: number | undefined,
) {
  const [camera, setCamera] = useState<MapCamera>(FIT_CAMERA);
  const drag = useRef<{ pointerId: number; x: number; y: number } | null>(null);
  const [dragging, setDragging] = useState(false);
  const hasOutline = Boolean(outline && outline.points.length > 0);

  useEffect(() => setCamera(FIT_CAMERA), [outline?.track]);

  useEffect(() => {
    if (!enabled || !outline || focusPct == null) return;
    const point = pointAt(outline.points, focusPct);
    if (point) setCamera((c) => focusOn(c, point));
    // `focusSeq` re-triggers a focus on the same corner after panning away.
  }, [enabled, outline, focusPct, focusSeq]);

  useEffect(() => {
    const svg = svgRef.current;
    if (!enabled || !svg) return;
    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      const rect = svg.getBoundingClientRect();
      const factor = WHEEL_ZOOM_PER_NOTCH ** (-e.deltaY / 100);
      setCamera((c) => zoomAt(c, clientToMap(c, rect, e.clientX, e.clientY), factor));
    };
    svg.addEventListener("wheel", onWheel, { passive: false });
    return () => svg.removeEventListener("wheel", onWheel);
  }, [svgRef, enabled, hasOutline]);

  const handlers = enabled
    ? {
        onPointerDown: (e: ReactPointerEvent<SVGSVGElement>) => {
          if (e.button !== 0) return;
          e.currentTarget.setPointerCapture(e.pointerId);
          drag.current = { pointerId: e.pointerId, x: e.clientX, y: e.clientY };
          setDragging(true);
        },
        onPointerMove: (e: ReactPointerEvent<SVGSVGElement>) => {
          const d = drag.current;
          if (!d || d.pointerId !== e.pointerId) return;
          const rect = e.currentTarget.getBoundingClientRect();
          const dx = e.clientX - d.x;
          const dy = e.clientY - d.y;
          d.x = e.clientX;
          d.y = e.clientY;
          setCamera((c) => panByPixels(c, rect, dx, dy));
        },
        onPointerUp: (e: ReactPointerEvent<SVGSVGElement>) => {
          if (drag.current?.pointerId !== e.pointerId) return;
          drag.current = null;
          setDragging(false);
        },
        onPointerCancel: () => {
          drag.current = null;
          setDragging(false);
        },
        onDoubleClick: () => setCamera(FIT_CAMERA),
      }
    : {};

  return {
    camera: enabled ? camera : FIT_CAMERA,
    dragging,
    handlers,
    reset: () => setCamera(FIT_CAMERA),
  };
}

/**
 * Lap paths for the current view. Memoized because chart hover re-renders this
 * widget on every mouse move, while the paths only change with the laps.
 */
function useLapGeometry(
  outline: TrackOutline | null,
  candidate: TrailSample[] | null,
  reference: TrailSample[] | null,
  mode: TrackMapMode,
) {
  return useMemo(() => {
    if (!outline) {
      return { segments: [], candidateLine: [], referenceLine: [], needsReimport: false };
    }
    // Line compare needs real GPS on both laps; pedal zones work either way.
    const canCompare =
      mode === "compare" &&
      hasRacingLine(outline, candidate) &&
      hasRacingLine(outline, reference);
    return {
      segments: canCompare || !candidate ? [] : pedalSegments(outline, candidate),
      candidateLine: canCompare && candidate ? tracePath(outline, candidate) : [],
      referenceLine: canCompare && reference ? tracePath(outline, reference) : [],
      needsReimport: mode === "compare" && !canCompare && Boolean(candidate || reference),
    };
  }, [outline, candidate, reference, mode]);
}

/** Circuit outline with cars, lap paths, and pedal zones placed by lap distance. */
export function TrackMapWidget({
  outline,
  snap,
  candidateTraces,
  referenceTraces,
  mode = "pedal",
  highlightPct,
  interactive = false,
  focusPct,
  focusSeq,
}: Props) {
  const hasOutline = Boolean(outline && outline.points.length > 0);
  const { segments, candidateLine, referenceLine, needsReimport } = useLapGeometry(
    hasOutline ? outline : null,
    candidateTraces ?? null,
    referenceTraces ?? null,
    mode,
  );
  const svgRef = useRef<SVGSVGElement>(null);
  const { camera, dragging, handlers, reset } = useMapCamera(
    svgRef,
    interactive,
    hasOutline ? outline : null,
    focusPct,
    focusSeq,
  );

  if (!outline || !hasOutline) {
    return (
      <div className="pw-trackmap pw-trackmap-empty">
        <p>No track map yet</p>
        <p className="pw-trackmap-hint">
          Import an IBT recorded at this track to generate one.
        </p>
      </div>
    );
  }

  const marker = highlightPct == null ? null : pointAt(outline.points, highlightPct);
  const cars = snap
    ? snap.competitors
        .filter((c) => !c.isPlayer)
        .map((c) => ({
          key: `car-${c.carIdx}`,
          label: `#${c.carNumber || c.carIdx} ${c.driverName}`,
          at: pointAt(outline.points, c.lapDistPct),
          pit: c.onPitRoad,
        }))
        .filter((c) => c.at !== null)
    : [];
  const player = snap ? pointAt(outline.points, snap.lapDistPct) : null;
  // Strokes and markers thin out as you zoom so a corner stays readable,
  // while still growing a little on screen.
  const k = 1 / Math.sqrt(camera.scale);
  const svgStyle = { "--pw-map-k": k } as CSSProperties;
  const startLine = startFinishSegment(outline.points, 0.022 * k);

  return (
    <div
      className={`pw-trackmap${interactive ? " pw-trackmap-interactive" : ""}${
        dragging ? " dragging" : ""
      }`}
    >
      <svg
        ref={svgRef}
        viewBox={viewBoxFromCamera(camera)}
        preserveAspectRatio="xMidYMid meet"
        role="img"
        style={svgStyle}
        {...handlers}
      >
        <title>{outline.track}</title>
        <path className="pw-trackmap-ribbon" d={outline.svgPath} />

        {segments.map((seg, i) => (
          <path
            key={`pedal-${i}`}
            className={`pw-trackmap-pedal ${seg.tone}`}
            d={toPathD(seg.points)}
          />
        ))}
        {referenceLine.length > 1 && (
          <path className="pw-trackmap-line reference" d={toPathD(referenceLine)} />
        )}
        {candidateLine.length > 1 && (
          <path className="pw-trackmap-line candidate" d={toPathD(candidateLine)} />
        )}

        {startLine && (
          <line
            className="pw-trackmap-start"
            x1={startLine.x1}
            y1={startLine.y1}
            x2={startLine.x2}
            y2={startLine.y2}
          />
        )}
        {cars.map((c) => (
          <circle
            key={c.key}
            className={`pw-trackmap-car${c.pit ? " pit" : ""}`}
            cx={c.at!.x}
            cy={c.at!.y}
            r={0.02 * k}
          >
            <title>{c.label}</title>
          </circle>
        ))}
        {player && (
          <circle className="pw-trackmap-me" cx={player.x} cy={player.y} r={0.026 * k} />
        )}
        {marker && (
          <circle className="pw-trackmap-marker" cx={marker.x} cy={marker.y} r={0.03 * k} />
        )}
      </svg>
      {interactive && !isFit(camera) && (
        <button
          type="button"
          className="pw-trackmap-reset"
          onClick={reset}
          title="Show the whole track (or double-click the map)"
        >
          Reset
        </button>
      )}
      {needsReimport && (
        <p className="pw-trackmap-hint">
          Re-import this session's IBT to compare racing lines.
        </p>
      )}
    </div>
  );
}
