import { useMemo, useState } from "react";
import type { LibraryLap, LibrarySession } from "../../shared/types";
import { formatDate, formatLapTime } from "../../shared/format";

interface Props {
  library: LibrarySession[];
  selectedSessionId: number | null;
  selectedLapId: number | null;
  selectedIds: ReadonlySet<number>;
  onOpenSession: (id: number) => void;
  onActivateLap: (sessionId: number, lapId: number) => void;
  onToggleLap: (lapId: number) => void;
  onToggleSession: (lapIds: number[]) => void;
  onDeleteSession: (id: number) => void;
  onDeleteAll: () => void;
  onReimportAll: () => void;
  reimporting: boolean;
}

interface VisibleSession {
  source: LibrarySession;
  laps: LibraryLap[];
}

interface CarNode {
  car: string;
  sessions: VisibleSession[];
}

interface TrackNode {
  track: string;
  cars: CarNode[];
}

function typeLetter(sessionType: string): string {
  return sessionType ? sessionType.charAt(0).toUpperCase() : "";
}

export function ExplorerSidebar({
  library,
  selectedSessionId,
  selectedLapId,
  selectedIds,
  onOpenSession,
  onActivateLap,
  onToggleLap,
  onToggleSession,
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
    const tracks = new Map<string, Map<string, VisibleSession[]>>();
    for (const row of library) {
      const session = row.session;
      if (hideEmpty && session.bestLapMs == null) continue;
      const blob = `${session.track} ${session.car} ${session.sessionDate} ${session.sessionType}`.toLowerCase();
      const laps = row.laps.filter((lap) => {
        if (!q || blob.includes(q)) return true;
        return (
          String(lap.lapNumber).includes(q) || formatLapTime(lap.lapTimeMs).toLowerCase().includes(q)
        );
      });
      if (laps.length === 0) continue;
      const track = session.track || "Unknown track";
      const car = session.car || "Unknown car";
      let cars = tracks.get(track);
      if (!cars) {
        cars = new Map();
        tracks.set(track, cars);
      }
      const list = cars.get(car) ?? [];
      list.push({ source: row, laps });
      list.sort((a, b) => b.source.session.sessionDate.localeCompare(a.source.session.sessionDate));
      cars.set(car, list);
    }
    const nodes: TrackNode[] = [...tracks.entries()]
      .sort(([a], [b]) => a.localeCompare(b))
      .map(([track, cars]) => ({
        track,
        cars: [...cars.entries()]
          .sort(([a], [b]) => a.localeCompare(b))
          .map(([car, sessions]) => ({ car, sessions })),
      }));
    return nodes;
  }, [library, query, hideEmpty]);

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
    <aside className="session-sidebar">
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
      <div className="explorer-scroll">
        {tree.length === 0 ? (
          <div className="muted explorer-empty">
            {library.length === 0 ? "No sessions imported." : "No matches."}
          </div>
        ) : (
          <div className="explorer-tree">
            {tree.map((track) => {
              const trackKey = `t:${track.track}`;
              const trackOpen = isOpen(trackKey);
              return (
                <div key={trackKey}>
                  <button type="button" className="explorer-row" onClick={() => toggle(trackKey)}>
                    <Twist open={trackOpen} />
                    <span className="explorer-name">{track.track}</span>
                  </button>
                  {trackOpen
                    ? track.cars.map((car) => {
                        const carKey = `c:${track.track}|${car.car}`;
                        const carOpen = isOpen(carKey);
                        return (
                          <div key={carKey} className="explorer-children">
                            <button
                              type="button"
                              className="explorer-row"
                              onClick={() => toggle(carKey)}
                            >
                              <Twist open={carOpen} />
                              <span className="explorer-name">{car.car}</span>
                            </button>
                            {carOpen
                              ? car.sessions.map((visible) => {
                                  const session = visible.source.session;
                                  const sessionKey = `s:${session.id}`;
                                  const sessionOpen = isOpen(sessionKey);
                                  const active = selectedSessionId === session.id;
                                  const outingIds = visible.source.laps.map((lap) => lap.id);
                                  const selectedCount = outingIds.filter((id) =>
                                    selectedIds.has(id),
                                  ).length;
                                  return (
                                    <div key={session.id} className="explorer-children">
                                      <div
                                        className={`explorer-row explorer-session${active ? " active" : ""}`}
                                      >
                                        <MembershipBox
                                          checked={
                                            outingIds.length > 0 && selectedCount === outingIds.length
                                          }
                                          indeterminate={
                                            selectedCount > 0 && selectedCount < outingIds.length
                                          }
                                          label={`Select laps from ${formatDate(session.sessionDate)}`}
                                          onToggle={() => onToggleSession(outingIds)}
                                        />
                                        <button
                                          type="button"
                                          className="explorer-twist"
                                          onClick={() => toggle(sessionKey)}
                                          aria-label={sessionOpen ? "Collapse laps" : "Expand laps"}
                                        >
                                          <Twist open={sessionOpen} />
                                        </button>
                                        <button
                                          type="button"
                                          className="explorer-label"
                                          onClick={() => onOpenSession(session.id)}
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
                                      {sessionOpen
                                        ? visible.laps.map((lap) => {
                                            const checked = selectedIds.has(lap.id);
                                            return (
                                              <div
                                                key={lap.id}
                                                className={`explorer-row explorer-lap${
                                                  checked && selectedLapId === lap.id ? " active" : ""
                                                }`}
                                              >
                                                <MembershipBox
                                                  checked={checked}
                                                  label={`Select lap ${lap.lapNumber}`}
                                                  onToggle={() => onToggleLap(lap.id)}
                                                />
                                                <button
                                                  type="button"
                                                  className="explorer-label"
                                                  onClick={() => onActivateLap(session.id, lap.id)}
                                                >
                                                  <span className="explorer-lap-num">{lap.lapNumber}</span>
                                                  <span className="explorer-meta">
                                                    {formatLapTime(lap.lapTimeMs)}
                                                  </span>
                                                  {lap.paceEligible ? (
                                                    <span className="pace-mark" title="Pace-eligible" />
                                                  ) : (
                                                    <span className="pace-mark off" />
                                                  )}
                                                </button>
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
            className="btn btn-ghost"
            onClick={onReimportAll}
            disabled={reimporting}
            title="Re-analyze every session whose IBT is still on disk"
          >
            {reimporting ? "Re-importing…" : "Re-import all"}
          </button>
          <button className="btn btn-ghost btn-danger sidebar-delete-all" onClick={onDeleteAll}>
            Delete all
          </button>
        </div>
      ) : null}
    </aside>
  );
}

function Twist({ open }: { open: boolean }) {
  return <span className={`explorer-twist-mark${open ? " open" : ""}`}>▸</span>;
}

function MembershipBox({
  checked,
  indeterminate = false,
  label,
  onToggle,
}: {
  checked: boolean;
  indeterminate?: boolean;
  label: string;
  onToggle: () => void;
}) {
  return (
    <input
      type="checkbox"
      className="explorer-check"
      checked={checked}
      ref={(node) => {
        if (node) node.indeterminate = indeterminate && !checked;
      }}
      aria-label={label}
      onClick={(event) => event.stopPropagation()}
      onChange={onToggle}
    />
  );
}
