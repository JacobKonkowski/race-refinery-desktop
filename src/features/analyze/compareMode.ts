import type { LapSummary } from "../../shared/types";

/** How ComparePanel should load and render for the current lap selection. */
export type CompareMode = "empty" | "solo" | "compare";

/**
 * Solo = one lap to inspect (reference only, or Cand and Ref are the same lap).
 * Compare = two different laps for overlay.
 */
export function compareMode(
  candidate: LapSummary | null,
  reference: LapSummary | null,
): CompareMode {
  if (!reference) return "empty";
  if (!candidate || candidate.id === reference.id) return "solo";
  return "compare";
}
