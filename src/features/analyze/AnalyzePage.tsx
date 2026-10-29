import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import {
  checkIracingConfig,
  confirmDialog,
  deleteSession,
  getSession,
  listLibrary,
  loadLaps,
  onImportComplete,
  reimportSession,
} from "../../shared/api";
import { formatDate } from "../../shared/format";
import { showToast } from "../../shared/toast";
import { useTrackMap } from "../../shared/useTrackMap";
import type {
  IracingConfigCheck,
  LapSummary,
  LibrarySession,
  SessionDetail,
  SetLap,
  TrackOutline,
} from "../../shared/types";
import { ComparePanel } from "./ComparePanel";
import { ConfigBanner } from "./ConfigBanner";
import { ExplorerSidebar } from "./ExplorerSidebar";
import { FuelTirePanel } from "./FuelTirePanel";
import { InsightsStrip } from "./InsightsStrip";
import { LapTable } from "./LapTable";
import { SessionHeader } from "./SessionHeader";
import { TrackMapPanel } from "./TrackMapPanel";
import { computeSessionStats, type SessionStats } from "./sessionStats";
import { useImportActions } from "./useImportActions";

type Workspace =
  | { kind: "session" }
  | {
      kind: "selection";
      lapIds: number[];
      track: string;
    };

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

function setGroupLabel(rows: SetLap[]): (lap: LapSummary) => {
  key: string;
  label: string;
  date: string;
  car: string;
  sessionType: string;
} {
  const byId = new Map(rows.map((row) => [row.lap.id, row]));
  return (lap) => {
    const row = byId.get(lap.id);
    return {
      key: `${row?.sessionId ?? 0}:${lap.sessionNum}`,
      label: [row ? formatDate(row.sessionDate) : "", row?.car ?? "", lap.sessionType]
        .filter(Boolean)
        .join(" · "),
      date: row ? formatDate(row.sessionDate) : "",
      car: row?.car ?? "",
      sessionType: lap.sessionType,
    };
  };
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
  const [library, setLibrary] = useState<LibrarySession[]>([]);
  const [workspace, setWorkspace] = useState<Workspace>({ kind: "session" });
  const [selectionRows, setSelectionRows] = useState<SetLap[]>([]);
  const [selectionError, setSelectionError] = useState<string | null>(null);
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [detail, setSession] = useState<SessionDetail | null>(null);
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
  /** Lap-id key already loaded into `selectionRows`, so a save does not fetch twice. */
  const loadedKey = useRef("");
  const [settledKey, setSettledKey] = useState("");
  const workspaceKindRef = useRef(workspace.kind);
  workspaceKindRef.current = workspace.kind;
  const importActions = useImportActions();

  const sessions = useMemo(() => library.map((row) => row.session), [library]);

  const selectSession = useCallback((id: number | null) => {
    setSelectedId(id);
    writeLastSessionId(id);
  }, []);

  const refreshLibrary = useCallback(async (): Promise<LibrarySession[]> => {
    const nextLibrary = await listLibrary();
    setLibrary(nextLibrary);
    return nextLibrary;
  }, []);

  useEffect(() => {
    refreshLibrary()
      .then((list) => {
        const ids = list.map((row) => row.session);
        setSelectedId((prev) => {
          if (prev != null && ids.some((s) => s.id === prev)) return prev;
          const stored = readLastSessionId();
          if (stored != null && ids.some((s) => s.id === stored)) return stored;
          return ids[0]?.id ?? null;
        });
      })
      .catch((e) => {
        console.error("listLibrary failed", e);
        showToast(`Failed to list sessions: ${String(e)}`, "error");
      });
    checkIracingConfig().then(setConfig).catch(() => undefined);
  }, [refreshLibrary]);

  useEffect(() => {
    const unlisten = onImportComplete(async (sessionId) => {
      if (bulkReimport.current) return;
      const list = await refreshLibrary();
      if (sessionId && list.some((row) => row.session.id === sessionId)) {
        setWorkspace({ kind: "session" });
        selectSession(sessionId);
      }
    });
    return () => {
      unlisten.then((fn) => fn()).catch(() => undefined);
    };
  }, [refreshLibrary, selectSession]);

  useEffect(() => {
    if (selectedId == null) {
      setSession(null);
      return;
    }
    writeLastSessionId(selectedId);
    let cancelled = false;
    setLoadingDetail(true);
    getSession(selectedId)
      .then((d) => {
        if (cancelled) return;
        setSession(d);
        if (workspaceKindRef.current !== "session") return;
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

  // Opening a session again after a selection must not keep another outing's lap ids.
  useEffect(() => {
    if (workspace.kind !== "session" || detail == null || detail.session.id !== selectedId) return;
    setReferenceLapId((prev) =>
      prev != null && detail.laps.some((lap) => lap.id === prev)
        ? prev
        : (defaultReferenceLap(detail.laps)?.id ?? null),
    );
    setCandidateLapId((prev) =>
      prev != null && detail.laps.some((lap) => lap.id === prev) ? prev : null,
    );
  }, [workspace, detail, selectedId]);

  const handleDelete = useCallback(
    async (sessionId: number) => {
      const ok = await confirmDialog(
        "Delete this session and its laps from the local database? This cannot be undone.",
        "Delete session",
      );
      if (!ok) return;
      try {
        await deleteSession(sessionId);
        const list = await refreshLibrary();
        setSelectedId((prev) => {
          const next = prev === sessionId ? list[0]?.session.id ?? null : prev;
          writeLastSessionId(next);
          return next;
        });
        setWorkspace((prev) => {
          if (prev.kind !== "selection") return prev;
          const live = new Set(list.flatMap((row) => row.laps.map((lap) => lap.id)));
          const lapIds = prev.lapIds.filter((id) => live.has(id));
          if (lapIds.length === prev.lapIds.length) return prev;
          return { ...prev, lapIds };
        });
      } catch (e) {
        showToast(`Delete failed: ${String(e)}`, "error");
      }
    },
    [refreshLibrary],
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
      await refreshLibrary();
      setSelectedId(null);
      writeLastSessionId(null);
      setWorkspace({ kind: "session" });
    } catch (e) {
      showToast(`Delete failed: ${String(e)}`, "error");
      await refreshLibrary();
    }
  }, [sessions, refreshLibrary]);

  const handleReimport = useCallback(
    async (sessionId: number) => {
      setReimporting(true);
      try {
        const newId = await reimportSession(sessionId);
        await refreshLibrary();
        setWorkspace({ kind: "session" });
        selectSession(newId);
        showToast("Session re-imported with the latest analysis.", "success");
      } catch (e) {
        showToast(String(e), "error");
      } finally {
        setReimporting(false);
      }
    },
    [refreshLibrary, selectSession],
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
      await refreshLibrary();
      setWorkspace({ kind: "session" });
      selectSession(nextSelected);
    }
    showToast(
      failed.length === 0
        ? `Re-imported ${done} session(s).`
        : `Re-imported ${done}; ${failed.length} skipped (IBT missing or unreadable).`,
      failed.length === 0 ? "success" : "info",
    );
  }, [sessions, selectedId, refreshLibrary, selectSession]);

  const workspaceKind = workspace.kind;
  const selectionKey = workspace.kind === "selection" ? workspace.lapIds.join(",") : "";

  useEffect(() => {
    if (workspaceKind !== "selection") return;
    if (selectionKey === "") {
      loadedKey.current = "";
      setSettledKey("");
      setSelectionRows([]);
      setSelectionError(null);
      return;
    }
    if (loadedKey.current === selectionKey) return;
    const ids = selectionKey.split(",").map(Number);
    let cancelled = false;
    setSelectionError(null);
    loadLaps(ids)
      .then((rows) => {
        if (cancelled) return;
        loadedKey.current = selectionKey;
        setSettledKey(selectionKey);
        setSelectionRows(rows);
        const nextLaps = rows.map((row) => row.lap);
        setReferenceLapId((prev) =>
          prev != null && nextLaps.some((lap) => lap.id === prev)
            ? prev
            : (defaultReferenceLap(nextLaps)?.id ?? null),
        );
        setCandidateLapId((prev) =>
          prev != null && nextLaps.some((lap) => lap.id === prev) ? prev : null,
        );
        setMapFocus(null);
      })
      .catch((e) => {
        if (cancelled) return;
        console.error("loadLaps failed", e);
        loadedKey.current = "";
        setSettledKey(selectionKey);
        setSelectionRows([]);
        setSelectionError(String(e));
        showToast(`Failed to load laps: ${String(e)}`, "error");
      });
    return () => {
      cancelled = true;
    };
  }, [workspaceKind, selectionKey]);

  const currentSelection = useCallback((): { ids: number[]; track: string | null } => {
    if (workspace.kind === "selection") {
      return {
        ids: workspace.lapIds,
        track: workspace.lapIds.length > 0 ? workspace.track : null,
      };
    }
    const row = library.find((entry) => entry.session.id === selectedId);
    return {
      ids: row?.laps.map((lap) => lap.id) ?? [],
      track: row && row.laps.length > 0 ? row.session.track : null,
    };
  }, [workspace, library, selectedId]);

  const commitSelection = useCallback(
    (ids: number[], track: string) => {
      if (ids.length === 0) {
        setWorkspace({ kind: "selection", lapIds: [], track: "" });
        return;
      }
      const whole = library.find((row) => {
        const lapIds = row.laps.map((lap) => lap.id);
        return lapIds.length === ids.length && lapIds.every((id) => ids.includes(id));
      });
      if (whole) {
        setWorkspace({ kind: "session" });
        selectSession(whole.session.id);
        return;
      }
      setWorkspace({ kind: "selection", lapIds: ids, track });
    },
    [library, selectSession],
  );

  const toggleLap = useCallback(
    (lapId: number) => {
      let track: string | null = null;
      for (const row of library) {
        if (row.laps.some((lap) => lap.id === lapId)) {
          track = row.session.track;
          break;
        }
      }
      if (track == null) return;
      const snap = currentSelection();
      const has = snap.ids.includes(lapId);
      if (!has && snap.ids.length > 0 && snap.track && snap.track !== track) {
        showToast("That lap is from a different track.", "error");
        return;
      }
      const ids = has ? snap.ids.filter((id) => id !== lapId) : [...snap.ids, lapId];
      commitSelection(ids, ids.length === 0 ? "" : snap.track || track);
    },
    [library, currentSelection, commitSelection],
  );

  const toggleSession = useCallback(
    (lapIds: number[]) => {
      if (lapIds.length === 0) return;
      let track: string | null = null;
      for (const row of library) {
        if (row.laps.some((lap) => lapIds.includes(lap.id))) {
          track = row.session.track;
          break;
        }
      }
      if (track == null) return;
      const snap = currentSelection();
      const allIn = lapIds.every((id) => snap.ids.includes(id));
      if (!allIn && snap.ids.length > 0 && snap.track && snap.track !== track) {
        showToast("That lap is from a different track.", "error");
        return;
      }
      const remove = new Set(lapIds);
      const ids = allIn
        ? snap.ids.filter((id) => !remove.has(id))
        : [...new Set([...snap.ids, ...lapIds])];
      commitSelection(ids, ids.length === 0 ? "" : snap.track || track);
    },
    [library, currentSelection, commitSelection],
  );

  const openSession = useCallback(
    (id: number) => {
      setWorkspace({ kind: "session" });
      selectSession(id);
    },
    [selectSession],
  );

  const activateLap = useCallback(
    (_sessionId: number, lapId: number) => {
      let track: string | null = null;
      for (const row of library) {
        if (row.laps.some((lap) => lap.id === lapId)) {
          track = row.session.track;
          break;
        }
      }
      if (track == null) return;
      const snap = currentSelection();
      if (!snap.ids.includes(lapId)) {
        if (snap.ids.length > 0 && snap.track && snap.track !== track) {
          showToast("That lap is from a different track.", "error");
          return;
        }
        commitSelection([...snap.ids, lapId], snap.track || track);
      }
      setCandidateLapId(lapId);
    },
    [library, currentSelection, commitSelection],
  );

  const laps = detail?.laps ?? [];
  const stats = useMemo(() => computeSessionStats(laps), [laps]);
  const sessionTypes = useMemo(
    () => [...new Set(laps.map((l) => l.sessionType).filter(Boolean))],
    [laps],
  );
  const selection = workspace.kind === "selection" ? workspace : null;
  const selectionLaps = selectionRows.map((row) => row.lap);
  const selectionReady =
    selection != null && settledKey === selection.lapIds.join(",") && selectionError == null;
  const visibleLaps = selection && selectionReady ? selectionLaps : laps;
  const mapTrack = selection && selection.lapIds.length > 0 ? selection.track : detail?.session.track;
  const trackMap = useTrackMap(mapTrack);
  const hasEligible = useMemo(() => laps.some((l) => l.paceEligible), [laps]);
  const okChannelPresent = useMemo(() => laps.some((l) => l.deltaBestOk !== null), [laps]);
  const candidate = visibleLaps.find((l) => l.id === candidateLapId) ?? null;
  const reference = visibleLaps.find((l) => l.id === referenceLapId) ?? null;
  const selectedIds = useMemo(() => {
    if (workspace.kind === "selection") return new Set(workspace.lapIds);
    if (selectedId == null) return new Set<number>();
    const row = library.find((entry) => entry.session.id === selectedId);
    return new Set(row?.laps.map((lap) => lap.id) ?? []);
  }, [workspace, library, selectedId]);

  return (
    <div className="analyze">
      <ExplorerSidebar
        library={library}
        selectedSessionId={workspace.kind === "session" ? selectedId : null}
        selectedLapId={candidateLapId}
        selectedIds={selectedIds}
        onOpenSession={openSession}
        onActivateLap={activateLap}
        onToggleLap={toggleLap}
        onToggleSession={toggleSession}
        onDeleteSession={handleDelete}
        onDeleteAll={handleDeleteAll}
        onReimportAll={handleReimportAll}
        reimporting={reimporting}
      />
      <div className="analyze-workspace">
        {selection ? (
          selection.lapIds.length === 0 ? (
            <div className="empty-state">
              <h2>No laps selected</h2>
              <p className="muted">Check laps from one track, or open a session.</p>
            </div>
          ) : !selectionReady && selectionError ? (
            <div className="loading">{selectionError}</div>
          ) : !selectionReady ? (
            <div className="loading">Loading laps…</div>
          ) : (
            <CompareWorkspace
              header={
                <div className="session-header">
                  <div>
                    <h2>Selection</h2>
                    <div className="muted">
                      {selection.track} · {selection.lapIds.length} lap
                      {selection.lapIds.length === 1 ? "" : "s"}
                    </div>
                  </div>
                </div>
              }
              laps={selectionLaps}
              track={selection.track}
              trackMap={trackMap}
              candidate={candidate}
              reference={reference}
              candidateLapId={candidateLapId}
              referenceLapId={referenceLapId}
              onSelectCandidate={setCandidateLapId}
              onSelectReference={setReferenceLapId}
              onHoverDistPct={setHighlightPct}
              onFocusDistPct={focusMapAt}
              highlightPct={highlightPct}
              mapFocus={mapFocus}
              paceOnly={paceOnly}
              onPaceOnly={setPaceOnly}
              moreLapColumns={moreLapColumns}
              onMoreLapColumns={() => setMoreLapColumns((v) => !v)}
              showInsights={false}
              showFuel={false}
              sameSubsession={false}
              deltaHeading="Δ Set"
              groupLabel={setGroupLabel(selectionRows)}
            />
          )
        ) : selectedId == null ? (
          <EmptyState config={config} importActions={importActions} />
        ) : loadingDetail ? (
          <div className="loading">Loading session…</div>
        ) : !detail ? (
          <div className="loading">Session not found.</div>
        ) : (
          <CompareWorkspace
            header={
              <SessionHeader
                session={detail.session}
                stats={stats}
                sessionTypes={sessionTypes}
                onReimport={() => handleReimport(detail.session.id)}
                reimporting={reimporting}
              />
            }
            notice={
              !hasEligible && laps.length > 0 && !okChannelPresent ? <ConfigBanner /> : null
            }
            laps={laps}
            track={detail.session.track}
            trackMap={trackMap}
            candidate={candidate}
            reference={reference}
            candidateLapId={candidateLapId}
            referenceLapId={referenceLapId}
            onSelectCandidate={setCandidateLapId}
            onSelectReference={setReferenceLapId}
            onHoverDistPct={setHighlightPct}
            onFocusDistPct={focusMapAt}
            highlightPct={highlightPct}
            mapFocus={mapFocus}
            paceOnly={paceOnly}
            onPaceOnly={setPaceOnly}
            moreLapColumns={moreLapColumns}
            onMoreLapColumns={() => setMoreLapColumns((v) => !v)}
            showInsights
            showFuel
            stats={stats}
          />
        )}
      </div>
    </div>
  );
}

function CompareWorkspace({
  header,
  notice,
  laps,
  track,
  trackMap,
  candidate,
  reference,
  candidateLapId,
  referenceLapId,
  onSelectCandidate,
  onSelectReference,
  onHoverDistPct,
  onFocusDistPct,
  highlightPct,
  mapFocus,
  paceOnly,
  onPaceOnly,
  moreLapColumns,
  onMoreLapColumns,
  showInsights,
  showFuel,
  stats,
  sameSubsession = true,
  deltaHeading,
  groupLabel,
}: {
  header: ReactNode;
  notice?: ReactNode;
  laps: LapSummary[];
  track: string;
  trackMap: TrackOutline | null;
  candidate: LapSummary | null;
  reference: LapSummary | null;
  candidateLapId: number | null;
  referenceLapId: number | null;
  onSelectCandidate: (id: number) => void;
  onSelectReference: (id: number) => void;
  onHoverDistPct: (pct: number | null) => void;
  onFocusDistPct: (pct: number) => void;
  highlightPct: number | null;
  mapFocus: { pct: number; seq: number } | null;
  paceOnly: boolean;
  onPaceOnly: (value: boolean) => void;
  moreLapColumns: boolean;
  onMoreLapColumns: () => void;
  showInsights: boolean;
  showFuel: boolean;
  stats?: SessionStats;
  sameSubsession?: boolean;
  deltaHeading?: string;
  groupLabel?: (lap: LapSummary) => {
    key: string;
    label: string;
    date: string;
    car: string;
    sessionType: string;
  };
}) {
  return (
    <>
      <div className="analyze-session-bar panel">{header}</div>
      {showInsights && stats ? <InsightsStrip stats={stats} /> : null}
      {notice}
      <div className="analyze-main">
        <ComparePanel
          laps={laps}
          candidate={candidate}
          reference={reference}
          onChangeReference={onSelectReference}
          onHoverDistPct={onHoverDistPct}
          onFocusDistPct={onFocusDistPct}
          sameSubsession={sameSubsession}
        />
        <div className="analyze-map-cell">
          <TrackMapPanel
            outline={trackMap}
            track={track}
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
                onChange={(e) => onPaceOnly(e.target.checked)}
              />
              Pace only
            </label>
            <button
              type="button"
              className={`btn btn-ghost${moreLapColumns ? " on" : ""}`}
              onClick={onMoreLapColumns}
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
              <span className="swatch ref" /> Right-click (or Shift-click) to set reference
            </span>
          </div>
          {candidateLapId != null && (referenceLapId == null || referenceLapId === candidateLapId) ? (
            <p className="lap-ref-hint">Right-click another lap as reference.</p>
          ) : null}
        </div>
        <div className="panel-body" style={{ padding: 0 }}>
          <LapTable
            laps={laps}
            candidateLapId={candidateLapId}
            referenceLapId={referenceLapId}
            onSelectCandidate={onSelectCandidate}
            onSelectReference={onSelectReference}
            paceOnly={paceOnly}
            showTireTemps={moreLapColumns}
            deltaHeading={deltaHeading}
            groupLabel={groupLabel}
          />
        </div>
      </div>
      {showFuel ? (
        <details className="analyze-secondary panel">
          <summary>Fuel &amp; tires</summary>
          <div className="panel-body">
            <FuelTirePanel laps={laps} />
          </div>
        </details>
      ) : null}
    </>
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
