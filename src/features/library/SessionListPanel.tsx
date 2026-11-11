import { useMemo, useState } from "react";
import type { LibrarySession } from "../../shared/types";
import { formatDate, formatLapTime } from "../../shared/format";

interface Props {
  library: LibrarySession[];
  focusedSessionId: number | null;
  selectedIds: ReadonlySet<number>;
  onFocusSession: (id: number) => void;
  onDeleteSession: (id: number) => void;
  onDeleteAll: () => void;
  onReimportAll: () => void;
  reimporting: boolean;
}

function typeLetter(sessionType: string): string {
  return sessionType ? sessionType.charAt(0).toUpperCase() : "";
}

export function SessionListPanel({
  library,
  focusedSessionId,
  selectedIds,
  onFocusSession,
  onDeleteSession,
  onDeleteAll,
  onReimportAll,
  reimporting,
}: Props) {
  const [query, setQuery] = useState("");
  const [hideEmpty, setHideEmpty] = useState(true);

  const sessions = useMemo(() => {
    const q = query.trim().toLowerCase();
    return library
      .filter((row) => {
        const session = row.session;
        if (hideEmpty && session.bestLapMs == null) return false;
        if (!q) return true;
        const blob = `${session.track} ${session.car} ${session.sessionDate} ${session.sessionType}`.toLowerCase();
        if (blob.includes(q)) return true;
        return row.laps.some(
          (lap) =>
            String(lap.lapNumber).includes(q) ||
            formatLapTime(lap.lapTimeMs).toLowerCase().includes(q),
        );
      })
      .sort((a, b) => b.session.sessionDate.localeCompare(a.session.sessionDate));
  }, [library, query, hideEmpty]);

  return (
    <aside className="library-master" aria-label="Sessions">
      <div className="sidebar-search">
        <input
          type="search"
          placeholder="Search track, car, date, lap…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
        <div className="sidebar-search-controls">
          <label className="sidebar-hide-empty">
            <input
              type="checkbox"
              checked={hideEmpty}
              onChange={(e) => setHideEmpty(e.target.checked)}
            />
            Hide no-pace-eligible sessions
          </label>
        </div>
      </div>
      <div className="session-list">
        {sessions.length === 0 ? (
          <div className="muted explorer-empty">
            {library.length === 0 ? "No sessions imported." : "No matches."}
          </div>
        ) : (
          sessions.map((row) => {
            const session = row.session;
            const outingIds = row.laps.map((lap) => lap.id);
            const selectedCount = outingIds.filter((id) => selectedIds.has(id)).length;
            return (
              <div
                key={session.id}
                className={`session-item${focusedSessionId === session.id ? " active" : ""}`}
                onClick={() => onFocusSession(session.id)}
                role="button"
                tabIndex={0}
                onKeyDown={(e) => {
                  if (e.key === "Enter" || e.key === " ") {
                    e.preventDefault();
                    onFocusSession(session.id);
                  }
                }}
              >
                <div className="si-track">
                  <span className="si-track-name">{session.track || "Unknown track"}</span>
                  {session.sessionType ? (
                    <span
                      className={`si-type si-type-${typeLetter(session.sessionType)}`}
                      title={session.sessionType}
                    >
                      {typeLetter(session.sessionType)}
                    </span>
                  ) : null}
                </div>
                <div className="si-car">{session.car || "Unknown car"}</div>
                <div className="si-meta">
                  <span>
                    {formatDate(session.sessionDate)}
                    {selectedCount > 0
                      ? ` · ${selectedCount}/${outingIds.length} in bag`
                      : null}
                  </span>
                  <span className="si-best">{formatLapTime(session.bestLapMs)}</span>
                </div>
                <button
                  type="button"
                  className="btn btn-ghost btn-danger si-delete"
                  title="Delete session"
                  onClick={(e) => {
                    e.stopPropagation();
                    onDeleteSession(session.id);
                  }}
                >
                  Delete
                </button>
              </div>
            );
          })
        )}
      </div>
      {library.length > 0 ? (
        <div className="sidebar-footer">
          <button
            type="button"
            className="btn btn-ghost"
            onClick={onReimportAll}
            disabled={reimporting}
            title="Re-analyze every session whose IBT is still on disk"
          >
            {reimporting ? "Re-importing…" : "Re-import all"}
          </button>
          <button
            type="button"
            className="btn btn-ghost btn-danger sidebar-delete-all"
            onClick={onDeleteAll}
          >
            Delete all
          </button>
        </div>
      ) : null}
    </aside>
  );
}
