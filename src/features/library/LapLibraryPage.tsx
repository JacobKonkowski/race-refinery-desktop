import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  confirmDialog,
  deleteSession,
  listLibrary,
  onImportComplete,
  reimportSession,
} from "../../shared/api";
import { openSessionInAnalyze } from "../../shared/analyzeOpen";
import { navigateToFeature } from "../../shared/navigation";
import { showToast } from "../../shared/toast";
import type { LibrarySession } from "../../shared/types";
import { useImportActions } from "../analyze/useImportActions";
import { SessionLapList } from "./SessionLapList";
import { SessionTablePanel } from "./SessionTablePanel";

export function LapLibraryPage() {
  const [library, setLibrary] = useState<LibrarySession[]>([]);
  const [focusedSessionId, setFocusedSessionId] = useState<number | null>(null);
  const [reimporting, setReimporting] = useState(false);
  const bulkReimport = useRef(false);
  const importActions = useImportActions();

  const sessions = useMemo(() => library.map((row) => row.session), [library]);

  const refreshLibrary = useCallback(async (): Promise<LibrarySession[]> => {
    const nextLibrary = await listLibrary();
    setLibrary(nextLibrary);
    return nextLibrary;
  }, []);

  useEffect(() => {
    refreshLibrary().catch((e) => {
      console.error("listLibrary failed", e);
      showToast(`Failed to list sessions: ${String(e)}`, "error");
    });
  }, [refreshLibrary]);

  useEffect(() => {
    const unlisten = onImportComplete(async () => {
      if (bulkReimport.current) return;
      await refreshLibrary();
    });
    return () => {
      unlisten.then((fn) => fn()).catch(() => undefined);
    };
  }, [refreshLibrary]);

  useEffect(() => {
    if (library.length === 0) {
      setFocusedSessionId(null);
      return;
    }
    if (focusedSessionId != null && library.some((row) => row.session.id === focusedSessionId)) {
      return;
    }
    setFocusedSessionId(library[0]?.session.id ?? null);
  }, [library, focusedSessionId]);

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
        await refreshLibrary();
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
    } catch (e) {
      showToast(`Delete failed: ${String(e)}`, "error");
      await refreshLibrary();
    }
  }, [sessions, refreshLibrary]);

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
    }
    showToast(
      failed.length === 0
        ? `Re-imported ${done} session(s).`
        : `Re-imported ${done}; ${failed.length} skipped (IBT missing or unreadable).`,
      failed.length === 0 ? "success" : "info",
    );
  }, [sessions, refreshLibrary]);

  const focusedSession = useMemo(
    () => library.find((row) => row.session.id === focusedSessionId) ?? null,
    [library, focusedSessionId],
  );

  return (
    <div className="lap-library">
      {library.length === 0 ? (
        <div className="library-workspace library-workspace-empty">
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
        </div>
      ) : (
        <div className="library-master-detail">
          <SessionTablePanel
            library={library}
            focusedSessionId={focusedSessionId}
            onFocusSession={setFocusedSessionId}
            onDeleteSession={handleDelete}
            onDeleteAll={handleDeleteAll}
            onReimportAll={handleReimportAll}
            reimporting={reimporting}
          />
          <div className="library-detail-column">
            <SessionLapList
              session={focusedSession}
              onOpenSession={openSession}
              onOpenLap={activateLap}
            />
          </div>
        </div>
      )}
    </div>
  );
}
