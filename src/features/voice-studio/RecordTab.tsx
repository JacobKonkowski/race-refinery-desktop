import { useEffect, useMemo, useRef, useState } from "react";
import {
  cancelVoiceCapture,
  confirmDialog,
  deleteVoiceClip,
  finishVoiceTake,
  getVoiceCaptureLevel,
  getVoicePackStatus,
  listInputDevices,
  playVoiceClip,
  startVoiceCapture,
  undoVoiceTake,
} from "../../shared/api";
import { showToast } from "../../shared/toast";
import type {
  AppSettings,
  InputDevice,
  VoicePackStatus,
  VoicePhrase,
  VoiceTakeResult,
} from "../../shared/types";
import {
  adjacentKey,
  categoryLabel,
  FILTERS,
  filterPhrases,
  groupPhrases,
  nextMissingKey,
  type PhraseFilter,
} from "./voiceStudio";

type Capture = "idle" | "starting" | "recording" | "saving" | "monitor";

interface Props {
  pack: VoicePackStatus;
  phrases: VoicePhrase[];
  settings: AppSettings;
  focusKey: string | null;
  onPackChanged: (pack: VoicePackStatus) => void;
  onSettings: (patch: Partial<AppSettings>) => void;
  onClone: () => void;
}

const isTyping = (target: EventTarget | null) =>
  target instanceof HTMLElement &&
  (["INPUT", "SELECT", "TEXTAREA"].includes(target.tagName) || target.isContentEditable);

export function RecordTab({
  pack,
  phrases,
  settings,
  focusKey,
  onPackChanged,
  onSettings,
  onClone,
}: Props) {
  const [filter, setFilter] = useState<PhraseFilter>("all");
  const missing = useMemo(() => new Set(pack.missing), [pack.missing]);
  const list = useMemo(() => filterPhrases(phrases, filter, missing), [phrases, filter, missing]);
  const [selectedKey, setSelectedKey] = useState<string>(
    () => focusKey ?? phrases.find((p) => missing.has(p.key))?.key ?? phrases[0]?.key ?? "",
  );
  const [capture, setCaptureState] = useState<Capture>("idle");
  const [level, setLevel] = useState(0);
  const [clipped, setClipped] = useState(false);
  const [lastTake, setLastTake] = useState<VoiceTakeResult | null>(null);
  const [undoKeys, setUndoKeys] = useState<Set<string>>(new Set());
  const [devices, setDevices] = useState<InputDevice[]>([]);

  const captureRef = useRef<Capture>("idle");
  const releasedRef = useRef(false);
  const meterTimer = useRef<number | null>(null);

  const phrase = phrases.find((p) => p.key === selectedKey) ?? null;
  const recorded = phrase ? !missing.has(phrase.key) : false;
  const mic = settings.audioCoachMicDevice;
  const micMissing = mic !== "" && devices.length > 0 && !devices.some((d) => d.name === mic);
  const undoTarget = undoKeys.has(selectedKey) ? selectedKey : (lastTake?.key ?? null);

  useEffect(() => {
    if (focusKey) setSelectedKey(focusKey);
  }, [focusKey]);

  useEffect(() => {
    listInputDevices()
      .then(setDevices)
      .catch((e) => showToast(`Could not list microphones: ${String(e)}`, "error"));
    return () => {
      stopMeter();
      if (captureRef.current !== "idle") void cancelVoiceCapture();
    };
  }, []);

  const setCapture = (next: Capture) => {
    captureRef.current = next;
    setCaptureState(next);
  };

  const startMeter = () => {
    stopMeter();
    setClipped(false);
    meterTimer.current = window.setInterval(() => {
      getVoiceCaptureLevel()
        .then((peak) => {
          if (peak >= 0.98) setClipped(true);
          setLevel((prev) => Math.max(peak, prev * 0.75));
        })
        .catch(() => undefined);
    }, 60);
  };

  const stopMeter = () => {
    if (meterTimer.current !== null) window.clearInterval(meterTimer.current);
    meterTimer.current = null;
    setLevel(0);
  };

  // Handlers run from window key listeners, so they read the latest values here.
  const latest = useRef({ pack, selectedKey, list, mic });
  latest.current = { pack, selectedKey, list, mic };
  const autoAdvance = useRef(settings.audioStudioAutoAdvance);
  autoAdvance.current = settings.audioStudioAutoAdvance;

  const beginTake = async () => {
    const { pack: p, selectedKey: key, mic: device } = latest.current;
    if (p.readOnly || !key || captureRef.current !== "idle") return;
    releasedRef.current = false;
    setCapture("starting");
    try {
      await startVoiceCapture(device);
    } catch (e) {
      setCapture("idle");
      showToast(`Microphone: ${String(e)}`, "error");
      return;
    }
    setCapture("recording");
    startMeter();
    if (releasedRef.current) await endTake();
  };

  const endTake = async () => {
    if (captureRef.current === "starting") {
      releasedRef.current = true;
      return;
    }
    if (captureRef.current !== "recording") return;
    const { pack: p, selectedKey: key, list: shown } = latest.current;
    setCapture("saving");
    stopMeter();
    try {
      const take = await finishVoiceTake(p.id, key);
      setLastTake(take);
      setUndoKeys((keys) => new Set(keys).add(key));
      const status = await getVoicePackStatus(p.id);
      onPackChanged(status);
      if (autoAdvance.current && take.warnings.length === 0) {
        const next = nextMissingKey(shown, new Set(status.missing), key);
        if (next) setSelectedKey(next);
      }
    } catch (e) {
      showToast(String(e), "error");
    } finally {
      setCapture("idle");
    }
  };

  const toggleMonitor = async () => {
    if (captureRef.current === "monitor") {
      stopMeter();
      await cancelVoiceCapture();
      setCapture("idle");
      return;
    }
    if (captureRef.current !== "idle") return;
    try {
      await startVoiceCapture(latest.current.mic);
      setCapture("monitor");
      startMeter();
    } catch (e) {
      showToast(`Microphone: ${String(e)}`, "error");
    }
  };

  const play = async () => {
    const { pack: p, selectedKey: key } = latest.current;
    try {
      const report = await playVoiceClip(p.id, key);
      if (report.missing.length > 0) showToast("Not recorded yet", "info");
    } catch (e) {
      showToast(String(e), "error");
    }
  };

  const step = (delta: number) => {
    const { list: shown, selectedKey: key } = latest.current;
    const next = adjacentKey(shown, key, delta);
    if (next) setSelectedKey(next);
  };

  const undo = async () => {
    if (!undoTarget) return;
    try {
      const restored = await undoVoiceTake(pack.id, undoTarget);
      setUndoKeys((keys) => {
        const next = new Set(keys);
        next.delete(undoTarget);
        return next;
      });
      if (lastTake?.key === undoTarget) setLastTake(null);
      onPackChanged(await getVoicePackStatus(pack.id));
      showToast(restored ? `Restored the previous ${undoTarget} take` : "Nothing to undo", "info");
    } catch (e) {
      showToast(String(e), "error");
    }
  };

  const remove = async () => {
    if (!phrase || !(await confirmDialog(`Delete the recording for “${phrase.prompt}”?`))) return;
    try {
      await deleteVoiceClip(pack.id, phrase.key);
      onPackChanged(await getVoicePackStatus(pack.id));
    } catch (e) {
      showToast(String(e), "error");
    }
  };

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (isTyping(e.target) || e.ctrlKey || e.altKey || e.metaKey) return;
      if (e.code === "Space") {
        e.preventDefault();
        if (!e.repeat) void beginTake();
      } else if (e.code === "ArrowDown" || e.code === "ArrowRight") {
        e.preventDefault();
        step(1);
      } else if (e.code === "ArrowUp" || e.code === "ArrowLeft") {
        e.preventDefault();
        step(-1);
      } else if (e.code === "KeyP" && !e.repeat) {
        void play();
      }
    };
    const onKeyUp = (e: KeyboardEvent) => {
      if (e.code !== "Space" || isTyping(e.target)) return;
      e.preventDefault();
      void endTake();
    };
    window.addEventListener("keydown", onKeyDown, true);
    window.addEventListener("keyup", onKeyUp, true);
    return () => {
      window.removeEventListener("keydown", onKeyDown, true);
      window.removeEventListener("keyup", onKeyUp, true);
    };
    // Bound once (handlers read refs) so a re-render never drops a key-up mid-take.
  }, []);

  const groups = groupPhrases(list);
  const busy = capture === "starting" || capture === "saving";
  const recording = capture === "recording" || capture === "starting";

  return (
    <div className="studio-record">
      <aside className="panel studio-checklist">
        <div className="panel-header">
          <div className="btn-row">
            {FILTERS.map((f) => (
              <button
                key={f.id}
                type="button"
                className={`tab${filter === f.id ? " active" : ""}`}
                onClick={() => setFilter(f.id)}
              >
                {f.label}
              </button>
            ))}
          </div>
        </div>
        <div className="studio-checklist-scroll">
          {groups.length === 0 ? (
            <p className="muted small studio-empty">Nothing missing here. Nice work.</p>
          ) : null}
          {groups.map((g) => (
            <div key={g.id}>
              <div className="studio-group">
                {categoryLabel(g.category)} <span className="muted">· {g.tier}</span>
              </div>
              {g.phrases.map((p) => (
                <button
                  key={p.key}
                  type="button"
                  className={`studio-item${p.key === selectedKey ? " active" : ""}`}
                  onClick={() => setSelectedKey(p.key)}
                >
                  <span className={`studio-dot${missing.has(p.key) ? "" : " done"}`} />
                  <span className="studio-item-prompt">{p.prompt}</span>
                </button>
              ))}
            </div>
          ))}
        </div>
      </aside>

      <section className="panel studio-prompt">
        <div className="panel-body">
          {pack.readOnly ? (
            <div className="studio-readonly">
              <span>The bundled pack is read-only. Clone it to record your own voice.</span>
              <button type="button" className="btn btn-primary" onClick={onClone}>
                Clone pack
              </button>
            </div>
          ) : null}

          {phrase ? (
            <>
              <div className="studio-prompt-meta muted small">
                <span className={`tier-badge${phrase.tier === "spotter" ? " spotter" : ""}`}>
                  {phrase.tier === "spotter" ? "Spotter" : "Engineer"}
                </span>
                <span>{categoryLabel(phrase.category)}</span>
                <code>{phrase.key}.wav</code>
                <span className={recorded ? "fast" : ""}>{recorded ? "Recorded" : "Missing"}</span>
              </div>
              <p className={`studio-prompt-text${recording ? " live" : ""}`}>{phrase.prompt}</p>
            </>
          ) : (
            <p className="muted">Pick a phrase from the list.</p>
          )}

          <div className={`studio-meter${clipped ? " clipped" : ""}`} aria-label="Input level">
            <span style={{ width: `${Math.min(100, Math.round(Math.sqrt(level) * 100))}%` }} />
          </div>

          <div className="btn-row studio-controls">
            <button
              type="button"
              className={`btn studio-record-btn${recording ? " live" : ""}`}
              disabled={pack.readOnly || !phrase || busy || capture === "monitor"}
              onPointerDown={(e) => {
                e.currentTarget.setPointerCapture(e.pointerId);
                void beginTake();
              }}
              onPointerUp={() => void endTake()}
              onPointerCancel={() => void endTake()}
            >
              {capture === "saving" ? "Saving…" : recording ? "Recording… release to save" : "Hold to record"}
            </button>
            <button type="button" className="btn" disabled={!recorded} onClick={() => void play()}>
              Play
            </button>
            <button
              type="button"
              className="btn"
              disabled={!undoTarget || pack.readOnly || busy}
              onClick={() => void undo()}
            >
              {undoTarget && undoTarget !== selectedKey ? `Undo ${undoTarget}` : "Undo last take"}
            </button>
            <button
              type="button"
              className="btn btn-danger"
              disabled={!recorded || pack.readOnly || busy}
              onClick={() => void remove()}
            >
              Delete
            </button>
            <span className="studio-spacer" />
            <button type="button" className="btn btn-ghost" onClick={() => step(-1)}>
              ◀ Prev
            </button>
            <button type="button" className="btn btn-ghost" onClick={() => step(1)}>
              Next ▶
            </button>
          </div>
          <p className="muted small">
            Hold <kbd>Space</kbd> (or the record button) while you speak and release to save. Each
            take is trimmed, noise-gated, and leveled automatically. Arrow keys move between
            phrases and <kbd>P</kbd> plays the current clip.
          </p>

          {lastTake ? (
            <div className="studio-take small">
              <span>
                Saved <code>{lastTake.key}</code> · {(lastTake.durationMs / 1000).toFixed(2)} s · peak{" "}
                {Math.round(lastTake.inputPeak * 100)}%
              </span>
              {lastTake.warnings.map((w) => (
                <span key={w} className="studio-warning">
                  {w}
                </span>
              ))}
            </div>
          ) : null}

          <div className="settings-section">
            <div className="settings-field">
              <span className="muted small settings-inline-label">Microphone</span>
              <select
                aria-label="Microphone"
                value={mic}
                disabled={capture !== "idle" && capture !== "monitor"}
                onChange={(e) => onSettings({ audioCoachMicDevice: e.target.value })}
              >
                <option value="">System default</option>
                {micMissing ? <option value={mic}>{mic} (not connected)</option> : null}
                {devices.map((d) => (
                  <option key={d.name} value={d.name}>
                    {d.name}
                    {d.isDefault ? " (default)" : ""}
                  </option>
                ))}
              </select>
              <button
                type="button"
                className={`btn${capture === "monitor" ? " btn-primary" : ""}`}
                disabled={capture !== "idle" && capture !== "monitor"}
                onClick={() => void toggleMonitor()}
              >
                {capture === "monitor" ? "Stop check" : "Check level"}
              </button>
            </div>
            <label className="toggle-row">
              <input
                type="checkbox"
                checked={settings.audioStudioAutoAdvance}
                onChange={(e) => onSettings({ audioStudioAutoAdvance: e.target.checked })}
              />
              <span>Jump to the next missing phrase after a clean take</span>
            </label>
          </div>
        </div>
      </section>
    </div>
  );
}
