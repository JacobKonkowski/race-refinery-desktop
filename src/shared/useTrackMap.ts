import { useEffect, useState } from "react";
import { getTrackMap } from "./api";
import type { TrackOutline } from "./types";

/**
 * Generated circuit outline for `track`, fetched once per track name.
 *
 * Outlines are static per circuit, so live surfaces load them on track change
 * rather than with each telemetry frame.
 */
export function useTrackMap(track: string | null | undefined): TrackOutline | null {
  const [outline, setOutline] = useState<TrackOutline | null>(null);

  useEffect(() => {
    if (!track) {
      setOutline(null);
      return;
    }
    let active = true;
    getTrackMap(track)
      .then((next) => {
        if (active) setOutline(next);
      })
      .catch(() => {
        if (active) setOutline(null);
      });
    return () => {
      active = false;
    };
  }, [track]);

  return outline;
}
