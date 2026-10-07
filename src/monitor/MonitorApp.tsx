import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useState } from "react";
import {
  getLiveSnapshot,
  getSettings,
  onLiveTelemetry,
  onSettingsChanged,
} from "../shared/api";
import type { AppSettings, LiveSnapshot, WidgetKind } from "../shared/types";
import { WIDGET_KINDS } from "../shared/types";
import { useLapTrail } from "../shared/useLapTrail";
import { useTrackMap } from "../shared/useTrackMap";
import { Widget } from "../widgets";

function kindFromLabel(label: string): WidgetKind | null {
  const prefix = "monitor-";
  if (!label.startsWith(prefix)) return null;
  const kind = label.slice(prefix.length);
  return (WIDGET_KINDS as string[]).includes(kind) ? (kind as WidgetKind) : null;
}

export function MonitorApp() {
  const [kind, setKind] = useState<WidgetKind | null>(null);
  const [snap, setSnap] = useState<LiveSnapshot | null>(null);
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [error, setError] = useState<string | null>(null);
  const trackMap = useTrackMap(snap?.track);
  const lapTrail = useLapTrail(snap);

  useEffect(() => {
    try {
      setKind(kindFromLabel(getCurrentWindow().label));
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    let cancelled = false;
    getSettings()
      .then((s) => {
        if (!cancelled) setSettings(s);
      })
      .catch(() => undefined);
    getLiveSnapshot()
      .then((s) => {
        if (!cancelled) setSnap(s);
      })
      .catch(() => undefined);

    const unsubs = Promise.all([
      onLiveTelemetry((payload) => {
        if (!cancelled) setSnap(payload);
      }),
      onSettingsChanged((next) => {
        if (!cancelled) setSettings(next);
      }),
    ]);

    return () => {
      cancelled = true;
      unsubs.then((fns) => fns.forEach((fn) => fn())).catch(() => undefined);
    };
  }, []);

  if (error) {
    return <div className="pw-widget monitor-error">{error}</div>;
  }
  if (!kind) {
    return <div className="pw-widget monitor-waiting">Unknown monitor window</div>;
  }
  if (!snap) {
    return (
      <div className="pw-widget monitor-shell" data-tauri-drag-region>
        <div className="monitor-waiting">Waiting for live telemetry…</div>
      </div>
    );
  }

  const fieldPace = settings?.overlayLayout?.fieldPaceMode ?? "best";

  return (
    <div className="pw-widget monitor-shell" data-tauri-drag-region>
      <Widget
        kind={kind}
        snap={snap}
        fieldPaceMode={fieldPace}
        outline={trackMap}
        trail={lapTrail}
      />
    </div>
  );
}
