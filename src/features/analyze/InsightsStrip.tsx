import type { SessionStats } from "./sessionStats";

interface Props {
  stats: SessionStats;
}

/** Light deterministic insights from lap data (no LLM). */
export function InsightsStrip({ stats }: Props) {
  const bullets: string[] = [];

  if (stats.paceLapCount >= 2 && stats.consistencyMs != null) {
    const sec = (stats.consistencyMs / 1000).toFixed(3);
    bullets.push(
      `Lap-time stdev ±${sec}s across ${stats.paceLapCount} pace laps.`,
    );
  }

  if (stats.weakSector != null && stats.weakSectorLossMs != null) {
    bullets.push(
      `Sector S${stats.weakSector} avg +${(stats.weakSectorLossMs / 1000).toFixed(3)}s vs best pace sector in session.`,
    );
  }

  if (stats.fuelOutlierLap != null && stats.fuelOutlierUsed != null) {
    bullets.push(
      `Lap ${stats.fuelOutlierLap} fuel use ${stats.fuelOutlierUsed.toFixed(2)} L (≥12% from median of pace laps).`,
    );
  }

  if (bullets.length === 0) return null;

  return (
    <div className="insights-strip panel">
      <div className="insights-label">Insights</div>
      <ul>
        {bullets.map((b) => (
          <li key={b}>{b}</li>
        ))}
      </ul>
    </div>
  );
}
