import { useCallback, useEffect, useState, type FormEvent } from "react";
import {
  cloneVoicePack,
  confirmDialog,
  deleteVoicePack,
  exportVoicePackZip,
  getSettings,
  importVoicePackWavs,
  importVoicePackZip,
  linkVoicePackFolder,
  listVoicePacks,
  unlinkVoicePackFolder,
} from "../../shared/api";
import { navigateToFeature } from "../../shared/navigation";
import { TierBadges } from "../../shared/TierBadges";
import { showToast } from "../../shared/toast";
import type { AppSettings, VoiceImportReport, VoicePackStatus } from "../../shared/types";
import { packLabel } from "../../shared/voicePacks";

interface Props {
  packId: string;
  /** Land pending local edits before a command that rewrites settings on disk. */
  flushSave: () => Promise<void>;
  /** Adopt settings the backend just persisted. */
  onSettings: (settings: AppSettings) => void;
  onSelect: (packId: string) => void;
  onTest: () => void;
}

function importSummary(report: VoiceImportReport): string {
  const parts = [`Imported ${report.imported.length} clip(s)`];
  if (report.overwritten.length) parts.push(`${report.overwritten.length} replaced`);
  if (report.unknown.length) parts.push(`${report.unknown.length} unknown file(s) skipped`);
  if (report.failed.length) parts.push(`${report.failed.length} unreadable`);
  return parts.join(", ");
}

export function VoicePackSection({ packId, flushSave, onSettings, onSelect, onTest }: Props) {
  const [packs, setPacks] = useState<VoicePackStatus[]>([]);
  const [cloneName, setCloneName] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const refresh = useCallback(async () => {
    try {
      setPacks(await listVoicePacks());
    } catch (e) {
      showToast(`Could not list voice packs: ${String(e)}`, "error");
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const active = packs.find((p) => p.id === packId) ?? null;

  /** Run a pack command, then reload the list (and settings when the backend rewrote them). */
  const run = async (work: () => Promise<void>, settingsChanged = false) => {
    setBusy(true);
    try {
      if (settingsChanged) await flushSave();
      await work();
      if (settingsChanged) onSettings(await getSettings());
      await refresh();
    } catch (e) {
      showToast(String(e), "error");
    } finally {
      setBusy(false);
    }
  };

  const submitClone = (e: FormEvent) => {
    e.preventDefault();
    const name = cloneName?.trim();
    if (!name || !active) return;
    void run(async () => {
      const pack = await cloneVoicePack(active.id, name);
      setCloneName(null);
      onSelect(pack.id);
      showToast(`Created "${pack.name}"`, "success");
    });
  };

  const importZip = () =>
    run(async () => {
      const result = await importVoicePackZip();
      if (!result) return;
      onSelect(result.pack.id);
      showToast(`${result.pack.name}: ${importSummary(result.report)}`, "success");
    });

  const importWavs = () =>
    run(async () => {
      if (!active) return;
      const report = await importVoicePackWavs(active.id);
      if (report) showToast(importSummary(report), "success");
    });

  const linkFolder = () =>
    run(async () => {
      await flushSave();
      const pack = await linkVoicePackFolder();
      if (!pack) return;
      onSettings(await getSettings());
      onSelect(pack.id);
      showToast(`Using folder pack "${pack.name}"`, "success");
    });

  const exportZip = () =>
    run(async () => {
      if (!active) return;
      const path = await exportVoicePackZip(active.id);
      if (path) showToast(`Exported to ${path}`, "success");
    });

  const removeActive = () =>
    run(async () => {
      if (!active) return;
      if (active.kind === "user") {
        const ok = await confirmDialog(
          `Delete "${active.name}" and all of its recordings? This cannot be undone.`,
          "Delete voice pack",
        );
        if (!ok) return;
        await deleteVoicePack(active.id);
      } else {
        await unlinkVoicePackFolder(active.id);
      }
    }, true);

  return (
    <div className="settings-section">
      <div className="settings-field">
        <span className="muted small settings-inline-label">Voice pack</span>
        <select
          aria-label="Coach voice pack"
          value={packId}
          disabled={busy}
          onChange={(e) => onSelect(e.target.value)}
        >
          {packs.map((p) => (
            <option key={p.id} value={p.id}>
              {packLabel(p)}
            </option>
          ))}
          {packs.length > 0 && !active ? (
            <option value={packId}>{packId} (not found, using bundled)</option>
          ) : null}
        </select>
        <button type="button" className="btn" disabled={busy} onClick={onTest}>
          Test
        </button>
      </div>
      {active ? (
        <div className="btn-row">
          <TierBadges spotter={active.spotter} engineer={active.engineer} />
          {active.customBeep ? <span className="muted small">Custom radio beep</span> : null}
        </div>
      ) : null}
      <p className="muted small">
        The coach only speaks recorded clips, so a callout with a missing clip is skipped. Record
        the Spotter set first: it covers flags, traffic, and other safety calls. The bundled pack
        is read-only; clone it to record your own.
      </p>
      <div className="btn-row voice-pack-actions">
        <button
          type="button"
          className="btn btn-primary"
          onClick={() => navigateToFeature("voice-studio")}
        >
          Open Voice Studio
        </button>
        <button
          type="button"
          className="btn"
          disabled={busy || !active}
          onClick={() => setCloneName(active ? `${active.name} copy` : "")}
        >
          Clone…
        </button>
        <button type="button" className="btn" disabled={busy} onClick={() => void importZip()}>
          Import zip
        </button>
        <button
          type="button"
          className="btn"
          disabled={busy || !active || active.readOnly}
          title={active?.readOnly ? "Clone the bundled pack first" : undefined}
          onClick={() => void importWavs()}
        >
          Import WAVs
        </button>
        <button type="button" className="btn" disabled={busy} onClick={() => void linkFolder()}>
          Use folder…
        </button>
        <button
          type="button"
          className="btn"
          disabled={busy || !active}
          onClick={() => void exportZip()}
        >
          Export zip
        </button>
        {active?.kind === "user" || active?.kind === "folder" ? (
          <button
            type="button"
            className="btn btn-ghost"
            disabled={busy}
            onClick={() => void removeActive()}
          >
            {active.kind === "user" ? "Delete pack" : "Unlink folder"}
          </button>
        ) : null}
      </div>
      {cloneName !== null ? (
        <form className="btn-row studio-name-form" onSubmit={submitClone}>
          <input
            className="studio-input"
            aria-label="New pack name"
            autoFocus
            value={cloneName}
            onChange={(e) => setCloneName(e.target.value)}
          />
          <button type="submit" className="btn btn-primary" disabled={busy || !cloneName.trim()}>
            Create
          </button>
          <button type="button" className="btn btn-ghost" onClick={() => setCloneName(null)}>
            Cancel
          </button>
        </form>
      ) : null}
    </div>
  );
}
