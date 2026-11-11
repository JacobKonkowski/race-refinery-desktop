import type { LibrarySession } from "../../shared/types";
import { formatDate, formatLapTime } from "../../shared/format";

interface Props {
  session: LibrarySession | null;
  selectedIds: ReadonlySet<number>;
  onToggleLap: (lapId: number) => void;
  onToggleAll: (lapIds: number[]) => void;
  onOpenSession: (sessionId: number) => void;
  onOpenLap: (sessionId: number, lapId: number) => void;
}

export function SessionLapChecklist({
  session,
  selectedIds,
  onToggleLap,
  onToggleAll,
  onOpenSession,
  onOpenLap,
}: Props) {
  if (session == null) {
    return (
      <section className="library-detail" aria-label="Laps">
        <div className="library-detail-empty muted">
          Select a session to check individual laps into the draft bag.
        </div>
      </section>
    );
  }

  const outing = session.session;
  const outingIds = session.laps.map((lap) => lap.id);
  const selectedCount = outingIds.filter((id) => selectedIds.has(id)).length;
  const allChecked = outingIds.length > 0 && selectedCount === outingIds.length;
  const someChecked = selectedCount > 0 && selectedCount < outingIds.length;

  return (
    <section className="library-detail" aria-label="Laps">
      <div className="library-detail-header">
        <div className="library-detail-title">
          <h2>{outing.track || "Unknown track"}</h2>
          <p className="muted">
            {outing.car || "Unknown car"} · {formatDate(outing.sessionDate)}
            {outing.sessionType ? ` · ${outing.sessionType}` : ""}
          </p>
        </div>
        <div className="library-detail-actions">
          <label className="library-select-all">
            <input
              type="checkbox"
              checked={allChecked}
              ref={(node) => {
                if (node) node.indeterminate = someChecked;
              }}
              aria-label="Select all laps in this session"
              onChange={() => onToggleAll(outingIds)}
            />
            All laps
          </label>
          <button
            type="button"
            className="btn"
            onClick={() => onOpenSession(outing.id)}
            title="Open this outing in Analyze"
          >
            Open
          </button>
        </div>
      </div>
      {session.laps.length === 0 ? (
        <p className="muted library-detail-empty">No laps in this session.</p>
      ) : (
        <ul className="library-lap-list">
          {session.laps.map((lap) => {
            const checked = selectedIds.has(lap.id);
            return (
              <li key={lap.id} className={`library-lap-row${checked ? " checked" : ""}`}>
                <input
                  type="checkbox"
                  className="library-lap-check"
                  checked={checked}
                  aria-label={`Select lap ${lap.lapNumber}`}
                  onChange={() => onToggleLap(lap.id)}
                />
                <button
                  type="button"
                  className="library-lap-label"
                  onClick={() => onOpenLap(outing.id, lap.id)}
                  title="Open this lap in Analyze"
                >
                  <span className="library-lap-num">L{lap.lapNumber}</span>
                  <span className="library-lap-time">{formatLapTime(lap.lapTimeMs)}</span>
                  {lap.paceEligible ? (
                    <span className="pace-mark" title="Pace-eligible" />
                  ) : (
                    <span className="pace-mark off" title="Not pace-eligible" />
                  )}
                </button>
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}
