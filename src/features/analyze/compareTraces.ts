/** Trace toggle keys and defaults for ComparePanel charts. */

export type TraceToggleKey =
  | "delta"
  | "speed"
  | "throttle"
  | "brake"
  | "gear"
  | "steering"
  | "clutch"
  | "rpm"
  | "latG"
  | "longG"
  | "yaw";

export type TraceToggleGroup = "timing" | "pedals" | "driver" | "dynamics";

export interface TraceToggleDef {
  key: TraceToggleKey;
  label: string;
  group: TraceToggleGroup;
  defaultOn: boolean;
}

export const TRACE_TOGGLES: TraceToggleDef[] = [
  { key: "delta", label: "Δ", group: "timing", defaultOn: true },
  { key: "speed", label: "Speed", group: "timing", defaultOn: true },
  { key: "throttle", label: "Throttle", group: "pedals", defaultOn: true },
  { key: "brake", label: "Brake", group: "pedals", defaultOn: true },
  { key: "gear", label: "Gear", group: "driver", defaultOn: true },
  { key: "steering", label: "Steer", group: "driver", defaultOn: false },
  { key: "clutch", label: "Clutch", group: "driver", defaultOn: false },
  { key: "rpm", label: "RPM", group: "driver", defaultOn: true },
  { key: "latG", label: "Lat G", group: "dynamics", defaultOn: true },
  { key: "longG", label: "Long G", group: "dynamics", defaultOn: true },
  { key: "yaw", label: "Yaw", group: "dynamics", defaultOn: false },
];

export function defaultTraceVisibility(): Record<TraceToggleKey, boolean> {
  return Object.fromEntries(TRACE_TOGGLES.map((t) => [t.key, t.defaultOn])) as Record<
    TraceToggleKey,
    boolean
  >;
}
