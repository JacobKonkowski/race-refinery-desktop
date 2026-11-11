import { useEffect, useMemo, useState } from "react";
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

interface VisibleSession {
  source: LibrarySession;
}

interface TrackNode {
  track: string;
  sessions: VisibleSession[];
}

interface CarNode {
  car: string;
  tracks: TrackNode[];
}

function typeLetter(sessionType: string): string {
  return sessionType ? sessionType.charAt(0).toUpperCase() : "";
}

function Twist({ open }: { open: boolean }) {
  return <span className={`explorer-twist-mark${open ? " open" : ""}`}>▸</span>;
}

export function LibraryTreePanel({
  library,
  focusedSessionId,
  onFocusSession,
  onDeleteSession,
  onDeleteAll,
  onReimportAll,
  reimporting,
}: Props) {
  const [query, setQuery] = useState("");
  const [hideEmpty, setHideEmpty] = useState(true);
  const [expanded, setExpanded] = useState<Set<string>>(() => new Set());

  const tree = useMemo(() => {
    const q = query.trim().toLowerCase();
    const cars = new Map<string, Map<string, VisibleSession[]>>();
    for (const row of library) {
      const session = row.session;
      if (hideEmpty && session.bestLapMs == null) continue;
      const blob = `${session.track} ${session.car} ${session.sessionDate} ${session.sessionType}`.toLowerCase();
      const matches =
        !q ||
        blob.includes(q) ||
        row.laps.some(
          (lap) =>
            String(lap.lapNumber).includes(q) ||
            formatLapTime(lap.lapTimeMs).toLowerCase().includes(q),
        );
      if (!matches) continue;
      const car = session.car || "Unknown car";
      const track = session.track || "Unknown track";
      let tracks = cars.get(car);
      if (!tracks) {
        tracks = new Map();
        cars.set(car, tracks);
      }
      const list = tracks.get(track) ?? [];
      list.push({ source: row });
      list.sort((a, b) => b.source.session.sessionDate.localeCompare(a.source.session.sessionDate));
      tracks.set(track, list);
    }
    const nodes: CarNode[] = [...cars.entries()]
      .sort(([a], [b]) => a.localeCompare(b))
      .map(([car, tracks]) => ({
        car,
        tracks: [...tracks.entries()]
          .sort(([a], [b]) => a.localeCompare(b))
          .map(([track, sessions]) => ({ track, sessions })),
      }));
    return nodes;
  }, [library, query, hideEmpty]);

  useEffect(() => {
    if (focusedSessionId == null) return;
    const row = library.find((item) => item.session.id === focusedSessionId);
    if (!row) return;
    const car = row.session.car || "Unknown car";
    const track = row.session.track || "Unknown track";
    const carKey = `c:${car}`;
    const trackKey = `t:${car}|${track}`;
    setExpanded((prev) => {
      if (prev.has(carKey) && prev.has(trackKey)) return prev;
      const next = new Set(prev);
      next.add(carKey);
      next.add(trackKey);
      return next;
    });
  }, [focusedSessionId, library]);

  const forceOpen = query.trim().length > 0;

  function toggle(key: string) {
    setExpanded((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  }

  function isOpen(key: string): boolean {
    return forceOpen || expanded.has(key);
  }

  return (
    <aside className="library-master" aria-label="Cars, tracks, and sessions">
      <div className="sidebar-search">
        <input
          type="search"
          placeholder="Search car, track, date, lap…"
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
      <div className="explorer-scroll">
        {tree.length === 0 ? (
          <div className="muted explorer-empty">
            {library.length === 0 ? "No sessions imported." : "No matches."}
          </div>
        ) : (
          <div className="explorer-tree">
            {tree.map((car) => {
              const carKey = `c:${car.car}`;
              const carOpen = isOpen(carKey);
              return (
                <div key={carKey}>
                  <button type="button" className="explorer-row" onClick={() => toggle(carKey)}>
                    <Twist open={carOpen} />
                    <span className="explorer-name">{car.car}</span>
                  </button>
                  {carOpen
                    ? car.tracks.map((track) => {
                        const trackKey = `t:${car.car}|${track.track}`;
                        const trackOpen = isOpen(trackKey);
                        return (
                          <div key={trackKey} className="explorer-children">
                            <button
                              type="button"
                              className="explorer-row"
                              onClick={() => toggle(trackKey)}
                            >
                              <Twist open={trackOpen} />
                              <span className="explorer-name">{track.track}</span>
                            </button>
                            {trackOpen
                              ? track.sessions.map((visible) => {
                                  const session = visible.source.session;
                                  const active = focusedSessionId === session.id;
                                  return (
                                    <div key={session.id} className="explorer-children">
                                      <div
                                        className={`explorer-row explorer-session${active ? " active" : ""}`}
                                      >
                                        <button
                                          type="button"
                                          className="explorer-label"
                                          onClick={() => onFocusSession(session.id)}
                                        >
                                          <span className="explorer-name">
                                            {formatDate(session.sessionDate)}
                                          </span>
                                          {session.sessionType ? (
                                            <span
                                              className={`si-type si-type-${typeLetter(session.sessionType)}`}
                                              title={session.sessionType}
                                            >
                                              {typeLetter(session.sessionType)}
                                            </span>
                                          ) : null}
                                          <span className="explorer-meta">
                                            {formatLapTime(session.bestLapMs)}
                                          </span>
                                        </button>
                                        <button
                                          type="button"
                                          className="btn btn-ghost btn-danger explorer-delete"
                                          title="Delete session"
                                          onClick={() => onDeleteSession(session.id)}
                                        >
                                          ×
                                        </button>
                                      </div>
                                    </div>
                                  );
                                })
                              : null}
                          </div>
                        );
                      })
                    : null}
                </div>
              );
            })}
          </div>
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
