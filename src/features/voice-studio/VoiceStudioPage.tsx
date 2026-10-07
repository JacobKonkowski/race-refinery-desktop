import { useCallback, useEffect, useMemo, useState } from "react";
import {
  cloneVoicePack,
  createVoicePack,
  getSettings,
  listVoicePacks,
  listVoicePhrases,
  onSettingsChanged,
  patchSettings,
  setActiveVoicePack,
} from "../../shared/api";
import { TierBadges } from "../../shared/TierBadges";
import { showToast } from "../../shared/toast";
import type { AppSettings, VoicePackStatus, VoicePhrase } from "../../shared/types";
import { PreviewTab } from "./PreviewTab";
import { RecordTab } from "./RecordTab";

type Tab = "record" | "preview";

/** Prefer the coach's pack when it is editable, else the first editable pack. */
function initialPack(packs: VoicePackStatus[], activeId: string): string | null {
  const active = packs.find((p) => p.id === activeId);
  if (active && !active.readOnly) return active.id;
  return (packs.find((p) => !p.readOnly) ?? active ?? packs[0])?.id ?? null;
}

export function VoiceStudioPage() {
  const [phrases, setPhrases] = useState<VoicePhrase[]>([]);
  const [packs, setPacks] = useState<VoicePackStatus[]>([]);
  const [packId, setPackId] = useState<string | null>(null);
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [tab, setTab] = useState<Tab>("record");
  const [focusKey, setFocusKey] = useState<string | null>(null);
  const [naming, setNaming] = useState<"new" | "clone" | null>(null);
  const [name, setName] = useState("");

  useEffect(() => {
    Promise.all([listVoicePhrases(), listVoicePacks(), getSettings()])
      .then(([ph, pk, s]) => {
        setPhrases(ph);
        setPacks(pk);
        setSettings(s);
        setPackId(initialPack(pk, s.audioCoachPackId));
      })
      .catch((e) => showToast(`Could not load Voice Studio: ${String(e)}`, "error"));
    const unlisten = onSettingsChanged(setSettings);
    return () => {
      unlisten.then((fn) => fn()).catch(() => undefined);
    };
  }, []);

  const pack = useMemo(() => packs.find((p) => p.id === packId) ?? null, [packs, packId]);

  const updatePack = useCallback((status: VoicePackStatus) => {
    setPacks((all) => all.map((p) => (p.id === status.id ? status : p)));
  }, []);

  const updateSettings = useCallback((patch: Partial<AppSettings>) => {
    setSettings((s) => (s ? { ...s, ...patch } : s));
    patchSettings(patch)
      .then(setSettings)
      .catch((e) => showToast(`Settings save failed: ${String(e)}`, "error"));
  }, []);

  const startNaming = (mode: "new" | "clone") => {
    setNaming(mode);
    setName(mode === "clone" && pack ? `${pack.name} copy` : "My voice");
  };

  const submitName = async () => {
    if (!naming || !name.trim()) return;
    try {
      const created =
        naming === "clone" && pack
          ? await cloneVoicePack(pack.id, name.trim())
          : await createVoicePack(name.trim());
      setPacks(await listVoicePacks());
      setPackId(created.id);
      setNaming(null);
      showToast(`Created “${created.name}”`, "success");
    } catch (e) {
      showToast(String(e), "error");
    }
  };

  const useForCoach = async () => {
    if (!pack) return;
    try {
      setSettings(await setActiveVoicePack(pack.id));
      showToast(`Coach now speaks with “${pack.name}”`, "success");
    } catch (e) {
      showToast(String(e), "error");
    }
  };

  const jumpToKey = (key: string) => {
    setFocusKey(key);
    setTab("record");
  };

  if (!settings) {
    return (
      <div className="studio-page">
        <p className="muted">Loading Voice Studio…</p>
      </div>
    );
  }

  const isActive = pack?.id === settings.audioCoachPackId;

  return (
    <div className="studio-page">
      <div className="panel">
        <div className="panel-header">
          <h2>Voice Studio</h2>
          {pack ? <TierBadges spotter={pack.spotter} engineer={pack.engineer} /> : null}
        </div>
        <div className="panel-body">
          <p className="muted small studio-intro">
            The coach speaks only with recorded clips. Record each phrase once and callouts like
            lap times are stitched together from them. Clips that are missing are skipped on
            track. Finish <strong>Spotter</strong> first for flags and traffic, then{" "}
            <strong>Engineer</strong> for numbers.
          </p>
          <div className="settings-field">
            <span className="muted small settings-inline-label">Pack</span>
            <select
              aria-label="Voice pack"
              value={packId ?? ""}
              onChange={(e) => {
                setPackId(e.target.value);
                setFocusKey(null);
              }}
            >
              {packs.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.name}
                  {p.readOnly ? " (bundled, read-only)" : p.kind === "folder" ? " (folder)" : ""}
                  {p.id === settings.audioCoachPackId ? " · coach" : ""}
                </option>
              ))}
            </select>
            <button type="button" className="btn" onClick={() => startNaming("new")}>
              New pack
            </button>
            <button
              type="button"
              className="btn"
              disabled={!pack}
              onClick={() => startNaming("clone")}
            >
              Clone
            </button>
            <button
              type="button"
              className="btn"
              disabled={!pack || isActive}
              onClick={() => void useForCoach()}
            >
              {isActive ? "Coach voice" : "Use for coach"}
            </button>
          </div>
          {naming ? (
            <form
              className="settings-field studio-name-form"
              onSubmit={(e) => {
                e.preventDefault();
                void submitName();
              }}
            >
              <span className="muted small settings-inline-label">
                {naming === "clone" ? "Clone as" : "Name"}
              </span>
              <input
                className="studio-input"
                autoFocus
                value={name}
                onChange={(e) => setName(e.target.value)}
                aria-label="Pack name"
              />
              <button type="submit" className="btn btn-primary" disabled={!name.trim()}>
                Create
              </button>
              <button type="button" className="btn btn-ghost" onClick={() => setNaming(null)}>
                Cancel
              </button>
            </form>
          ) : null}
          <div className="btn-row studio-tabs">
            <button
              type="button"
              className={`tab${tab === "record" ? " active" : ""}`}
              onClick={() => setTab("record")}
            >
              Record
            </button>
            <button
              type="button"
              className={`tab${tab === "preview" ? " active" : ""}`}
              onClick={() => setTab("preview")}
            >
              Preview
            </button>
          </div>
        </div>
      </div>

      {pack ? (
        tab === "record" ? (
          <RecordTab
            key={pack.id}
            pack={pack}
            phrases={phrases}
            settings={settings}
            focusKey={focusKey}
            onPackChanged={updatePack}
            onSettings={updateSettings}
            onClone={() => startNaming("clone")}
          />
        ) : (
          <PreviewTab key={pack.id} pack={pack} phrases={phrases} onJumpToKey={jumpToKey} />
        )
      ) : (
        <p className="muted">No voice packs found.</p>
      )}
    </div>
  );
}
