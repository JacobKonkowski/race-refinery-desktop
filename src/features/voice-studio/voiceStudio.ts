/** Pure helpers for the Voice Studio checklist, teleprompter, and composer. */
import type { VoicePhrase } from "../../shared/types";

export type PhraseFilter = "all" | "spotter" | "engineer" | "missing";

export const FILTERS: { id: PhraseFilter; label: string }[] = [
  { id: "all", label: "All" },
  { id: "spotter", label: "Spotter" },
  { id: "engineer", label: "Engineer" },
  { id: "missing", label: "Missing only" },
];

const CATEGORY_LABELS: Record<string, string> = {
  intro: "Intro",
  flags: "Flags",
  traffic: "Traffic",
  gaps: "Gaps",
  race: "Race",
  fuel: "Fuel",
  pace: "Pace",
  numbers: "Numbers",
  glue: "Glue words",
};

export function categoryLabel(category: string): string {
  return CATEGORY_LABELS[category] ?? category;
}

export function filterPhrases(
  phrases: VoicePhrase[],
  filter: PhraseFilter,
  missing: ReadonlySet<string>,
): VoicePhrase[] {
  switch (filter) {
    case "spotter":
    case "engineer":
      return phrases.filter((p) => p.tier === filter);
    case "missing":
      return phrases.filter((p) => missing.has(p.key));
    default:
      return phrases;
  }
}

/** Consecutive runs of one tier + category, for checklist section headers. */
export function groupPhrases(
  phrases: VoicePhrase[],
): { id: string; tier: VoicePhrase["tier"]; category: string; phrases: VoicePhrase[] }[] {
  const groups: ReturnType<typeof groupPhrases> = [];
  for (const p of phrases) {
    const last = groups[groups.length - 1];
    if (last && last.tier === p.tier && last.category === p.category) {
      last.phrases.push(p);
    } else {
      groups.push({ id: `${p.tier}-${p.category}`, tier: p.tier, category: p.category, phrases: [p] });
    }
  }
  return groups;
}

/**
 * The next missing phrase after `current` in list order, wrapping around.
 * Registry order puts Spotter before Engineer, so "All" finishes Spotter first.
 */
export function nextMissingKey(
  list: VoicePhrase[],
  missing: ReadonlySet<string>,
  current: string,
): string | null {
  const start = list.findIndex((p) => p.key === current);
  for (let i = 1; i <= list.length; i++) {
    const p = list[(start + i + list.length) % list.length];
    if (p.key !== current && missing.has(p.key)) return p.key;
  }
  return null;
}

/** The phrase `step` places away from `current`, clamped to the list. */
export function adjacentKey(list: VoicePhrase[], current: string, step: number): string | null {
  if (list.length === 0) return null;
  const index = list.findIndex((p) => p.key === current);
  const next = Math.min(list.length - 1, Math.max(0, (index < 0 ? 0 : index) + step));
  return list[next].key;
}

/** "1:29.452" or "89.452" seconds -> milliseconds; null when unparseable. */
export function parseLapTime(text: string): number | null {
  const trimmed = text.trim();
  if (!trimmed) return null;
  const match = /^(?:(\d+):)?(\d+(?:\.\d+)?)$/.exec(trimmed);
  if (!match) return null;
  const minutes = match[1] ? Number(match[1]) : 0;
  const seconds = Number(match[2]);
  if (match[1] && seconds >= 60) return null;
  return Math.round((minutes * 60 + seconds) * 1000);
}

/** Empty input -> undefined, else a finite number (or undefined). */
export function optionalNumber(text: string): number | undefined {
  if (text.trim() === "") return undefined;
  const n = Number(text);
  return Number.isFinite(n) ? n : undefined;
}
