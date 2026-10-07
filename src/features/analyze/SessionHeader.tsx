import type { SessionSummary } from "../../shared/types";
import { formatDate, formatLapTime } from "../../shared/format";
import type { SessionStats } from "./sessionStats";
import { truncateIbtName } from "./sessionStats";

interface Props {
  session: SessionSummary;
  stats: SessionStats;
  sessionTypes: string[];
  onReimport: () => void;
  reimporting: boolean;
}

export function SessionHeader({ session, stats, sessionTypes, onReimport, reimporting }: Props) {
  const consistency =
    stats.consistencyMs == null
      ? "—"
      : `±${(stats.consistencyMs / 1000).toFixed(3)}s`;

  return (
    <div className="session-header">
      <div>
        <div className="sh-title">{session.track || "Unknown track"}</div>
        <div className="muted">{session.car || "Unknown car"}</div>
        <div className="muted sh-ibt" title={session.ibtPath}>
          {truncateIbtName(session.ibtPath)}
          <button
            className="btn btn-ghost sh-reimport"
            onClick={onReimport}
            disabled={reimporting}
            title="Re-analyze this session's IBT with the latest Race Refinery analysis"
          >
            {reimporting ? "Re-importing…" : "Re-import"}
          </button>
        </div>
      </div>
      <div className="sh-facts">
        {sessionTypes.length > 0 ? (
          <Fact label="Type" value={sessionTypes.join(" → ")} />
        ) : null}
        <Fact label="Date" value={formatDate(session.sessionDate)} />
        <Fact label="Laps" value={String(session.lapCount)} />
        <Fact
          label="Best (pace)"
          value={formatLapTime(session.bestLapMs)}
          accent={session.bestLapMs != null}
        />
        <Fact
          label="Theo. best"
          value={formatLapTime(stats.theoreticalBestMs)}
          accent={stats.theoreticalBestMs != null}
          title="Stitched best pace-eligible sectors (not a driven lap)"
        />
        <Fact label="Consistency" value={consistency} />
      </div>
    </div>
  );
}

function Fact({
  label,
  value,
  accent,
  title,
}: {
  label: string;
  value: string;
  accent?: boolean;
  title?: string;
}) {
  return (
    <div className="fact" title={title}>
      <span className="fact-label">{label}</span>
      <span className={`fact-value${accent ? " accent" : ""}`}>{value}</span>
    </div>
  );
}
