import type { LapSummary } from "../../shared/types";

/** How ComparePanel should load and render for the current lap selection. */
export type CompareMode = "empty" | "solo" | "same" | "compare";

export function compareMode(
  candidate: LapSummary | null,
  reference: LapSummary | null,
): CompareMode {
  if (!reference) return "empty";
  if (!candidate) return "solo";
  if (candidate.id === reference.id) return "same";
  return "compare";
}
