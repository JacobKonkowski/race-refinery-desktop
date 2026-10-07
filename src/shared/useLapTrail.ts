import { useEffect, useRef, useState } from "react";
import type { LiveSnapshot, TrailSample } from "./types";

/** Keep roughly this many samples per lap; ~0.4% of the lap between points. */
const MIN_STEP_PCT = 1 / 256;

/**
 * Current-lap trail accumulated from live snapshots.
 *
 * The snapshot carries only the latest sample, so the trail is built here rather
 * than re-sent at 10 Hz. It resets on a new lap and thins samples by distance so
 * a long lap cannot grow without bound.
 */
export function useLapTrail(snap: LiveSnapshot | null): TrailSample[] {
  const [trail, setTrail] = useState<TrailSample[]>([]);
  const lapRef = useRef<number | null>(null);

  useEffect(() => {
    if (!snap) {
      setTrail([]);
      lapRef.current = null;
      return;
    }

    const sample: TrailSample = {
      distPct: snap.lapDistPct,
      throttle: snap.throttle,
      brake: snap.brake,
      throttleRaw: snap.throttleRaw ?? null,
      brakeRaw: snap.brakeRaw ?? null,
      lat: snap.lat ?? null,
      lon: snap.lon ?? null,
    };

    const isNewLap = lapRef.current !== snap.lap;
    lapRef.current = snap.lap;

    setTrail((prev) => {
      const last = prev[prev.length - 1];
      // A new lap, or distance jumping backwards (reset / tow), starts over.
      if (isNewLap || (last && sample.distPct < last.distPct)) {
        return [sample];
      }
      if (last && sample.distPct - last.distPct < MIN_STEP_PCT) {
        return prev;
      }
      return [...prev, sample];
    });
  }, [snap]);

  return trail;
}
