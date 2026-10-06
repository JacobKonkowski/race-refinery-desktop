import { useCallback, useEffect, useRef, useState } from "react";
import {
  captureControllerButton,
  getSettings,
  recenterVr,
  saveSettings,
  testAudioCoach,
} from "../../shared/api";
import { Slider } from "../../shared/Slider";
import { showToast } from "../../shared/toast";
import { defaultOverlayLayout } from "../../shared/types";
import type { AppSettings, WidgetPlacement } from "../../shared/types";
import { coachTestMessage } from "../../shared/voicePacks";
import { VoicePackSection } from "./VoicePackSection";

/** Index = VR overlay slot (see `OverlayLayout::widgets`). */
const WIDGET_NAMES = ["Coach", "Standings", "Relative", "Radar", "Track map"];

type BoolKey = {
  [K in keyof AppSettings]-?: AppSettings[K] extends boolean | undefined ? K : never;
}[keyof AppSettings];

/** Coach callout categories, in the order drivers usually think about them. */
const COACH_CATEGORIES: { key: BoolKey; label: string }[] = [
  { key: "audioPackAlertsEnabled", label: "Spotter (cars alongside)" },
  { key: "audioPackPrecursorsEnabled", label: "Spotter early warnings" },
  { key: "audioFlagsEnabled", label: "Flags" },
  { key: "audioIncidentsEnabled", label: "Incidents" },
  { key: "audioGapAlertsEnabled", label: "Gaps ahead / behind" },
  { key: "audioPositionCalloutsEnabled", label: "Position changes" },
  { key: "audioPaceEnabled", label: "Lap and sector pace" },
  { key: "audioInvalidLapEnabled", label: "Invalid laps" },
  { key: "audioFuelRaceEnabled", label: "Fuel to finish (races)" },
  { key: "audioStrategyEnabled", label: "Fuel strategy / low fuel" },
  { key: "audioTyreAlertsEnabled", label: "Hot tyres" },
  { key: "audioRaceClockEnabled", label: "Laps / time remaining" },
  { key: "audioPitsOpenEnabled", label: "Pits open" },
  { key: "audioSessionIntroEnabled", label: "Session intro" },
];

const MODIFIER_CODES = new Set([
  "ControlLeft",
  "ControlRight",
  "ShiftLeft",
  "ShiftRight",
  "AltLeft",
  "AltRight",
  "MetaLeft",
  "MetaRight",
]);

/** Keys that are safe to bind without a modifier (they rarely type anything). */
const BARE_KEY = /^(F\d{1,2}|Numpad.+|Pause|ScrollLock|Insert)$/;

/**
 * Build a global-shortcut accelerator (e.g. "Ctrl+Shift+R") from a key event.
 * Returns null for modifier-only presses and for bare typing keys.
 */
function acceleratorFromEvent(e: KeyboardEvent): string | null {
  if (MODIFIER_CODES.has(e.code)) return null;
  const key = e.code.replace(/^Key/, "").replace(/^Digit/, "");
  const mods = [
    e.ctrlKey && "Ctrl",
    e.altKey && "Alt",
    e.shiftKey && "Shift",
    e.metaKey && "Super",
  ].filter(Boolean) as string[];
  if (mods.length === 0 && !BARE_KEY.test(key)) return null;
  return [...mods, key].join("+");
}

const meters = (v: number) => `${v >= 0 ? "+" : ""}${v.toFixed(2)} m`;

export function SettingsPage() {
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [widgetIndex, setWidgetIndex] = useState(0);
  const [capturingKey, setCapturingKey] = useState(false);
  const [capturingButton, setCapturingButton] = useState(false);
  const settingsRef = useRef<AppSettings | null>(null);
  const saveTimer = useRef<number | null>(null);

  useEffect(() => {
    getSettings()
      .then((s) => {
        settingsRef.current = s;
        setSettings(s);
      })
      .catch((e) => showToast(`Could not load settings: ${String(e)}`, "error"));
    return () => {
      if (saveTimer.current !== null) window.clearTimeout(saveTimer.current);
    };
  }, []);

  const apply = (next: AppSettings) => {
    settingsRef.current = next;
    setSettings(next);
  };

  /** Write the latest settings to disk once edits (e.g. a slider drag) settle. */
  const scheduleSave = () => {
    if (saveTimer.current !== null) window.clearTimeout(saveTimer.current);
    saveTimer.current = window.setTimeout(() => {
      const latest = settingsRef.current;
      if (!latest) return;
      saveSettings(latest).catch((e) => showToast(`Settings save failed: ${String(e)}`, "error"));
    }, 150);
  };

  /** Write a pending edit now instead of waiting for the debounce. */
  const flushSave = useCallback(async () => {
    if (saveTimer.current === null) return;
    window.clearTimeout(saveTimer.current);
    saveTimer.current = null;
    if (settingsRef.current) await saveSettings(settingsRef.current);
  }, []);

  /** The test reads settings from disk, so land any pending edit (e.g. a new pack) first. */
  const testCoach = async () => {
    try {
      await flushSave();
      const message = coachTestMessage(await testAudioCoach());
      if (message) showToast(message);
    } catch (e) {
      showToast(`Coach test failed: ${String(e)}`, "error");
    }
  };

  const update = (patch: Partial<AppSettings>) => {
    const current = settingsRef.current;
    if (!current) return;
    apply({ ...current, ...patch });
    scheduleSave();
  };

  const updateWidget = (patch: Partial<WidgetPlacement>) => {
    const current = settingsRef.current;
    if (!current) return;
    const widgets = current.overlayLayout.widgets.map((w, i) =>
      i === widgetIndex ? { ...w, ...patch } : w,
    );
    apply({ ...current, overlayLayout: { ...current.overlayLayout, widgets } });
    scheduleSave();
  };

  /** Binding edits: only keep them if the backend accepted (hotkey may be taken). */
  const saveBinding = useCallback(async (patch: Partial<AppSettings>) => {
    const current = settingsRef.current;
    if (!current) return;
    const next = { ...current, ...patch };
    try {
      await saveSettings(next);
      apply(next);
    } catch (e) {
      showToast(String(e), "error");
    }
  }, []);

  useEffect(() => {
    if (!capturingKey) return;
    const onKey = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
      if (e.code === "Escape") {
        setCapturingKey(false);
        return;
      }
      const accel = acceleratorFromEvent(e);
      if (!accel) return;
      setCapturingKey(false);
      void saveBinding({ vrRecenterHotkey: accel });
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [capturingKey, saveBinding]);

  const bindButton = async () => {
    setCapturingButton(true);
    try {
      const binding = await captureControllerButton();
      if (binding) {
        await saveBinding({ vrRecenterButton: binding });
      } else {
        showToast("No button pressed", "error");
      }
    } catch (e) {
      showToast(String(e), "error");
    } finally {
      setCapturingButton(false);
    }
  };

  const recenter = () =>
    recenterVr().catch((e) => showToast(`Recenter failed: ${String(e)}`, "error"));

  if (!settings) {
    return (
      <div className="settings-page">
        <p className="muted">Loading settings…</p>
      </div>
    );
  }

  const widget = settings.overlayLayout.widgets[widgetIndex];
  const defaults = defaultOverlayLayout().widgets[widgetIndex];
  const world = widget.vrLock === "world";
  const button = settings.vrRecenterButton;
  const native = settings.vrMode !== "web";

  return (
    <div className="settings-page">
      <div className="panel">
        <div className="panel-header">
          <h2>VR HUD</h2>
        </div>
        <div className="panel-body">
          <div className="btn-row">
            <span className="muted small settings-inline-label">Mode</span>
            <button
              type="button"
              className={`tab${native ? " active" : ""}`}
              onClick={() => update({ vrMode: "native" })}
            >
              Native (in-headset layer)
            </button>
            <button
              type="button"
              className={`tab${!native ? " active" : ""}`}
              onClick={() => update({ vrMode: "web" })}
            >
              Web (browser / OpenKneeboard)
            </button>
          </div>
          <p className="muted small">
            {native
              ? "Draws widgets directly in the headset through the Race Refinery OpenXR layer."
              : "Serves the HUD at http://127.0.0.1:17342/vr for a browser or OpenKneeboard tab."}{" "}
            Takes effect the next time you start the HUD.
          </p>
        </div>
      </div>

      <div className="panel">
        <div className="panel-header">
          <h2>VR placement</h2>
        </div>
        <div className="panel-body">
          <div className="btn-row">
            {WIDGET_NAMES.map((name, i) => (
              <button
                key={name}
                type="button"
                className={`tab${i === widgetIndex ? " active" : ""}`}
                onClick={() => setWidgetIndex(i)}
              >
                {name}
              </button>
            ))}
          </div>

          <div className="settings-section">
            <label className="toggle-row">
              <input
                type="checkbox"
                checked={widget.enabled}
                onChange={(e) => updateWidget({ enabled: e.target.checked })}
              />
              <span>Show {WIDGET_NAMES[widgetIndex]} (headset and monitor overlays)</span>
            </label>

            <div className="btn-row settings-mode">
              <span className="muted small">Anchor</span>
              <button
                type="button"
                className={`tab${world ? " active" : ""}`}
                onClick={() => updateWidget({ vrLock: "world" })}
              >
                Fixed in cockpit
              </button>
              <button
                type="button"
                className={`tab${!world ? " active" : ""}`}
                onClick={() => updateWidget({ vrLock: "head" })}
              >
                Follow head
              </button>
            </div>
            <p className="muted small">
              {world
                ? "Stays put in the car; look at it when you want it. Recenter to bring it in front of you."
                : "Moves with your head and always stays in view."}
            </p>
          </div>

          <div className="vr-sliders">
            <Slider
              label="Depth"
              value={widget.vrOffsetZ}
              min={-0.6}
              max={0.8}
              step={0.02}
              format={(v) => (v === 0 ? "default" : `${Math.abs(v).toFixed(2)} m ${v > 0 ? "closer" : "farther"}`)}
              onChange={(v) => updateWidget({ vrOffsetZ: v })}
            />
            <Slider
              label="Left/right"
              value={widget.vrOffsetX}
              min={-1}
              max={1}
              step={0.02}
              format={meters}
              onChange={(v) => updateWidget({ vrOffsetX: v })}
            />
            <Slider
              label="Height"
              value={widget.vrOffsetY}
              min={-0.6}
              max={0.6}
              step={0.02}
              format={meters}
              onChange={(v) => updateWidget({ vrOffsetY: v })}
            />
            <Slider
              label="Tilt"
              value={widget.vrTiltDeg}
              min={-45}
              max={45}
              step={1}
              format={(v) => `${v.toFixed(0)}°`}
              onChange={(v) => updateWidget({ vrTiltDeg: v })}
            />
            <Slider
              label="Size"
              value={widget.vrScale}
              min={0.25}
              max={1.25}
              step={0.05}
              format={(v) => `${v.toFixed(2)}×`}
              onChange={(v) => updateWidget({ vrScale: v })}
            />
            <Slider
              label="Opacity"
              value={widget.vrOpacity}
              min={0.2}
              max={1}
              step={0.05}
              format={(v) => `${Math.round(v * 100)}%`}
              onChange={(v) => updateWidget({ vrOpacity: v })}
            />
          </div>
          <div className="btn-row settings-section">
            <button
              type="button"
              className="btn btn-ghost"
              onClick={() =>
                updateWidget({
                  vrLock: defaults.vrLock,
                  vrOffsetX: defaults.vrOffsetX,
                  vrOffsetY: defaults.vrOffsetY,
                  vrOffsetZ: defaults.vrOffsetZ,
                  vrTiltDeg: defaults.vrTiltDeg,
                  vrScale: defaults.vrScale,
                  vrOpacity: defaults.vrOpacity,
                })
              }
            >
              Reset placement
            </button>
          </div>
          <p className="muted small">
            Changes apply in the headset within a frame or two while the HUD is running.
            Monitor overlay windows keep their own size and position.
          </p>
        </div>
      </div>

      <div className="panel">
        <div className="panel-header">
          <h2>VR recenter</h2>
        </div>
        <div className="panel-body">
          <p className="muted small">
            Recentering places cockpit-fixed widgets in front of where your head is right now.
            Bindings work while iRacing has focus, and iRacing still receives the press.
          </p>
          <div className="btn-row">
            <button type="button" className="btn" onClick={recenter}>
              Recenter now
            </button>
          </div>

          <div className="bind-row">
            <span className="bind-label">Keyboard</span>
            <span className="bind-value">
              {capturingKey ? (
                "Press a key combo… (Esc to cancel)"
              ) : settings.vrRecenterHotkey ? (
                <code>{settings.vrRecenterHotkey}</code>
              ) : (
                <span className="muted">Not set</span>
              )}
            </span>
            <button
              type="button"
              className="btn"
              disabled={capturingButton}
              onClick={() => setCapturingKey((v) => !v)}
            >
              {capturingKey ? "Cancel" : "Bind"}
            </button>
            <button
              type="button"
              className="btn btn-ghost"
              disabled={!settings.vrRecenterHotkey || capturingKey}
              onClick={() => void saveBinding({ vrRecenterHotkey: "" })}
            >
              Clear
            </button>
          </div>

          <div className="bind-row">
            <span className="bind-label">Wheel button</span>
            <span className="bind-value">
              {capturingButton ? (
                "Press a button on your wheel or button box…"
              ) : button ? (
                <code>
                  {button.deviceName || "Controller"} – Button {button.button + 1}
                </code>
              ) : (
                <span className="muted">Not set</span>
              )}
            </span>
            <button
              type="button"
              className="btn"
              disabled={capturingButton || capturingKey}
              onClick={() => void bindButton()}
            >
              Bind
            </button>
            <button
              type="button"
              className="btn btn-ghost"
              disabled={!button || capturingButton}
              onClick={() => void saveBinding({ vrRecenterButton: null })}
            >
              Clear
            </button>
          </div>
          <p className="muted small">
            Keyboard combos need a modifier (Ctrl, Alt, Shift) unless you use F-keys, the numpad,
            Pause, Scroll Lock, or Insert. The combo is reserved system-wide while Race Refinery runs.
          </p>
        </div>
      </div>

      <div className="panel">
        <div className="panel-header">
          <h2>Audio coach</h2>
        </div>
        <div className="panel-body">
          <label className="toggle-row">
            <input
              type="checkbox"
              checked={settings.audioCoachEnabled}
              onChange={(e) => update({ audioCoachEnabled: e.target.checked })}
            />
            <span>Enable the audio coach</span>
          </label>

          <VoicePackSection
            packId={settings.audioCoachPackId}
            flushSave={flushSave}
            onSettings={apply}
            onSelect={(id) => update({ audioCoachPackId: id })}
            onTest={() => void testCoach()}
          />

          <div className="vr-sliders">
            <Slider
              label="Volume"
              value={settings.audioCoachVolume}
              min={0}
              max={1}
              step={0.05}
              format={(v) => `${Math.round(v * 100)}%`}
              onChange={(v) => update({ audioCoachVolume: v })}
            />
            <Slider
              label="Pause"
              value={settings.audioInterMessageGapMs}
              min={0}
              max={2000}
              step={100}
              format={(v) => `${(v / 1000).toFixed(1)} s`}
              onChange={(v) => update({ audioInterMessageGapMs: v })}
            />
            <Slider
              label="Low fuel"
              value={settings.audioCoachFuelThreshold}
              min={0}
              max={20}
              step={0.5}
              format={(v) => (v === 0 ? "off" : `${v.toFixed(1)} L`)}
              onChange={(v) => update({ audioCoachFuelThreshold: v })}
            />
          </div>

          <div className="settings-section">
            <div className="btn-row">
              <span className="muted small settings-inline-label">Chatter</span>
              {(["minimal", "normal", "verbose"] as const).map((level) => (
                <button
                  key={level}
                  type="button"
                  className={`tab${settings.audioCoachChatterLevel === level ? " active" : ""}`}
                  onClick={() => update({ audioCoachChatterLevel: level })}
                >
                  {level[0].toUpperCase() + level.slice(1)}
                </button>
              ))}
            </div>
            <div className="btn-row">
              <span className="muted small settings-inline-label">Fuel calls</span>
              {(["normal", "conservative"] as const).map((mode) => (
                <button
                  key={mode}
                  type="button"
                  className={`tab${settings.audioFuelStrategySensitivity === mode ? " active" : ""}`}
                  onClick={() => update({ audioFuelStrategySensitivity: mode })}
                >
                  {mode === "normal" ? "Normal margin" : "Extra margin"}
                </button>
              ))}
            </div>
            <label className="toggle-row">
              <input
                type="checkbox"
                checked={settings.audioRadioEffectsEnabled}
                onChange={(e) => update({ audioRadioEffectsEnabled: e.target.checked })}
              />
              <span>Radio beep before each call</span>
            </label>
          </div>

          <div className="settings-section">
            <h3 className="settings-subhead">Callouts</h3>
            <div className="settings-toggle-grid">
              {COACH_CATEGORIES.map(({ key, label }) => (
                <label key={key} className="toggle-row">
                  <input
                    type="checkbox"
                    checked={Boolean(settings[key])}
                    onChange={(e) => update({ [key]: e.target.checked } as Partial<AppSettings>)}
                  />
                  <span>{label}</span>
                </label>
              ))}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
