import { useEffect, useRef, useState } from "react";
import {
  controlVoicePreview,
  getVoicePreviewStatus,
  listVoicePresets,
  playSpotterPlaylist,
  playVoiceComposition,
  playVoicePreset,
} from "../../shared/api";
import { showToast } from "../../shared/toast";
import type {
  VoiceComposition,
  VoicePackStatus,
  VoicePhrase,
  VoicePlaylist,
  VoicePlayReport,
  VoicePreset,
  VoicePreviewStatus,
} from "../../shared/types";
import { optionalNumber, parseLapTime } from "./voiceStudio";

interface Props {
  pack: VoicePackStatus;
  phrases: VoicePhrase[];
  onJumpToKey: (key: string) => void;
}

/** Composer inputs as typed; blank fields are left out of the callout. */
interface Fields {
  radioBeep: boolean;
  lap: string;
  lapTime: string;
  deltaS: string;
  position: string;
  gapAheadS: string;
  gapBehindS: string;
  fuelLiters: string;
  fuelLaps: string;
  incidents: string;
  incidentLimit: string;
  number: string;
}

const START_FIELDS: Fields = {
  radioBeep: true,
  lap: "12",
  lapTime: "1:29.452",
  deltaS: "-0.3",
  position: "",
  gapAheadS: "",
  gapBehindS: "",
  fuelLiters: "",
  fuelLaps: "",
  incidents: "",
  incidentLimit: "",
  number: "",
};

const FIELD_INPUTS: { key: Exclude<keyof Fields, "radioBeep">; label: string; placeholder: string }[] = [
  { key: "lap", label: "Lap", placeholder: "12" },
  { key: "lapTime", label: "Lap time", placeholder: "1:29.452" },
  { key: "deltaS", label: "Delta (s)", placeholder: "-0.3" },
  { key: "position", label: "Position", placeholder: "3" },
  { key: "gapAheadS", label: "Gap ahead (s)", placeholder: "1.4" },
  { key: "gapBehindS", label: "Gap behind (s)", placeholder: "2.0" },
  { key: "fuelLiters", label: "Fuel (L)", placeholder: "12" },
  { key: "fuelLaps", label: "Laps of fuel", placeholder: "5.5" },
  { key: "incidents", label: "Incidents", placeholder: "15" },
  { key: "incidentLimit", label: "Incident limit", placeholder: "17" },
  { key: "number", label: "Number", placeholder: "29" },
];

function toComposition(f: Fields): VoiceComposition {
  const int = (s: string) => {
    const n = optionalNumber(s);
    return n === undefined ? undefined : Math.max(0, Math.round(n));
  };
  const delta = optionalNumber(f.deltaS);
  return {
    radioBeep: f.radioBeep,
    lap: int(f.lap),
    lapTimeMs: parseLapTime(f.lapTime) ?? undefined,
    deltaMs: delta === undefined ? undefined : delta * 1000,
    position: int(f.position),
    gapAheadS: optionalNumber(f.gapAheadS),
    gapBehindS: optionalNumber(f.gapBehindS),
    fuelLiters: optionalNumber(f.fuelLiters),
    fuelLaps: optionalNumber(f.fuelLaps),
    incidents: int(f.incidents),
    incidentLimit: int(f.incidentLimit),
    number: int(f.number),
  };
}

export function PreviewTab({ pack, phrases, onJumpToKey }: Props) {
  const [presets, setPresets] = useState<VoicePreset[]>([]);
  const [fields, setFields] = useState<Fields>(START_FIELDS);
  const [report, setReport] = useState<VoicePlayReport | null>(null);
  const [playlist, setPlaylist] = useState<VoicePlaylist | null>(null);
  const [status, setStatus] = useState<VoicePreviewStatus | null>(null);
  const pollTimer = useRef<number | null>(null);

  useEffect(() => {
    listVoicePresets()
      .then(setPresets)
      .catch(() => setPresets([]));
    return () => {
      stopPolling();
      void controlVoicePreview("stop");
    };
  }, []);

  const stopPolling = () => {
    if (pollTimer.current !== null) window.clearInterval(pollTimer.current);
    pollTimer.current = null;
  };

  const startPolling = () => {
    stopPolling();
    pollTimer.current = window.setInterval(() => {
      getVoicePreviewStatus()
        .then((s) => {
          setStatus(s);
          if (!s.playing) stopPolling();
        })
        .catch(() => stopPolling());
    }, 250);
  };

  const show = (r: VoicePlayReport) => {
    setReport(r);
    setPlaylist(null);
    startPolling();
  };

  const playPreset = (id: string) =>
    playVoicePreset(pack.id, id)
      .then(show)
      .catch((e) => showToast(String(e), "error"));

  const playComposed = () => {
    const composition = toComposition(fields);
    playVoiceComposition(pack.id, composition)
      .then((r) => {
        if (!r.text) showToast("Fill in at least one field", "info");
        show(r);
      })
      .catch((e) => showToast(String(e), "error"));
  };

  const startPlaylist = async () => {
    try {
      const list = await playSpotterPlaylist(pack.id);
      setPlaylist(list);
      setReport(null);
      if (list.keys.length === 0) {
        showToast("No Spotter lines recorded yet", "info");
        return;
      }
      startPolling();
    } catch (e) {
      showToast(String(e), "error");
    }
  };

  const control = (action: "pause" | "resume" | "skip" | "stop") =>
    controlVoicePreview(action)
      .then(() => getVoicePreviewStatus().then(setStatus))
      .catch((e) => showToast(String(e), "error"));

  const promptFor = (key: string) => phrases.find((p) => p.key === key)?.prompt ?? key;
  const playlistActive = playlist !== null && status?.playing === true;
  const currentKey = playlistActive ? playlist.keys[status.index] : null;

  return (
    <div className="studio-preview">
      <div className="panel">
        <div className="panel-header">
          <h2>Soundboard</h2>
        </div>
        <div className="panel-body">
          <p className="muted small">
            Hear callouts stitched exactly as the coach builds them on track, including the
            crossfades between clips. Use it to catch joins that sound off, especially two-clip
            numbers like 29.
          </p>
          <div className="btn-row">
            {presets.map((p) => (
              <button key={p.id} type="button" className="btn" onClick={() => void playPreset(p.id)}>
                {p.label}
              </button>
            ))}
          </div>

          <h3 className="settings-subhead studio-subhead">Composer</h3>
          <div className="studio-composer">
            {FIELD_INPUTS.map((f) => (
              <label key={f.key} className="studio-composer-field">
                <span className="muted small">{f.label}</span>
                <input
                  className="studio-input"
                  value={fields[f.key]}
                  placeholder={f.placeholder}
                  onChange={(e) => setFields({ ...fields, [f.key]: e.target.value })}
                />
              </label>
            ))}
          </div>
          <div className="btn-row studio-subhead">
            <label className="toggle-row">
              <input
                type="checkbox"
                checked={fields.radioBeep}
                onChange={(e) => setFields({ ...fields, radioBeep: e.target.checked })}
              />
              <span>Radio beep first</span>
            </label>
            <button type="button" className="btn btn-primary" onClick={playComposed}>
              Play composition
            </button>
            <button type="button" className="btn btn-ghost" onClick={() => setFields(START_FIELDS)}>
              Reset
            </button>
          </div>

          {report ? (
            <div className="studio-report">
              <div>
                <span className="muted small">Says </span>
                <code>{report.text || "(nothing)"}</code>
              </div>
              {report.missing.length > 0 ? (
                <div className="studio-missing">
                  <span className="muted small">Skipped, not recorded:</span>
                  {report.missing.map((k) => (
                    <button key={k} type="button" className="studio-chip" onClick={() => onJumpToKey(k)}>
                      {k}
                    </button>
                  ))}
                </div>
              ) : null}
            </div>
          ) : null}
        </div>
      </div>

      <div className="panel">
        <div className="panel-header">
          <h2>Spotter QC</h2>
        </div>
        <div className="panel-body">
          <p className="muted small">
            Plays every recorded Spotter line in order: a quick check of volume and timing before
            you drive.
          </p>
          <div className="btn-row">
            <button type="button" className="btn btn-primary" onClick={() => void startPlaylist()}>
              {playlistActive ? "Restart" : "Play Spotter lines"}
            </button>
            <button
              type="button"
              className="btn"
              disabled={!playlistActive}
              onClick={() => void control(status?.paused ? "resume" : "pause")}
            >
              {status?.paused ? "Resume" : "Pause"}
            </button>
            <button
              type="button"
              className="btn"
              disabled={!playlistActive}
              onClick={() => void control("skip")}
            >
              Skip
            </button>
            <button
              type="button"
              className="btn"
              disabled={!playlistActive}
              onClick={() => void control("stop")}
            >
              Stop
            </button>
          </div>
          {currentKey && status ? (
            <p className="studio-now-playing">
              <span className="muted small">
                {status.index + 1} / {status.total}
              </span>{" "}
              {promptFor(currentKey)} <code>{currentKey}</code>
            </p>
          ) : null}
          {playlist && playlist.missing.length > 0 ? (
            <div className="studio-missing">
              <span className="muted small">
                Not recorded ({playlist.missing.length}):
              </span>
              {playlist.missing.map((k) => (
                <button key={k} type="button" className="studio-chip" onClick={() => onJumpToKey(k)}>
                  {k}
                </button>
              ))}
            </div>
          ) : null}
        </div>
      </div>
    </div>
  );
}
