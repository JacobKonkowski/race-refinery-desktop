/**
 * Shared serde types for the Tauri IPC boundary (`camelCase` JSON).
 * Rust definitions: `src-tauri/src/storage/models.rs`, `analysis/compare.rs`,
 * and (when registered) live / settings / audio / vr modules.
 *
 * Lap facts mirror iRacing exactly — the sim's `_OK` flags, pit-road samples, and
 * distance coverage. There is no invented lap kind or opaque "valid" flag.
 */

export interface SessionSummary {
  id: number;
  ibtPath: string;
  track: string;
  car: string;
  sessionDate: string;
  lapCount: number;
  /** Fastest pace-eligible lap, if any. */
  bestLapMs: number | null;
  importedAt: string;
  /** Session type of the latest sub-session (by session number), e.g. "Race". */
  sessionType: string;
}

export interface SectorTime {
  sectorNum: number;
  timeMs: number;
}

export interface LapSummary {
  id: number;
  sessionNum: number;
  sessionType: string;
  iracingLap: number;
  lapNumber: number;
  lapTimeMs: number | null;
  /** `LapDeltaToBestLap_OK`, or `null` when the channel was absent in the IBT. */
  deltaBestOk: boolean | null;
  deltaSessionBestOk: boolean | null;
  onPitRoadStart: boolean;
  onPitRoadEnd: boolean;
  lapDistPctMin: number | null;
  lapDistPctMax: number | null;
  /** Derived: reported time and both `_OK` flags true. */
  paceEligible: boolean;
  fuelStart: number | null;
  fuelUsed: number | null;
  avgSpeed: number | null;
  lfTemp: number | null;
  rfTemp: number | null;
  lrTemp: number | null;
  rrTemp: number | null;
  /** Mean tire pressures (kPa); null before schema v9 / without channel. */
  lfPressure: number | null;
  rfPressure: number | null;
  lrPressure: number | null;
  rrPressure: number | null;
  /** True when the lap has at least one sparse traffic event. */
  hasTraffic: boolean;
  sectors: SectorTime[];
  /** Delta to the fastest pace-eligible lap in this sub-session. */
  deltaToBestMs: number | null;
}

export interface SessionDetail {
  session: SessionSummary;
  laps: LapSummary[];
}

export interface TracePoint {
  distPct: number;
  speed: number;
  /** Applied pedals (after auto-blip / TC / ABS). */
  throttle: number;
  brake: number;
  /** Driver pedals; `null` for sessions imported before v5 traces or sources without the channel. */
  throttleRaw: number | null;
  brakeRaw: number | null;
  clutch: number | null;
  clutchRaw: number | null;
  handbrakeRaw: number | null;
  /** `BrakeABSactive`; `null` for sessions imported before v6 traces. */
  absActive: boolean | null;
  gear: number;
  steering: number;
  /** GPS at this sample; `null` for sessions imported before v3 traces. */
  lat: number | null;
  lon: number | null;
  /** ms since the lap's first frame; `null` for sessions imported before v4 traces. */
  elapsedMs: number | null;
  /** Engine RPM; `null` before schema v7. */
  rpm: number | null;
  latAccel: number | null;
  longAccel: number | null;
  yawRate: number | null;
}

export interface LapTrace {
  lapId: number;
  lapNumber: number;
  points: TracePoint[];
}

export interface ImportStatus {
  active: boolean;
  currentFile: string | null;
  progressPct: number;
  message: string;
}

export interface IracingConfigCheck {
  appIniPath: string;
  telemetryDir: string;
  memEnabled: boolean;
  diskEnabled: boolean;
  warnings: string[];
}

/* --- Comparison (from `compare_laps`) --- */

export interface SectorDelta {
  sectorNum: number;
  candidateMs: number | null;
  referenceMs: number | null;
  deltaMs: number | null;
}

export interface AlignedPoint {
  distPct: number;
  candidateSpeed: number | null;
  referenceSpeed: number | null;
  candidateThrottle: number | null;
  referenceThrottle: number | null;
  candidateBrake: number | null;
  referenceBrake: number | null;
  candidateGear: number | null;
  referenceGear: number | null;
  candidateSteering: number | null;
  referenceSteering: number | null;
  candidateClutch: number | null;
  referenceClutch: number | null;
  candidateRpm: number | null;
  referenceRpm: number | null;
  candidateLatAccel: number | null;
  referenceLatAccel: number | null;
  candidateLongAccel: number | null;
  referenceLongAccel: number | null;
  candidateYawRate: number | null;
  referenceYawRate: number | null;
  /** Running gap (candidate − reference, ms); `null` where either lap has no time curve. */
  cumulativeDeltaMs: number | null;
}

export interface TrafficEvent {
  distPct: number;
  kind: string;
}

/** "recorded" = both laps carry elapsed time; "estimated" = integrated from speed. */
export type TimingSource = "recorded" | "estimated";

/** Candidate vs reference through one corner. Positive = candidate slower / later. */
export interface CornerDelta {
  /** 1-based in track order; detected from speed, not the official turn numbers. */
  number: number;
  entryPct: number;
  apexPct: number;
  exitPct: number;
  /** Time through this reference-defined window on each lap's timeline. */
  candidateTimeMs: number | null;
  referenceTimeMs: number | null;
  /** Candidate − reference through the corner; + = slower. */
  timeDeltaMs: number;
  entryDeltaMs: number;
  exitDeltaMs: number;
  /** m/s */
  candidateMinSpeed: number | null;
  referenceMinSpeed: number | null;
  /** Positive = candidate braked later. */
  brakePointDeltaM: number | null;
  /** Positive = candidate reached full throttle later. */
  throttlePointDeltaM: number | null;
  candidate: CornerTechnique;
  reference: CornerTechnique;
}

/** How one lap drove one corner (driver pedals, own timeline). */
export interface CornerTechnique {
  /** `null` when the lap has no `BrakeABSactive` (imported before schema v6). */
  absMs: number | null;
  /** 0..1; `null` when the lap didn't brake for the corner. */
  peakBrake: number | null;
  trailBrakeMs: number | null;
  coastMs: number;
  /** `null` when taken flat or full throttle never comes before the exit. */
  apexToThrottleMs: number | null;
}

export type LapRole = "candidate" | "reference";
export type AssistKind = "abs";

/** A stretch of lap distance where ABS intervened. */
export interface AssistSpan {
  lap: LapRole;
  kind: AssistKind;
  startPct: number;
  endPct: number;
}

/** One lap through one corner, relative to the reference lap. */
export interface ConsistencyPoint {
  lapId: number;
  /** Metres after the reference brake point (negative = earlier). */
  brakeOffsetM: number | null;
  timeDeltaMs: number;
  /** m/s */
  minSpeed: number | null;
}

/** Every clean lap through one of the reference lap's corners (`corner_consistency`). */
export interface CornerConsistency {
  number: number;
  apexPct: number;
  brakeSpreadM: number | null;
  points: ConsistencyPoint[];
}

export interface LapComparison {
  candidateLapId: number;
  referenceLapId: number;
  candidateTimeMs: number | null;
  referenceTimeMs: number | null;
  deltaMs: number | null;
  sectorDeltas: SectorDelta[];
  series: AlignedPoint[];
  corners: CornerDelta[];
  /** Where ABS intervened on either lap. */
  assists: AssistSpan[];
  candidateTraffic: TrafficEvent[];
  referenceTraffic: TrafficEvent[];
  timing: TimingSource | null;
  trackLengthM: number | null;
}

/* --- Live telemetry --- */

export type LiveConnectionState =
  | "disconnected"
  | "waitingForSession"
  | "reconnecting"
  | "connected"
  | "error";

export interface LiveStatus {
  state: LiveConnectionState;
  message: string;
}

export interface LiveSectorProgress {
  sectorNum: number;
  timeMs: number | null;
  completed: boolean;
}

export type PackState =
  | "off"
  | "clear"
  | "carLeft"
  | "carRight"
  | "threeWide"
  | "twoCarsLeft"
  | "twoCarsRight";

export interface CompetitorEntry {
  carIdx: number;
  driverName: string;
  carNumber: string;
  classId: number;
  classColor: string;
  position: number;
  classPosition: number;
  bestLapMs: number | null;
  lastLapMs: number | null;
  onPitRoad: boolean;
  isPlayer: boolean;
  lapDistPct: number;
  gapToPlayerS: number | null;
}

/** Live telemetry + field context from `get_live_snapshot` / `live-telemetry` events. */
export interface LiveSnapshot {
  track: string;
  car: string;
  sessionType: string;
  lap: number;
  lapTimeMs: number;
  lastLapMs: number | null;
  lastLapValid: boolean;
  bestLapMs: number | null;
  deltaToBestMs: number | null;
  deltaToLastMs: number | null;
  fuelLevel: number;
  speed: number;
  lapDistPct: number;
  /** Applied pedals (after auto-blip / TC / ABS). */
  throttle: number;
  brake: number;
  /** Driver pedals; `null` when the sim omits the channel. */
  throttleRaw: number | null;
  brakeRaw: number | null;
  clutch: number | null;
  clutchRaw: number | null;
  handbrakeRaw: number | null;
  /** Player GPS when the sim provides it. */
  lat: number | null;
  lon: number | null;
  currentSector: number;
  sectorBoundaries: number[];
  sectors: LiveSectorProgress[];
  lfTemp: number;
  rfTemp: number;
  lrTemp: number;
  rrTemp: number;
  onPitRoad: boolean;
  competitors: CompetitorEntry[];
  playerPosition: number | null;
  playerClassPosition: number | null;
  sessionFastestLapMs: number | null;
  deltaToSessionBestMs: number | null;
  deltaToSessionOptimalMs: number | null;
  gapToCarAheadS: number | null;
  gapToCarBehindS: number | null;
  packState: PackState;
  sessionFlags: number;
  incidentCount: number;
  sessionLapsRemain: number | null;
  sessionTimeRemainS: number | null;
  pitsOpen: boolean;
  onTrack: boolean;
}


/** One outline vertex: lap fraction plus position in a `0 0 1 1` viewBox. */
export interface OutlinePoint {
  pct: number;
  x: number;
  y: number;
}

/** Maps GPS degrees into an outline's unit box; mirrors `TrackProjection`. */
export interface TrackProjection {
  originLat: number;
  originLon: number;
  minX: number;
  minY: number;
  scale: number;
  offsetX: number;
  offsetY: number;
}

/** Circuit outline generated from IBT GPS samples (`get_track_map`). */
export interface TrackOutline {
  track: string;
  points: OutlinePoint[];
  /** Closed SVG path over a `0 0 1 1` viewBox. */
  svgPath: string;
  /** Lap fraction spanned by the source samples. */
  coverage: number;
  /** GPS samples the outline was built from. */
  sampleCount: number;
  /** Absent on outlines cached before racing lines; GPS cannot be placed then. */
  projection?: TrackProjection | null;
}

/** Minimum shape needed to draw a pedal-colored path (a `TracePoint` fits). */
export interface TrailSample {
  distPct: number;
  throttle: number;
  brake: number;
  /** Driver pedals; pedal coloring prefers these over applied when present. */
  throttleRaw?: number | null;
  brakeRaw?: number | null;
  lat: number | null;
  lon: number | null;
}

export type WidgetKind = "coach" | "standings" | "relative" | "radar" | "trackmap";

export const WIDGET_KINDS: WidgetKind[] = [
  "coach",
  "standings",
  "relative",
  "radar",
  "trackmap",
];

export const WIDGET_LABELS: Record<WidgetKind, string> = {
  coach: "Coach HUD",
  standings: "Standings",
  relative: "Relative",
  radar: "Radar",
  trackmap: "Track Map",
};

export interface WidgetPlacement {
  /** Shared enable flag for monitor windows and VR slots. */
  enabled: boolean;
  /** Monitor window screen position / size (pixels). */
  desktopX: number;
  desktopY: number;
  desktopW: number;
  desktopH: number;
  /** VR anchor: fixed in the cockpit ("world") or following the head. */
  vrLock: VrLock;
  /** VR placement (meters / degrees / multipliers) on top of the per-kind base pose. */
  vrOffsetX: number;
  vrOffsetY: number;
  vrOffsetZ: number;
  vrTiltDeg: number;
  vrScale: number;
  vrOpacity: number;
}

export type VrLock = "world" | "head";

/** A DirectInput controller button (wheel, button box, ...). */
export interface ControllerBinding {
  deviceGuid: string;
  deviceName: string;
  button: number;
}

export interface OverlayLayout {
  widgets: WidgetPlacement[];
  fieldPaceMode: string;
}

/** Default layout, mirroring `OverlayLayout::default()` in the Rust settings. */
export function defaultOverlayLayout(): OverlayLayout {
  const base = (over: Partial<WidgetPlacement>): WidgetPlacement => ({
    enabled: false,
    desktopX: 24,
    desktopY: 24,
    desktopW: 320,
    desktopH: 180,
    vrLock: "world",
    vrOffsetX: 0,
    vrOffsetY: 0,
    vrOffsetZ: 0,
    vrTiltDeg: 0,
    vrScale: 0.55,
    vrOpacity: 0.75,
    ...over,
  });
  return {
    widgets: [
      base({ enabled: true, desktopX: 24, desktopY: 24, desktopW: 360, desktopH: 200 }),
      base({ desktopX: 24, desktopY: 244, desktopW: 320, desktopH: 300 }),
      base({ desktopX: 360, desktopY: 244, desktopW: 300, desktopH: 240 }),
      base({ desktopX: 404, desktopY: 24, desktopW: 200, desktopH: 200 }),
      base({ desktopX: 620, desktopY: 24, desktopW: 320, desktopH: 320 }),
    ],
    fieldPaceMode: "best",
  };
}

/** User preferences persisted to `%LOCALAPPDATA%\\race-refinery\\settings.json`. */
export interface AppSettings {
  ollamaUrl?: string;
  ollamaModel?: string;
  overlayX?: number;
  overlayY?: number;
  overlayWidth?: number;
  overlayHeight?: number;
  vrOverlayEnabled: boolean;
  vrOverlayScale: number;
  vrMode: string;
  vrHudOffset: number;
  vrHudOpacity: number;
  vrRecenterHotkey: string;
  /** Optional wheel / button-box button that recenters the VR anchor. */
  vrRecenterButton: ControllerBinding | null;
  vrFieldPaceMode: string;
  overlayLayout: OverlayLayout;
  audioCoachEnabled: boolean;
  /** Active voice pack: `default` (bundled), a user pack id, or an absolute folder path. */
  audioCoachPackId: string;
  /** Folders linked as voice packs. */
  audioCoachPackFolders: string[];
  /** Voice Studio microphone name; empty = system default. */
  audioCoachMicDevice: string;
  /** Voice Studio jumps to the next missing phrase after each saved take. */
  audioStudioAutoAdvance: boolean;
  audioCoachVolume: number;
  audioCoachFuelThreshold: number;
  audioPackAlertsEnabled: boolean;
  audioFlagsEnabled: boolean;
  audioIncidentsEnabled: boolean;
  audioFuelRaceEnabled: boolean;
  audioGapAlertsEnabled: boolean;
  audioPaceEnabled: boolean;
  audioStrategyEnabled: boolean;
  audioRaceClockEnabled: boolean;
  audioPitsOpenEnabled: boolean;
  audioCoachChatterLevel: "minimal" | "normal" | "verbose";
  audioSessionIntroEnabled: boolean;
  audioPositionCalloutsEnabled: boolean;
  audioTyreAlertsEnabled: boolean;
  audioInvalidLapEnabled: boolean;
  audioRadioEffectsEnabled: boolean;
  audioPackPrecursorsEnabled: boolean;
  audioFuelStrategySensitivity: "normal" | "conservative";
  audioInterMessageGapMs: number;
  audioVoiceCommandsEnabled?: boolean;
  audioVoicePushToTalkKey?: string;
}

export interface AudioCoachStatus {
  active: boolean;
  lastMessage: string;
  /** Active voice pack reference and its completeness. */
  packId: string;
  packName: string;
  spotter: TierCount;
  engineer: TierCount;
}

/** Recorded vs total phrases in one completeness tier. */
export interface TierCount {
  recorded: number;
  total: number;
}

export type PhraseTier = "spotter" | "engineer";

/** A clip a voice pack can hold (`list_voice_phrases`). */
export interface VoicePhrase {
  key: string;
  /** What the speaker says into the mic. */
  prompt: string;
  tier: PhraseTier;
  category: string;
}

export interface VoicePackStatus {
  /** Reference stored in `audioCoachPackId`. */
  id: string;
  name: string;
  author: string;
  kind: "bundled" | "user" | "folder";
  readOnly: boolean;
  dir: string;
  spotter: TierCount;
  engineer: TierCount;
  /** Phrase keys without a clip, in registry order. */
  missing: string[];
  /** The pack overrides the built-in radio beep. */
  customBeep: boolean;
}

export interface VoiceImportReport {
  imported: string[];
  overwritten: string[];
  unknown: string[];
  failed: string[];
}

export interface VoicePackImport {
  pack: VoicePackStatus;
  report: VoiceImportReport;
}

export interface InputDevice {
  name: string;
  isDefault: boolean;
}

export interface VoiceTakeResult {
  key: string;
  durationMs: number;
  /** Raw input peak (0-1) before normalization. */
  inputPeak: number;
  warnings: string[];
}

/** What a coach test or soundboard callout played; `missing` clips were skipped. */
export interface VoicePlayReport {
  text: string;
  missing: string[];
}

export interface VoicePreset {
  id: string;
  label: string;
}

/** Soundboard composer input; each set field adds its callout. */
export interface VoiceComposition {
  radioBeep?: boolean;
  lap?: number;
  lapTimeMs?: number;
  sector?: number;
  sectorTimeMs?: number;
  deltaMs?: number;
  position?: number;
  gapAheadS?: number;
  gapBehindS?: number;
  fuelLiters?: number;
  fuelLaps?: number;
  incidents?: number;
  incidentLimit?: number;
  number?: number;
}

export interface VoicePlaylist {
  keys: string[];
  missing: string[];
}

export interface VoicePreviewStatus {
  playing: boolean;
  paused: boolean;
  index: number;
  total: number;
  text: string;
}

export type VoicePreviewControl = "pause" | "resume" | "skip" | "stop";

export interface MonitorOverlayStatus {
  active: boolean;
  message: string;
  windows: string[];
}

export interface VrOverlayStatus {
  active: boolean;
  runtime: string;
  message: string;
  hudUrl: string;
  mode: string;
  layerInstalled: boolean;
}

export interface NativeVrStatus {
  active: boolean;
  layerInstalled: boolean;
  /** Proxy: fresh SHM publish + layer installed (no layer heartbeat file). */
  compositorActive: boolean;
  telemetryPublishing: boolean;
  lastFrameAgeMs: number | null;
  writeAgeMs: number | null;
  overlayCount: number;
  lastError: string | null;
}

export interface VrLayerDiagnostics {
  registered: boolean;
  manifestPath: string | null;
  dllPresent: boolean;
  dllPath: string | null;
  layerDisabled: boolean;
  ready: boolean;
  iracingOpenXrVrMode: number | null;
  iracingOpenXrEnabled: boolean | null;
  issues: string[];
}
