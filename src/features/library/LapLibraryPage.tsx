import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  confirmDialog,
  deleteLapSet,
  deleteSession,
  getLapSet,
  listLapSets,
  listLibrary,
  onImportComplete,
  reimportSession,
  saveLapSet,
} from "../../shared/api";
import {
  getAnalyzeOpen,
  getLibraryBag,
  openSelectionInAnalyze,
  openSessionInAnalyze,
  setLibraryBag,
  subscribeAnalyzeOpen,
  subscribeLibraryBag,
  type LibraryBag,
} from "../../shared/analyzeOpen";
import { navigateToFeature } from "../../shared/navigation";
import { showToast } from "../../shared/toast";
import type { LapSetSummary, LibrarySession } from "../../shared/types";
import { useImportActions } from "../analyze/useImportActions";
import { ExplorerSidebar } from "./ExplorerSidebar";
import { SetsPanel } from "./SetsPanel";

export function LapLibraryPage() {
  const [library, setLibrary] = useState<LibrarySession[]>([]);
  const [sets, setSets] = useState<LapSetSummary[]>([]);
  const [bag, setBag] = useState<LibraryBag>(() => getLibraryBag());
  const [analyzeOpen, setAnalyzeOpenState] = useState(() => getAnalyzeOpen());
  const [saveName, setSaveName] = useState(() => getLibraryBag().name);
  const [reimporting, setReimporting] = useState(false);
  const bulkReimport = useRef(false);
  const importActions = useImportActions();

  const sessions = useMemo(() => library.map((row) => row.session), [library]);

  const refreshLibrary = useCallback(async (): Promise<LibrarySession[]> => {
    const nextLibrary = await listLibrary();
    setLibrary(nextLibrary);
    return nextLibrary;
  }, []);

  const refreshSets = useCallback(async (): Promise<LapSetSummary[]> => {
    const next = await listLapSets();
    setSets(next);
    return next;
  }, []);

  useEffect(() => subscribeLibraryBag(() => setBag(getLibraryBag())), []);
  useEffect(() => subscribeAnalyzeOpen(() => setAnalyzeOpenState(getAnalyzeOpen())), []);

  useEffect(() => {
    refreshLibrary().catch((e) => {
      console.error("listLibrary failed", e);
      showToast(`Failed to list sessions: ${String(e)}`, "error");
    });
    refreshSets().catch((e) => {
      console.error("listLapSets failed", e);
      showToast(`Failed to list sets: ${String(e)}`, "error");
    });
  }, [refreshLibrary, refreshSets]);

  useEffect(() => {
    const unlisten = onImportComplete(async () => {
      if (bulkReimport.current) return;
      await refreshLibrary();
    });
    return () => {
      unlisten.then((fn) => fn()).catch(() => undefined);
    };
  }, [refreshLibrary]);

  const commitBag = useCallback((next: LibraryBag) => {
    setLibraryBag(next);
    setSaveName(next.name);
  }, []);

  const currentSelection = useCallback((): { ids: number[]; track: string | null } => {
    return {
      ids: bag.lapIds,
      track: bag.lapIds.length > 0 ? bag.track : null,
    };
  }, [bag]);

  const applyIds = useCallback(
    (ids: number[], track: string) => {
      commitBag({
        lapIds: ids,
        track: ids.length === 0 ? "" : track,
        name: bag.track === track ? bag.name : "",
        setId: bag.track === track ? bag.setId : null,
      });
    },
    [bag, commitBag],
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
      applyIds(ids, ids.length === 0 ? "" : snap.track || track);
    },
    [library, currentSelection, applyIds],
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
      applyIds(ids, ids.length === 0 ? "" : snap.track || track);
    },
    [library, currentSelection, applyIds],
  );

  const openSession = useCallback((id: number) => {
    openSessionInAnalyze(id);
    navigateToFeature("analyze");
  }, []);

  const activateLap = useCallback((sessionId: number, lapId: number) => {
    openSessionInAnalyze(sessionId, lapId);
    navigateToFeature("analyze");
  }, []);

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
        const live = new Set(list.flatMap((row) => row.laps.map((lap) => lap.id)));
        const lapIds = bag.lapIds.filter((id) => live.has(id));
        if (lapIds.length !== bag.lapIds.length) {
          commitBag({
            ...bag,
            lapIds,
            track: lapIds.length === 0 ? "" : bag.track,
            setId: lapIds.length === 0 ? null : bag.setId,
          });
        }
      } catch (e) {
        showToast(`Delete failed: ${String(e)}`, "error");
      }
    },
    [refreshLibrary, bag, commitBag],
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
      commitBag({ lapIds: [], track: "", name: "", setId: null });
    } catch (e) {
      showToast(`Delete failed: ${String(e)}`, "error");
      await refreshLibrary();
    }
  }, [sessions, refreshLibrary, commitBag]);

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
    try {
      for (const s of sessions) {
        try {
          await reimportSession(s.id);
          done += 1;
        } catch {
          failed.push(s.track || `session ${s.id}`);
        }
      }
    } finally {
      bulkReimport.current = false;
      setReimporting(false);
      await refreshLibrary();
      commitBag({ lapIds: [], track: "", name: "", setId: null });
    }
    showToast(
      failed.length === 0
        ? `Re-imported ${done} session(s).`
        : `Re-imported ${done}; ${failed.length} skipped (IBT missing or unreadable).`,
      failed.length === 0 ? "success" : "info",
    );
  }, [sessions, refreshLibrary, commitBag]);

  const handleSaveSet = useCallback(async () => {
    const name = saveName.trim();
    if (!name) {
      showToast("Set name is required.", "error");
      return;
    }
    if (bag.lapIds.length === 0 || !bag.track) {
      showToast("Check at least one lap.", "error");
      return;
    }
    try {
      const id = await saveLapSet(bag.setId, name, bag.track, bag.lapIds);
      await refreshSets();
      commitBag({ ...bag, setId: id, name });
      showToast(bag.setId != null ? "Set updated." : "Set saved.", "success");
    } catch (e) {
      showToast(String(e), "error");
    }
  }, [saveName, bag, refreshSets, commitBag]);

  const handleOpenSet = useCallback(
    async (setId: number) => {
      try {
        const detail = await getLapSet(setId);
        if (!detail) {
          showToast("Set not found.", "error");
          await refreshSets();
          return;
        }
        commitBag({
          lapIds: detail.laps.map((row) => row.lap.id),
          track: detail.set.track,
          name: detail.set.name,
          setId: detail.set.id,
        });
      } catch (e) {
        showToast(String(e), "error");
      }
    },
    [commitBag, refreshSets],
  );

  const handleRenameSet = useCallback(
    async (setId: number, name: string) => {
      const target = sets.find((s) => s.id === setId);
      if (!target) return;
      try {
        const detail = await getLapSet(setId);
        if (!detail) {
          showToast("Set not found.", "error");
          await refreshSets();
          return;
        }
        await saveLapSet(
          setId,
          name,
          detail.set.track,
          detail.laps.map((row) => row.lap.id),
        );
        await refreshSets();
        if (bag.setId === setId) {
          commitBag({ ...bag, name });
        }
        showToast("Set renamed.", "success");
      } catch (e) {
        showToast(String(e), "error");
      }
    },
    [sets, refreshSets, bag, commitBag],
  );

  const handleDeleteSet = useCallback(
    async (setId: number) => {
      const ok = await confirmDialog("Delete this set? The laps stay imported.", "Delete set");
      if (!ok) return;
      try {
        await deleteLapSet(setId);
        await refreshSets();
        if (bag.setId === setId) {
          commitBag({ ...bag, setId: null });
        }
      } catch (e) {
        showToast(String(e), "error");
      }
    },
    [refreshSets, bag, commitBag],
  );

  const handleOpenInAnalyze = useCallback(() => {
    if (bag.lapIds.length === 0 || !bag.track) {
      showToast("Check at least one lap.", "error");
      return;
    }
    const whole = library.find((row) => {
      const lapIds = row.laps.map((lap) => lap.id);
      return lapIds.length === bag.lapIds.length && lapIds.every((id) => bag.lapIds.includes(id));
    });
    if (whole && bag.setId == null) {
      openSessionInAnalyze(whole.session.id);
    } else {
      openSelectionInAnalyze(bag.lapIds, bag.track, { name: bag.name, setId: bag.setId });
    }
    navigateToFeature("analyze");
  }, [bag, library]);

  const selectedIds = useMemo(() => new Set(bag.lapIds), [bag.lapIds]);
  const selectedSessionId =
    analyzeOpen.kind === "session" ? analyzeOpen.sessionId : null;
  const selectedLapId =
    analyzeOpen.kind === "session" ? analyzeOpen.candidateLapId : null;

  return (
    <div className="lap-library">
      <ExplorerSidebar
        library={library}
        selectedSessionId={selectedSessionId}
        selectedLapId={selectedLapId}
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
      <div className="library-workspace">
        {library.length === 0 ? (
          <div className="empty-state">
            <h2>No sessions yet</h2>
            <p className="muted">
              Import an iRacing <code>.ibt</code> file, or scan your telemetry folder.
              <br />
              Record telemetry in the sim with <strong>Alt+L</strong>.
            </p>
            <div className="empty-actions">
              <button className="btn" onClick={importActions.handleScan} disabled={importActions.busy}>
                Scan folder
              </button>
              <button
                className="btn btn-primary"
                onClick={importActions.handleImport}
                disabled={importActions.busy}
              >
                Import IBT
              </button>
            </div>
          </div>
        ) : (
          <>
            <section className="library-selection panel">
              <div className="panel-header">
                <h2>Selection</h2>
              </div>
              <div className="library-selection-body">
                <p className="muted">
                  {bag.lapIds.length === 0
                    ? "Check laps from one track to build a compare bag. Click a session or lap to open it in Analyze."
                    : `${bag.track} · ${bag.lapIds.length} lap${bag.lapIds.length === 1 ? "" : "s"}`}
                </p>
                <div className="library-selection-row">
                  <input
                    className="selection-title-input"
                    value={saveName}
                    onChange={(e) => setSaveName(e.target.value)}
                    placeholder="Set name"
                    aria-label="Set name"
                  />
                  <button type="button" className="btn" onClick={() => void handleSaveSet()}>
                    {bag.setId != null ? "Update set" : "Save as set"}
                  </button>
                  <button
                    type="button"
                    className="btn btn-primary"
                    onClick={handleOpenInAnalyze}
                    disabled={bag.lapIds.length === 0}
                  >
                    Open in Analyze
                  </button>
                </div>
              </div>
            </section>
            <SetsPanel
              sets={sets}
              activeSetId={bag.setId}
              onOpen={(id) => void handleOpenSet(id)}
              onRename={(id, name) => void handleRenameSet(id, name)}
              onDelete={(id) => void handleDeleteSet(id)}
            />
          </>
        )}
      </div>
    </div>
  );
}
