import type { TierCount } from "./types";

interface Props {
  spotter: TierCount;
  engineer: TierCount;
}

function Badge({ label, count, title }: { label: string; count: TierCount; title: string }) {
  const complete = count.total > 0 && count.recorded >= count.total;
  return (
    <span className={`tier-badge${complete ? " complete" : ""}`} title={title}>
      {label} {count.recorded}/{count.total}
    </span>
  );
}

/** Voice pack completeness: Spotter covers flags and traffic, Engineer adds numbers. */
export function TierBadges({ spotter, engineer }: Props) {
  return (
    <span className="tier-badges">
      <Badge
        label="Spotter"
        count={spotter}
        title="Flags, traffic, race clock, and fuel calls: enough for on-track radio"
      />
      <Badge
        label="Engineer"
        count={engineer}
        title="Numbers and glue words for lap times, gaps, deltas, and fuel counts"
      />
    </span>
  );
}
