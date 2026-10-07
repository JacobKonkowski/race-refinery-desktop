import { useMemo, useState } from "react";
import type { SessionSummary } from "../../shared/types";
import { formatDate, formatLapTime } from "../../shared/format";

interface Props {
  sessions: SessionSummary[];
  selectedId: number | null;
  onSelect: (id: number) => void;
  onDelete: (id: number) => void;
  onDeleteAll: () => void;
  onReimportAll: () => void;
  reimporting: boolean;
}

export function SessionBrowser({
  sessions,
  selectedId,
  onSelect,
  onDelete,
  onDeleteAll,
  onReimportAll,
  reimporting,
}: Props) {
  const [query, setQuery] = useState("");
  const [sortBy, setSortBy] = useState<"date" | "car" | "track">("date");
  const [hideEmpty, setHideEmpty] = useState(true);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    let result = sessions;
    if (q) {
      result = result.filter(
        (s) =>
          s.track.toLowerCase().includes(q) ||
          s.car.toLowerCase().includes(q) ||
          s.sessionDate.toLowerCase().includes(q),
      );
    }
    if (hideEmpty) {
      result = result.filter((s) => s.bestLapMs !== null);
    }
    result = [...result].sort((a, b) => {
      if (sortBy === "car") return a.car.localeCompare(b.car);
      if (sortBy === "track") return a.track.localeCompare(b.track);
      return b.sessionDate.localeCompare(a.sessionDate);
    });
    return result;
  }, [sessions, query, sortBy, hideEmpty]);

  return (
    <aside className="session-sidebar">
      <div className="sidebar-search">
        <input
          type="text"
          placeholder="Search track or car…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
        <div className="sidebar-search-controls">
          <select
            aria-label="Sort sessions by"
            value={sortBy}
            onChange={(e) => setSortBy(e.target.value as "date" | "car" | "track")}
          >
            <option value="date">Date</option>
            <option value="car">Car</option>
            <option value="track">Track</option>
          </select>
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
        {filtered.length === 0 ? (
          <div className="muted" style={{ padding: 16, fontSize: 13 }}>
            {sessions.length === 0 ? "No sessions imported." : "No matches."}
          </div>
        ) : (
          filtered.map((s) => (
            <div
              key={s.id}
              className={`session-item${s.id === selectedId ? " active" : ""}`}
              onClick={() => onSelect(s.id)}
            >
              <div className="si-track">
                <span className="si-track-name">{s.track || "Unknown track"}</span>
                {s.sessionType ? (
                  <span
                    className={`si-type si-type-${s.sessionType.charAt(0).toUpperCase()}`}
                    title={s.sessionType}
                  >
                    {s.sessionType.charAt(0).toUpperCase()}
                  </span>
                ) : null}
              </div>
              <div className="si-car">{s.car || "Unknown car"}</div>
              <div className="si-meta">
                <span>{formatDate(s.sessionDate)}</span>
                <span className="si-best">{formatLapTime(s.bestLapMs)}</span>
              </div>
              <button
                className="btn btn-ghost btn-danger si-delete"
                title="Delete session"
                onClick={(e) => {
                  e.stopPropagation();
                  onDelete(s.id);
                }}
              >
                Delete
              </button>
            </div>
          ))
        )}
      </div>
      {sessions.length > 0 ? (
        <div className="sidebar-footer">
          <button
            className="btn btn-ghost"
            onClick={onReimportAll}
            disabled={reimporting}
            title="Re-analyze every session whose IBT is still on disk"
          >
            {reimporting ? "Re-importing…" : "Re-import all"}
          </button>
          <button
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
