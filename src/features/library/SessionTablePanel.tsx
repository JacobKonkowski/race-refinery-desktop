import { useMemo, useState } from "react";
import type { LibrarySession } from "../../shared/types";
import { formatDate, formatLapTime } from "../../shared/format";

interface Props {
  library: LibrarySession[];
  focusedSessionId: number | null;
  onFocusSession: (id: number) => void;
  onDeleteSession: (id: number) => void;
  onDeleteAll: () => void;
  onReimportAll: () => void;
  reimporting: boolean;
}

type SortKey = "date" | "track" | "car" | "type" | "laps" | "pace";
type SortDir = "asc" | "desc";

function typeLetter(sessionType: string): string {
  return sessionType ? sessionType.charAt(0).toUpperCase() : "";
}

function uniqueSorted(values: Iterable<string>): string[] {
  return [...new Set([...values].filter(Boolean))].sort((a, b) => a.localeCompare(b));
}

function compareSessions(
  a: LibrarySession,
  b: LibrarySession,
  sortKey: SortKey,
  sortDir: SortDir,
): number {
  const sa = a.session;
  const sb = b.session;
  let cmp = 0;
  switch (sortKey) {
    case "date":
      cmp = sa.sessionDate.localeCompare(sb.sessionDate);
      break;
    case "track":
      cmp = (sa.track || "").localeCompare(sb.track || "");
      break;
    case "car":
      cmp = (sa.car || "").localeCompare(sb.car || "");
      break;
    case "type":
      cmp = (sa.sessionType || "").localeCompare(sb.sessionType || "");
      break;
    case "laps":
      cmp = sa.lapCount - sb.lapCount;
      break;
    case "pace": {
      const pa = sa.bestLapMs ?? Number.POSITIVE_INFINITY;
      const pb = sb.bestLapMs ?? Number.POSITIVE_INFINITY;
      cmp = pa - pb;
      break;
    }
  }
  if (cmp === 0) cmp = sb.sessionDate.localeCompare(sa.sessionDate) || sb.id - sa.id;
  return sortDir === "asc" ? cmp : -cmp;
}

export function SessionTablePanel({
  library,
  focusedSessionId,
  onFocusSession,
  onDeleteSession,
  onDeleteAll,
  onReimportAll,
  reimporting,
}: Props) {
  const [query, setQuery] = useState("");
  const [trackFilter, setTrackFilter] = useState("");
  const [carFilter, setCarFilter] = useState("");
  const [typeFilter, setTypeFilter] = useState("");
  const [hideEmpty, setHideEmpty] = useState(true);
  const [sortKey, setSortKey] = useState<SortKey>("date");
  const [sortDir, setSortDir] = useState<SortDir>("desc");

  const trackOptions = useMemo(
    () => uniqueSorted(library.map((row) => row.session.track)),
    [library],
  );
  const carOptions = useMemo(
    () => uniqueSorted(library.map((row) => row.session.car)),
    [library],
  );
  const typeOptions = useMemo(
    () => uniqueSorted(library.map((row) => row.session.sessionType)),
    [library],
  );

  const sessions = useMemo(() => {
    const q = query.trim().toLowerCase();
    return library
      .filter((row) => {
        const session = row.session;
        if (hideEmpty && session.bestLapMs == null) return false;
        if (trackFilter && session.track !== trackFilter) return false;
        if (carFilter && session.car !== carFilter) return false;
        if (typeFilter && session.sessionType !== typeFilter) return false;
        if (!q) return true;
        const blob =
          `${session.track} ${session.car} ${session.sessionDate} ${session.sessionType} ${session.lapCount}`.toLowerCase();
        if (blob.includes(q)) return true;
        return row.laps.some(
          (lap) =>
            String(lap.lapNumber).includes(q) ||
            formatLapTime(lap.lapTimeMs).toLowerCase().includes(q),
        );
      })
      .sort((a, b) => compareSessions(a, b, sortKey, sortDir));
  }, [library, query, trackFilter, carFilter, typeFilter, hideEmpty, sortKey, sortDir]);

  function toggleSort(key: SortKey) {
    if (sortKey === key) {
      setSortDir((prev) => (prev === "asc" ? "desc" : "asc"));
      return;
    }
    setSortKey(key);
    setSortDir(key === "pace" || key === "date" ? "desc" : "asc");
  }

  function sortMark(key: SortKey): string {
    if (sortKey !== key) return "";
    return sortDir === "asc" ? " ↑" : " ↓";
  }

  return (
    <section className="library-session-table" aria-label="Sessions">
      <div className="library-table-toolbar">
        <input
          type="search"
          placeholder="Filter sessions…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          aria-label="Filter sessions"
        />
        <select
          value={trackFilter}
          onChange={(e) => setTrackFilter(e.target.value)}
          aria-label="Filter by track"
        >
          <option value="">All tracks</option>
          {trackOptions.map((track) => (
            <option key={track} value={track}>
              {track}
            </option>
          ))}
        </select>
        <select
          value={carFilter}
          onChange={(e) => setCarFilter(e.target.value)}
          aria-label="Filter by car"
        >
          <option value="">All cars</option>
          {carOptions.map((car) => (
            <option key={car} value={car}>
              {car}
            </option>
          ))}
        </select>
        <select
          value={typeFilter}
          onChange={(e) => setTypeFilter(e.target.value)}
          aria-label="Filter by type"
        >
          <option value="">All types</option>
          {typeOptions.map((type) => (
            <option key={type} value={type}>
              {type}
            </option>
          ))}
        </select>
        <label className="library-table-hide-empty">
          <input
            type="checkbox"
            checked={hideEmpty}
            onChange={(e) => setHideEmpty(e.target.checked)}
          />
          Hide no-pace-eligible
        </label>
        {library.length > 0 ? (
          <div className="library-table-actions">
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
              className="btn btn-ghost btn-danger"
              onClick={onDeleteAll}
            >
              Delete all
            </button>
          </div>
        ) : null}
      </div>
      <div className="library-table-scroll">
        {sessions.length === 0 ? (
          <div className="muted explorer-empty">
            {library.length === 0 ? "No sessions imported." : "No matches."}
          </div>
        ) : (
          <table className="data-table library-sessions-data">
            <thead>
              <tr>
                <th>
                  <button type="button" className="library-sort" onClick={() => toggleSort("date")}>
                    Date{sortMark("date")}
                  </button>
                </th>
                <th>
                  <button type="button" className="library-sort" onClick={() => toggleSort("track")}>
                    Track{sortMark("track")}
                  </button>
                </th>
                <th>
                  <button type="button" className="library-sort" onClick={() => toggleSort("car")}>
                    Car{sortMark("car")}
                  </button>
                </th>
                <th>
                  <button type="button" className="library-sort" onClick={() => toggleSort("type")}>
                    Type{sortMark("type")}
                  </button>
                </th>
                <th className="num">
                  <button type="button" className="library-sort" onClick={() => toggleSort("laps")}>
                    Laps{sortMark("laps")}
                  </button>
                </th>
                <th className="num">
                  <button type="button" className="library-sort" onClick={() => toggleSort("pace")}>
                    Best pace{sortMark("pace")}
                  </button>
                </th>
                <th className="library-col-actions" aria-label="Actions" />
              </tr>
            </thead>
            <tbody>
              {sessions.map((row) => {
                const session = row.session;
                const selected = focusedSessionId === session.id;
                return (
                  <tr
                    key={session.id}
                    className={selected ? "selected" : undefined}
                    onClick={() => onFocusSession(session.id)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter" || e.key === " ") {
                        e.preventDefault();
                        onFocusSession(session.id);
                      }
                    }}
                    tabIndex={0}
                    aria-selected={selected}
                  >
                    <td>{formatDate(session.sessionDate)}</td>
                    <td>{session.track || "Unknown track"}</td>
                    <td>{session.car || "Unknown car"}</td>
                    <td>
                      {session.sessionType ? (
                        <span
                          className={`si-type si-type-${typeLetter(session.sessionType)}`}
                          title={session.sessionType}
                        >
                          {session.sessionType}
                        </span>
                      ) : (
                        "—"
                      )}
                    </td>
                    <td className="num">{session.lapCount}</td>
                    <td className="num">{formatLapTime(session.bestLapMs)}</td>
                    <td className="library-col-actions">
                      <button
                        type="button"
                        className="btn btn-ghost btn-danger library-row-delete"
                        title="Delete session"
                        onClick={(e) => {
                          e.stopPropagation();
                          onDeleteSession(session.id);
                        }}
                      >
                        ×
                      </button>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        )}
      </div>
    </section>
  );
}
