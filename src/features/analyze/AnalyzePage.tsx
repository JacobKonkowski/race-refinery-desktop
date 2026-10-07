import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  checkIracingConfig,
  confirmDialog,
  deleteSession,
  getSession,
  listSessions,
  onImportComplete,
  reimportSession,
} from "../../shared/api";
import { showToast } from "../../shared/toast";
import { useTrackMap } from "../../shared/useTrackMap";
import type {
  IracingConfigCheck,
  LapSummary,
  SessionDetail,
  SessionSummary,
} from "../../shared/types";
import { ComparePanel } from "./ComparePanel";
import { ConfigBanner } from "./ConfigBanner";
import { FuelTirePanel } from "./FuelTirePanel";
import { InsightsStrip } from "./InsightsStrip";
import { LapTable } from "./LapTable";
import { SessionBrowser } from "./SessionBrowser";
import { SessionHeader } from "./SessionHeader";
import { TrackMapPanel } from "./TrackMapPanel";
import { computeSessionStats } from "./sessionStats";
import { useImportActions } from "./useImportActions";

const LAST_SESSION_KEY = "raceRefinery.lastSessionId";

/** Fastest pace-eligible lap in the session (the default compare reference). */
function defaultReferenceLap(laps: LapSummary[]): LapSummary | null {
  return laps
    .filter((l) => l.paceEligible && l.lapTimeMs != null)
    .reduce<LapSummary | null>((best, l) => {
      if (!best || (l.lapTimeMs ?? Infinity) < (best.lapTimeMs ?? Infinity)) return l;
      return best;
    }, null);
}

function readLastSessionId(): number | null {
  try {
    const raw = localStorage.getItem(LAST_SESSION_KEY);
    if (!raw) return null;
    const n = Number(raw);
    return Number.isFinite(n) ? n : null;
  } catch {
    return null;
  }
}

function writeLastSessionId(id: number | null) {
  try {
    if (id == null) localStorage.removeItem(LAST_SESSION_KEY);
    else localStorage.setItem(LAST_SESSION_KEY, String(id));
  } catch {
    /* ignore quota / private mode */
  }
}

export function AnalyzePage() {
  const [sessions, setSessions] = useState<SessionSummary[]>([]);
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [detail, setDetail] = useState<SessionDetail | null>(null);
  const [loadingDetail, setLoadingDetail] = useState(false);
  const [candidateLapId, setCandidateLapId] = useState<number | null>(null);
  const [referenceLapId, setReferenceLapId] = useState<number | null>(null);
  const [config, setConfig] = useState<IracingConfigCheck | null>(null);
  /** Lap fraction hovered on the compare charts, mirrored on the track map. */
  const [highlightPct, setHighlightPct] = useState<number | null>(null);
  /** Corner-table / chart focus: zoom the map there. `seq` re-triggers the same pct. */
  const [mapFocus, setMapFocus] = useState<{ pct: number; seq: number } | null>(null);
  const focusMapAt = useCallback(
    (pct: number) => setMapFocus((prev) => ({ pct, seq: (prev?.seq ?? 0) + 1 })),
    [],
  );
  const [reimporting, setReimporting] = useState(false);
  const [paceOnly, setPaceOnly] = useState(false);
  const [moreLapColumns, setMoreLapColumns] = useState(false);
  /** Suppresses auto-select on each import-complete during "Re-import all". */
  const bulkReimport = useRef(false);
  const importActions = useImportActions();

  const selectSession = useCallback((id: number | null) => {
    setSelectedId(id);
    writeLastSessionId(id);
  }, []);

  const refreshSessions = useCallback(async (): Promise<SessionSummary[]> => {
    const list = await listSessions();
    setSessions(list);
    return list;
  }, []);

  useEffect(() => {
    refreshSessions()
      .then((list) => {
        setSelectedId((prev) => {
          if (prev != null && list.some((s) => s.id === prev)) return prev;
          const stored = readLastSessionId();
          if (stored != null && list.some((s) => s.id === stored)) return stored;
          return list[0]?.id ?? null;
        });
      })
      .catch((e) => {
        console.error("listSessions failed", e);
        showToast(`Failed to list sessions: ${String(e)}`, "error");
      });
    checkIracingConfig().then(setConfig).catch(() => undefined);
  }, [refreshSessions]);

  useEffect(() => {
    const unlisten = onImportComplete(async (sessionId) => {
      if (bulkReimport.current) return;
      const list = await refreshSessions();
      if (sessionId && list.some((s) => s.id === sessionId)) {
        selectSession(sessionId);
      }
    });
    return () => {
      unlisten.then((fn) => fn()).catch(() => undefined);
    };
  }, [refreshSessions, selectSession]);

  useEffect(() => {
    if (selectedId == null) {
      setDetail(null);
      return;
    }
    writeLastSessionId(selectedId);
    let cancelled = false;
    setLoadingDetail(true);
    getSession(selectedId)
      .then((d) => {
        if (cancelled) return;
        setDetail(d);
        const ref = d ? defaultReferenceLap(d.laps) : null;
        setReferenceLapId(ref?.id ?? null);
        setCandidateLapId(null);
        setMapFocus(null);
      })
      .catch((e) => {
        console.error("getSession failed", e);
        showToast(`Failed to load session: ${String(e)}`, "error");
      })
      .finally(() => !cancelled && setLoadingDetail(false));
    return () => {
      cancelled = true;
    };
  }, [selectedId]);

  const handleDelete = useCallback(
    async (sessionId: number) => {
      const ok = await confirmDialog(
        "Delete this session and its laps from the local database? This cannot be undone.",
        "Delete session",
      );
      if (!ok) return;
      try {
        await deleteSession(sessionId);
        const list = await refreshSessions();
        setSelectedId((prev) => {
          const next = prev === sessionId ? list[0]?.id ?? null : prev;
          writeLastSessionId(next);
          return next;
        });
      } catch (e) {
        showToast(`Delete failed: ${String(e)}`, "error");
      }
    },
    [refreshSessions],
  );

  const handleDeleteAll = useCallback(async () => {
    if (sessions.length === 0) return;
    const ok = await confirmDialog(
      `Delete all ${sessions.length} session(s) and their laps from the local database? This cannot be undone.`,
      "Delete all sessions",
    );
    if (!ok) return;
    try {
      for (const s of sessions) {
        await deleteSession(s.id);
      }
      await refreshSessions();
      setSelectedId(null);
      writeLastSessionId(null);
    } catch (e) {
      showToast(`Delete failed: ${String(e)}`, "error");
      await refreshSessions();
    }
  }, [sessions, refreshSessions]);

  const handleReimport = useCallback(
    async (sessionId: number) => {
      setReimporting(true);
      try {
        const newId = await reimportSession(sessionId);
        await refreshSessions();
        selectSession(newId);
        showToast("Session re-imported with the latest analysis.", "success");
      } catch (e) {
        showToast(String(e), "error");
      } finally {
        setReimporting(false);
      }
    },
    [refreshSessions, selectSession],
  );

  const handleReimportAll = useCallback(async () => {
    if (sessions.length === 0) return;
    const ok = await confirmDialog(
      `Re-analyze all ${sessions.length} session(s) from their IBT files? Sessions whose file is gone are left as they are.`,
      "Re-import all sessions",
    );
    if (!ok) return;
    setReimporting(true);
    bulkReimport.current = true;
    let done = 0;
    const failed: string[] = [];
    let nextSelected = selectedId;
    try {
      for (const s of sessions) {
        try {
          const newId = await reimportSession(s.id);
          if (s.id === selectedId) nextSelected = newId;
          done += 1;
        } catch {
          failed.push(s.track || `session ${s.id}`);
        }
      }
    } finally {
      bulkReimport.current = false;
      setReimporting(false);
      await refreshSessions();
      selectSession(nextSelected);
    }
    showToast(
      failed.length === 0
        ? `Re-imported ${done} session(s).`
        : `Re-imported ${done}; ${failed.length} skipped (IBT missing or unreadable).`,
      failed.length === 0 ? "success" : "info",
    );
  }, [sessions, selectedId, refreshSessions, selectSession]);

  const laps = detail?.laps ?? [];
  const stats = useMemo(() => computeSessionStats(laps), [laps]);
  const sessionTypes = useMemo(
    () => [...new Set(laps.map((l) => l.sessionType).filter(Boolean))],
    [laps],
  );
  const trackMap = useTrackMap(detail?.session.track);
  const hasEligible = useMemo(() => laps.some((l) => l.paceEligible), [laps]);
  const okChannelPresent = useMemo(
    () => laps.some((l) => l.deltaBestOk !== null),
    [laps],
  );
  const candidate = laps.find((l) => l.id === candidateLapId) ?? null;
  const reference = laps.find((l) => l.id === referenceLapId) ?? null;

  return (
    <div className="analyze">
      <SessionBrowser
        sessions={sessions}
        selectedId={selectedId}
        onSelect={selectSession}
        onDelete={handleDelete}
        onDeleteAll={handleDeleteAll}
        onReimportAll={handleReimportAll}
        reimporting={reimporting}
      />
      <div className="analyze-workspace">
        {selectedId == null ? (
          <EmptyState config={config} importActions={importActions} />
        ) : loadingDetail ? (
          <div className="loading">Loading session…</div>
        ) : !detail ? (
          <div className="loading">Session not found.</div>
        ) : (
          <>
            <div className="analyze-session-bar panel">
              <SessionHeader
                session={detail.session}
                stats={stats}
                sessionTypes={sessionTypes}
                onReimport={() => handleReimport(detail.session.id)}
                reimporting={reimporting}
              />
            </div>

            <InsightsStrip stats={stats} />

            {!hasEligible && laps.length > 0 && !okChannelPresent ? (
              <ConfigBanner />
            ) : null}

            <div className="analyze-main">
              <ComparePanel
                laps={laps}
                candidate={candidate}
                reference={reference}
                onChangeReference={setReferenceLapId}
                onHoverDistPct={setHighlightPct}
                onFocusDistPct={focusMapAt}
              />
              <div className="analyze-map-cell">
                <TrackMapPanel
                  outline={trackMap}
                  track={detail.session.track}
                  candidate={candidate}
                  reference={reference}
                  highlightPct={highlightPct}
                  focus={mapFocus}
                />
              </div>
            </div>

            <div className="panel analyze-lap-picker">
              <div className="panel-header">
                <h2>Laps</h2>
                <div className="lap-picker-tools">
                  <label className="lap-filter">
                    <input
                      type="checkbox"
                      checked={paceOnly}
                      onChange={(e) => setPaceOnly(e.target.checked)}
                    />
                    Pace only
                  </label>
                  <button
                    type="button"
                    className={`btn btn-ghost${moreLapColumns ? " on" : ""}`}
                    onClick={() => setMoreLapColumns((v) => !v)}
                  >
                    {moreLapColumns ? "Fewer columns" : "More columns"}
                  </button>
                </div>
              </div>
              <div className="lap-select-help muted">
                <div className="lap-legend">
                  <span>
                    <span className="swatch cand" /> Click a lap to compare
                  </span>
                  <span>
                    <span className="swatch ref" /> Right-click (or Shift-click) to set
                    reference
                  </span>
                </div>
                {candidateLapId != null &&
                (referenceLapId == null || referenceLapId === candidateLapId) ? (
                  <p className="lap-ref-hint">Right-click another lap as reference.</p>
                ) : null}
              </div>
              <div className="panel-body" style={{ padding: 0 }}>
                <LapTable
                  laps={laps}
                  candidateLapId={candidateLapId}
                  referenceLapId={referenceLapId}
                  onSelectCandidate={setCandidateLapId}
                  onSelectReference={setReferenceLapId}
                  paceOnly={paceOnly}
                  showTireTemps={moreLapColumns}
                />
              </div>
            </div>

            <details className="analyze-secondary panel">
              <summary>Fuel &amp; tires</summary>
              <div className="panel-body">
                <FuelTirePanel laps={laps} />
              </div>
            </details>
          </>
        )}
      </div>
    </div>
  );
}

function EmptyState({
  config,
  importActions,
}: {
  config: IracingConfigCheck | null;
  importActions: ReturnType<typeof useImportActions>;
}) {
  const { busy, handleImport, handleScan } = importActions;
  return (
    <div className="empty-state">
      <h2>No sessions yet</h2>
      <p className="muted">
        Import an iRacing <code>.ibt</code> file, or scan your telemetry folder.
        <br />
        Record telemetry in the sim with <strong>Alt+L</strong>.
      </p>
      {config && !config.diskEnabled ? (
        <p className="muted">
          Tip: set <code>irsdkEnableDisk=1</code> in{" "}
          <code>Documents\iRacing\app.ini</code> to record IBT files.
        </p>
      ) : null}
      <div className="empty-actions">
        <button className="btn" onClick={handleScan} disabled={busy}>
          Scan folder
        </button>
        <button className="btn btn-primary" onClick={handleImport} disabled={busy}>
          Import IBT
        </button>
      </div>
    </div>
  );
}
