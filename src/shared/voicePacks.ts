import type { VoicePackStatus, VoicePlayReport } from "./types";

/** Picker label: name plus where the pack lives. */
export function packLabel(pack: VoicePackStatus): string {
  if (pack.readOnly) return `${pack.name} (bundled)`;
  if (pack.kind === "folder") return `${pack.name} (folder)`;
  return pack.name;
}

/** Toast text for a coach test that skipped clips; null when nothing was missing. */
export function coachTestMessage(report: VoicePlayReport): string | null {
  if (report.missing.length === 0) return null;
  const shown = report.missing.slice(0, 4).join(", ");
  const more = report.missing.length > 4 ? ` and ${report.missing.length - 4} more` : "";
  return `Skipped ${report.missing.length} clip(s) this pack has not recorded: ${shown}${more}. Record them in Voice Studio.`;
}
