import { useCallback, useEffect, useRef, useState } from "react";
import {
  buildOpenKneeboardUrl,
  checkVrHudHealth,
  getAudioCoachStatus,
  getLiveSnapshot,
  getLiveStatus,
  getMonitorOverlayStatus,
  getNativeVrStatus,
  getSettings,
  getVrLayerDiagnostics,
  getVrOverlayStatus,
  installVrLayer,
  onLiveStatus,
  onLiveTelemetry,
  openVrHudPreview,
  patchSettings,
  recenterVr,
  saveSettings,
  startAudioCoach,
  startDemoClock,
  startLiveMonitor,
  startMonitorOverlay,
  startVrOverlay,
  stopAudioCoach,
  stopDemoClock,
  stopLiveMonitor,
  stopMonitorOverlay,
  stopVrOverlay,
  testAudioCoach,
  uninstallVrLayer,
} from "../../shared/api";
import { formatDelta, formatLapTime, formatLiters, formatTemp } from "../../shared/format";
import { Slider } from "../../shared/Slider";
import { TierBadges } from "../../shared/TierBadges";
import { showToast } from "../../shared/toast";
import type {
  AppSettings,
  AudioCoachStatus,
  LiveSnapshot,
  LiveStatus,
  MonitorOverlayStatus,
  NativeVrStatus,
  VrLayerDiagnostics,
  VrOverlayStatus,
  WidgetPlacement,
} from "../../shared/types";
import { useLapTrail } from "../../shared/useLapTrail";
import { coachTestMessage } from "../../shared/voicePacks";
import { useTrackMap } from "../../shared/useTrackMap";
import { CoachWidget, TrackMapWidget } from "../../widgets";
import { SessionLeaderboard } from "./SessionLeaderboard";

const VR_PREVIEW_URL = "http://127.0.0.1:17342/vr";

function stateClass(state: LiveStatus["state"]): string {
  switch (state) {
    case "connected":
      return "live-pill live-pill-ok";
    case "waitingForSession":
    case "reconnecting":
      return "live-pill live-pill-wait";
    case "error":
      return "live-pill live-pill-err";
    default:
      return "live-pill";
  }
}

function stateLabel(state: LiveStatus["state"]): string {
  switch (state) {
    case "connected":
      return "Connected";
    case "waitingForSession":
      return "Waiting";
    case "reconnecting":
      return "Reconnecting";
    case "error":
      return "Error";
    default:
      return "Disconnected";
  }
}

export function LivePage() {
  const [status, setStatus] = useState<LiveStatus>({
    state: "disconnected",
    message: "Live monitor stopped",
  });
  const [snap, setSnap] = useState<LiveSnapshot | null>(null);
  const [running, setRunning] = useState(false);
  const [demoRunning, setDemoRunning] = useState(false);
  const [vrStatus, setVrStatus] = useState<VrOverlayStatus | null>(null);
  const [monitorStatus, setMonitorStatus] = useState<MonitorOverlayStatus | null>(null);
  const [nativeVr, setNativeVr] = useState<NativeVrStatus | null>(null);
  const [layerDiag, setLayerDiag] = useState<VrLayerDiagnostics | null>(null);
  const [vrHudHealthy, setVrHudHealthy] = useState<boolean | null>(null);
  const [audioStatus, setAudioStatus] = useState<AudioCoachStatus | null>(null);
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const [liveStatus, snapshot, vr, monitor, audio, cfg] = await Promise.all([
        getLiveStatus(),
        getLiveSnapshot().catch(() => null),
        getVrOverlayStatus().catch(() => null),
        getMonitorOverlayStatus().catch(() => null),
        getAudioCoachStatus().catch(() => null),
        getSettings().catch(() => null),
      ]);
      setStatus(liveStatus);
      setSnap(liveStatus.state === "connected" || snapshot ? snapshot : null);
      setRunning(liveStatus.state !== "disconnected");
      setVrStatus(vr);
      setMonitorStatus(monitor);
      setAudioStatus(audio);
      setSettings(cfg);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    refresh();
    const unsubs = Promise.all([
      onLiveTelemetry((payload) => {
        setSnap(payload);
        setRunning(true);
      }),
      onLiveStatus((payload) => {
        setStatus(payload);
        setRunning(payload.state !== "disconnected");
      }),
    ]);
    const audioPoll = setInterval(() => {
      getAudioCoachStatus().then(setAudioStatus).catch(() => undefined);
    }, 2000);
    return () => {
      clearInterval(audioPoll);
      unsubs.then((fns) => fns.forEach((fn) => fn())).catch(() => undefined);
    };
  }, [refresh]);

  useEffect(() => {
    if (!vrStatus?.active) {
      setVrHudHealthy(null);
      return;
    }
    let cancelled = false;
    const poll = () => {
      checkVrHudHealth()
        .then((ok) => {
          if (!cancelled) setVrHudHealthy(ok);
        })
        .catch(() => {
          if (!cancelled) setVrHudHealthy(false);
        });
    };
    poll();
    const id = setInterval(poll, 3000);
    return () => {
      cancelled = true;
      clearInterval(id);
    };
  }, [vrStatus?.active]);

  useEffect(() => {
    if (!vrStatus?.active || vrStatus.mode !== "native") return;
    const id = setInterval(() => {
      getNativeVrStatus()
        .then(setNativeVr)
        .catch(() => undefined);
    }, 2000);
    return () => clearInterval(id);
  }, [vrStatus?.active, vrStatus?.mode]);

  const refreshLayerDiag = useCallback(async () => {
    try {
      setLayerDiag(await getVrLayerDiagnostics());
    } catch {
      setLayerDiag(null);
    }
  }, []);

  useEffect(() => {
    refreshLayerDiag();
  }, [refreshLayerDiag, vrStatus?.layerInstalled]);

  const hudUrl =
    settings && vrStatus?.hudUrl
      ? buildOpenKneeboardUrl(settings, vrStatus.hudUrl.split("?")[0])
      : vrStatus?.hudUrl ?? VR_PREVIEW_URL;

  const run = async (label: string, fn: () => Promise<void>) => {
    setError(null);
    try {
      await fn();
      await refresh();
    } catch (e) {
      const msg = `${label}: ${String(e)}`;
      setError(msg);
      showToast(msg, "error");
    }
  };

  const toggleAudioSetting = async (key: keyof AppSettings, value: boolean) => {
    try {
      const next = await patchSettings({ [key]: value } as Partial<AppSettings>);
      setSettings(next);
    } catch (e) {
      showToast(`Settings update failed: ${String(e)}`, "error");
    }
  };

  const vrSaveTimer = useRef<number | null>(null);
  useEffect(
    () => () => {
      if (vrSaveTimer.current !== null) window.clearTimeout(vrSaveTimer.current);
    },
    [],
  );

  // Only the coach slot's VR fields change here; desktop* stays with the monitor overlay.
  const updateCoachVr = (patch: Partial<WidgetPlacement>) => {
    if (!settings) return;
    const widgets = settings.overlayLayout.widgets.map((w, i) => (i === 0 ? { ...w, ...patch } : w));
    const next = { ...settings, overlayLayout: { ...settings.overlayLayout, widgets } };
    setSettings(next);
    if (vrSaveTimer.current !== null) window.clearTimeout(vrSaveTimer.current);
    vrSaveTimer.current = window.setTimeout(() => {
      saveSettings(next).catch((e) => showToast(`VR settings save failed: ${String(e)}`, "error"));
    }, 150);
  };

  const coachPlacement = settings?.overlayLayout?.widgets?.[0];
  const fieldPace = settings?.overlayLayout?.fieldPaceMode ?? "best";
  const trackMap = useTrackMap(snap?.track);
  const lapTrail = useLapTrail(snap);

  return (
    <div className="live-page">
      <div className="panel live-header-panel">
        <div className="live-header-row">
          <div>
            <h2>Live telemetry</h2>
            <span className="muted">Real-time data from iRacing shared memory</span>
          </div>
          <span className={stateClass(status.state)}>{stateLabel(status.state)}</span>
        </div>
        <p className="live-status-msg">{status.message}</p>
        {!status.message.toLowerCase().includes("mem") && status.state === "disconnected" ? (
          <p className="muted small">
            Tip: set <code>irsdkEnableMem=1</code> in Documents\iRacing\app.ini for live telemetry.
          </p>
        ) : null}

        <div className="btn-row live-actions">
          {!running ? (
            <button type="button" className="btn btn-primary" onClick={() => run("Start live", startLiveMonitor)}>
              Start live monitor
            </button>
          ) : (
            <button type="button" className="btn" onClick={() => run("Stop live", stopLiveMonitor)}>
              Stop live
            </button>
          )}
          {!demoRunning ? (
            <button
              type="button"
              className="btn"
              onClick={() =>
                run("Start demo", async () => {
                  await startDemoClock();
                  setDemoRunning(true);
                })
              }
            >
              Start Demo
            </button>
          ) : (
            <button
              type="button"
              className="btn"
              onClick={() =>
                run("Stop demo", async () => {
                  await stopDemoClock();
                  setDemoRunning(false);
                })
              }
            >
              Stop Demo
            </button>
          )}
        </div>
        {error && <p className="live-error">{error}</p>}
      </div>

      <div className="live-columns">
        <div className="live-main">
          {!snap ? (
            <div className="empty-state panel" style={{ height: "auto", minHeight: 180 }}>
              <p className="muted">
                {running || demoRunning
                  ? "Waiting for telemetry…"
                  : "Start the live monitor (or Demo) to see metrics."}
              </p>
            </div>
          ) : (
            <>
              <div className="panel live-meta">
                <div className="panel-body" style={{ display: "flex", justifyContent: "space-between" }}>
                  <div>
                    <strong>{snap.track || "Live session"}</strong>
                    <span className="muted"> · {snap.car || "—"}</span>
                  </div>
                  <span className="muted">{snap.sessionType}</span>
                </div>
              </div>

              <div className="live-metrics panel">
                <div className="panel-header">
                  <h2>Metrics</h2>
                </div>
                <div className="panel-body live-metrics-grid">
                  <Metric label="Lap" value={String(snap.lap)} />
                  <Metric label="Lap time" value={formatLapTime(snap.lapTimeMs)} />
                  <Metric label="Last" value={formatLapTime(snap.lastLapMs)} />
                  <Metric label="Best" value={formatLapTime(snap.bestLapMs)} />
                  <Metric label="Δ Best" value={formatDelta(snap.deltaToBestMs)} />
                  <Metric label="Fuel" value={formatLiters(snap.fuelLevel)} />
                  <Metric label="Speed" value={`${Math.round(snap.speed)}`} />
                  <Metric label="Pos" value={snap.playerPosition != null ? `P${snap.playerPosition}` : "—"} />
                  <Metric
                    label="Class"
                    value={snap.playerClassPosition != null ? `P${snap.playerClassPosition}` : "—"}
                  />
                  <Metric label="Inc" value={String(snap.incidentCount)} />
                  <Metric label="LF" value={formatTemp(snap.lfTemp)} />
                  <Metric label="RF" value={formatTemp(snap.rfTemp)} />
                  <Metric label="LR" value={formatTemp(snap.lrTemp)} />
                  <Metric label="RR" value={formatTemp(snap.rrTemp)} />
                </div>
              </div>

              
              <div className="panel">
                <div className="panel-header">
                  <h2>Track map</h2>
                  <span className="muted">{snap.track}</span>
                </div>
                <div className="panel-body">
                  <div className="pw-widget live-trackmap-preview">
                    <TrackMapWidget
                      outline={trackMap}
                      snap={snap}
                      candidateTraces={lapTrail}
                    />
                  </div>
                </div>
              </div>

              <SessionLeaderboard competitors={snap.competitors} />

              <div className="panel">
                <div className="panel-header">
                  <h2>Coach HUD preview</h2>
                </div>
                <div className="panel-body">
                  <div className="pw-widget live-coach-preview">
                    <CoachWidget snap={snap} fieldPaceMode={fieldPace} />
                  </div>
                </div>
              </div>
            </>
          )}
        </div>

        <aside className="live-side">
          <div className="panel">
            <div className="panel-header">
              <h2>Audio coach</h2>
            </div>
            <div className="panel-body">
              <div className="btn-row">
                <button
                  type="button"
                  className="btn btn-primary"
                  onClick={() =>
                    run("Test coach", async () => {
                      const message = coachTestMessage(await testAudioCoach());
                      if (message) showToast(message);
                    })
                  }
                >
                  Test Coach
                </button>
                <button
                  type="button"
                  className="btn"
                  onClick={() =>
                    run(
                      audioStatus?.active ? "Stop coach" : "Start coach",
                      audioStatus?.active ? stopAudioCoach : startAudioCoach,
                    )
                  }
                >
                  {audioStatus?.active ? "Stop coach" : "Start coach"}
                </button>
              </div>
              {audioStatus ? (
                <div className="btn-row">
                  <span className="muted small">Voice: {audioStatus.packName}</span>
                  <TierBadges spotter={audioStatus.spotter} engineer={audioStatus.engineer} />
                </div>
              ) : null}
              {audioStatus?.lastMessage ? (
                <p className="audio-coach-last muted small">
                  <strong>Last spoken:</strong> {audioStatus.lastMessage}
                </p>
              ) : (
                <p className="muted small">No message yet.</p>
              )}
              {settings ? (
                <div className="audio-toggles">
                  <Toggle
                    label="Pack alerts"
                    checked={settings.audioPackAlertsEnabled}
                    onChange={(v) => toggleAudioSetting("audioPackAlertsEnabled", v)}
                  />
                  <Toggle
                    label="Flags"
                    checked={settings.audioFlagsEnabled}
                    onChange={(v) => toggleAudioSetting("audioFlagsEnabled", v)}
                  />
                  <Toggle
                    label="Incidents"
                    checked={settings.audioIncidentsEnabled}
                    onChange={(v) => toggleAudioSetting("audioIncidentsEnabled", v)}
                  />
                  <Toggle
                    label="Pace"
                    checked={settings.audioPaceEnabled}
                    onChange={(v) => toggleAudioSetting("audioPaceEnabled", v)}
                  />
                </div>
              ) : (
                <p className="muted small">Settings unavailable until backend registers.</p>
              )}
            </div>
          </div>

          <div className="panel">
            <div className="panel-header">
              <h2>Monitor overlays</h2>
            </div>
            <div className="panel-body">
              <p className="muted small">
                Always-on-top transparent windows for each enabled widget. Shared
                enable flags; desktop placement uses <code>desktopX/Y/W/H</code>.
              </p>
              <div className="btn-row">
                <button
                  type="button"
                  className="btn btn-primary"
                  onClick={() =>
                    run(
                      monitorStatus?.active ? "Stop monitor" : "Start monitor",
                      monitorStatus?.active ? stopMonitorOverlay : startMonitorOverlay,
                    )
                  }
                >
                  {monitorStatus?.active ? "Stop monitor overlays" : "Start monitor overlays"}
                </button>
              </div>
              <p className="muted small">{monitorStatus?.message || "Monitor overlay idle"}</p>
              {monitorStatus?.windows?.length ? (
                <p className="muted small">
                  Open: {monitorStatus.windows.join(", ")}
                </p>
              ) : null}
            </div>
          </div>

          <div className="panel">
            <div className="panel-header">
              <h2>VR / HUD</h2>
            </div>
            <div className="panel-body">
              <div className="btn-row">
                <button
                  type="button"
                  className="btn"
                  onClick={() =>
                    run(layerDiag?.registered ? "Reinstall layer" : "Install layer", installVrLayer)
                  }
                >
                  {layerDiag?.registered ? "Reinstall layer" : "Install VR layer"}
                </button>
                {layerDiag?.registered ? (
                  <button
                    type="button"
                    className="btn btn-ghost"
                    onClick={() => run("Uninstall layer", uninstallVrLayer)}
                  >
                    Uninstall
                  </button>
                ) : null}
                <button
                  type="button"
                  className="btn"
                  onClick={() =>
                    run(
                      vrStatus?.active ? "Stop HUD" : "Start HUD",
                      vrStatus?.active ? stopVrOverlay : startVrOverlay,
                    )
                  }
                >
                  {vrStatus?.active ? "Stop HUD" : "Start HUD"}
                </button>
                {vrStatus?.active ? (
                  <button
                    type="button"
                    className="btn"
                    title="Re-anchor world-locked widgets in front of your current head position"
                    onClick={() => run("Recenter", recenterVr)}
                  >
                    Recenter
                  </button>
                ) : null}
                <button
                  type="button"
                  className="btn"
                  onClick={() =>
                    run("Open preview", async () => {
                      try {
                        await openVrHudPreview();
                      } catch {
                        window.open(VR_PREVIEW_URL, "_blank", "noopener,noreferrer");
                      }
                    })
                  }
                >
                  Browser preview
                </button>
              </div>
              <p className="muted small">
                Preview URL: <code>{hudUrl || VR_PREVIEW_URL}</code>
              </p>
              {vrHudHealthy === true && (
                <span className="vr-health-pill vr-health-ok">HUD server ready</span>
              )}
              {vrHudHealthy === false && (
                <span className="vr-health-pill vr-health-err">HUD server not responding</span>
              )}

              {coachPlacement ? (
                <div className="vr-sliders">
                  <h3>Coach in headset</h3>
                  <Slider
                    label="Size"
                    value={coachPlacement.vrScale}
                    min={0.25}
                    max={1.25}
                    step={0.05}
                    format={(v) => `${v.toFixed(2)}×`}
                    onChange={(v) => updateCoachVr({ vrScale: v })}
                  />
                  <Slider
                    label="Opacity"
                    value={coachPlacement.vrOpacity}
                    min={0.2}
                    max={1}
                    step={0.05}
                    format={(v) => `${Math.round(v * 100)}%`}
                    onChange={(v) => updateCoachVr({ vrOpacity: v })}
                  />
                  <Slider
                    label="Height"
                    value={coachPlacement.vrOffsetY}
                    min={-0.4}
                    max={0.4}
                    step={0.02}
                    format={(v) => `${v >= 0 ? "+" : ""}${v.toFixed(2)} m`}
                    onChange={(v) => updateCoachVr({ vrOffsetY: v })}
                  />
                </div>
              ) : null}

              <div className="vr-checklist">
                <h3>RaceLab-off checklist</h3>
                <ul className="muted small">
                  <li>Disable RaceLab VR (and other OpenXR API layers) before using Race Refinery native.</li>
                  <li>Install the Race Refinery OpenXR layer once, then restart iRacing in OpenXR / VR mode.</li>
                  <li>Only one implicit OpenXR layer stack should be active — two layers fight for compositing.</li>
                  <li>Confirm layer diagnostics show ready, then Start HUD and verify the test-pattern quad.</li>
                </ul>
              </div>

              <div className="vr-diag">
                <h3>Diagnostics</h3>
                <dl className="diag-grid">
                  <dt>VR message</dt>
                  <dd>{vrStatus?.message || "—"}</dd>
                  <dt>Mode</dt>
                  <dd>{vrStatus?.mode || "—"}</dd>
                  <dt>Layer installed</dt>
                  <dd>{String(vrStatus?.layerInstalled ?? layerDiag?.registered ?? "—")}</dd>
                  <dt>Layer ready</dt>
                  <dd>{layerDiag ? String(layerDiag.ready) : "—"}</dd>
                  <dt>DLL present</dt>
                  <dd>{layerDiag ? String(layerDiag.dllPresent) : "—"}</dd>
                  <dt>Compositor</dt>
                  <dd>{nativeVr ? String(nativeVr.compositorActive) : "—"}</dd>
                  <dt>Telemetry pub</dt>
                  <dd>{nativeVr ? String(nativeVr.telemetryPublishing) : "—"}</dd>
                  <dt>Write age</dt>
                  <dd>
                    {nativeVr?.writeAgeMs != null
                      ? `${nativeVr.writeAgeMs} ms`
                      : nativeVr?.lastFrameAgeMs != null
                        ? `${nativeVr.lastFrameAgeMs} ms`
                        : "—"}
                  </dd>
                  <dt>Overlay count</dt>
                  <dd>{nativeVr != null ? String(nativeVr.overlayCount) : "—"}</dd>
                  <dt>Last error</dt>
                  <dd>{nativeVr?.lastError || "—"}</dd>
                </dl>
                {layerDiag && layerDiag.issues.length > 0 ? (
                  <ul className="muted small">
                    {layerDiag.issues.map((issue) => (
                      <li key={issue}>{issue}</li>
                    ))}
                  </ul>
                ) : null}
              </div>
            </div>
          </div>
        </aside>
      </div>
    </div>
  );
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div className="fact">
      <span className="fact-label">{label}</span>
      <span className="fact-value">{value}</span>
    </div>
  );
}

function Toggle({
  label,
  checked,
  onChange,
}: {
  label: string;
  checked: boolean;
  onChange: (v: boolean) => void;
}) {
  return (
    <label className="toggle-row">
      <input type="checkbox" checked={checked} onChange={(e) => onChange(e.target.checked)} />
      <span>{label}</span>
    </label>
  );
}
