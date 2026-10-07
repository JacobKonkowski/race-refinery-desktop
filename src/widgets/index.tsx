import type { LiveSnapshot, TrackOutline, TrailSample, WidgetKind } from "../shared/types";
import { CoachWidget } from "./CoachWidget";
import { RadarWidget } from "./RadarWidget";
import { RelativeWidget } from "./RelativeWidget";
import { StandingsWidget } from "./StandingsWidget";
import { TrackMapWidget } from "./TrackMapWidget";
import "./widgets.css";

export { CoachWidget, StandingsWidget, RelativeWidget, RadarWidget, TrackMapWidget };
export type { TrackMapMode } from "./TrackMapWidget";

interface WidgetProps {
  kind: WidgetKind;
  snap: LiveSnapshot;
  fieldPaceMode: string;
  outline?: TrackOutline | null;
  trail?: TrailSample[];
}

/** Renders the widget for a kind, shared by monitor windows and HUD previews. */
export function Widget({ kind, snap, fieldPaceMode, outline, trail }: WidgetProps) {
  switch (kind) {
    case "standings":
      return <StandingsWidget snap={snap} />;
    case "relative":
      return <RelativeWidget snap={snap} />;
    case "radar":
      return <RadarWidget snap={snap} />;
    case "trackmap":
      return <TrackMapWidget outline={outline ?? null} snap={snap} candidateTraces={trail} />;
    case "coach":
    default:
      return <CoachWidget snap={snap} fieldPaceMode={fieldPaceMode} />;
  }
}
