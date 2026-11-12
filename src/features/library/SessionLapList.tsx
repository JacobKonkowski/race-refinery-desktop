import type { LibrarySession } from "../../shared/types";
import { formatDate, formatLapTime } from "../../shared/format";

interface Props {
  session: LibrarySession | null;
  onOpenSession: (sessionId: number) => void;
  onOpenLap: (sessionId: number, lapId: number) => void;
}

export function SessionLapList({ session, onOpenSession, onOpenLap }: Props) {
  if (session == null) {
    return (
      <section className="library-detail" aria-label="Laps">
        <div className="library-detail-empty muted">
          Select a session to browse its laps.
        </div>
      </section>
    );
  }

  const outing = session.session;

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
          <button
            type="button"
            className="btn btn-primary"
            onClick={() => onOpenSession(outing.id)}
            title="Open this outing in Analyze"
          >
            Open in Analyze
          </button>
        </div>
      </div>
      {session.laps.length === 0 ? (
        <p className="muted library-detail-empty">No laps in this session.</p>
      ) : (
        <ul className="library-lap-list">
          {session.laps.map((lap) => (
            <li key={lap.id} className="library-lap-row">
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
          ))}
        </ul>
      )}
    </section>
  );
}
