/**
 * Handoff from Lap Library → Analyze. Survives feature remounts when the shell
 * swaps the active outlet.
 */

export type AnalyzeOpen =
  | { kind: "empty" }
  | { kind: "session"; sessionId: number; candidateLapId: number | null }
  | {
      kind: "selection";
      lapIds: number[];
      track: string;
      name: string;
      setId: number | null;
    };

export type LibraryBag = {
  lapIds: number[];
  track: string;
  name: string;
  setId: number | null;
};

let analyzeOpen: AnalyzeOpen = { kind: "empty" };
let libraryBag: LibraryBag = { lapIds: [], track: "", name: "", setId: null };

const analyzeListeners = new Set<() => void>();
const bagListeners = new Set<() => void>();

function emit(listeners: Set<() => void>) {
  for (const listener of listeners) listener();
}

export function getAnalyzeOpen(): AnalyzeOpen {
  return analyzeOpen;
}

export function setAnalyzeOpen(next: AnalyzeOpen) {
  analyzeOpen = next;
  emit(analyzeListeners);
}

export function subscribeAnalyzeOpen(listener: () => void): () => void {
  analyzeListeners.add(listener);
  return () => {
    analyzeListeners.delete(listener);
  };
}

export function getLibraryBag(): LibraryBag {
  return libraryBag;
}

export function setLibraryBag(next: LibraryBag) {
  libraryBag = next;
  emit(bagListeners);
}

export function subscribeLibraryBag(listener: () => void): () => void {
  bagListeners.add(listener);
  return () => {
    bagListeners.delete(listener);
  };
}

export function openSessionInAnalyze(sessionId: number, candidateLapId: number | null = null) {
  setAnalyzeOpen({ kind: "session", sessionId, candidateLapId });
}

export function openSelectionInAnalyze(
  lapIds: number[],
  track: string,
  opts?: { name?: string; setId?: number | null },
) {
  setAnalyzeOpen({
    kind: "selection",
    lapIds: [...lapIds],
    track,
    name: opts?.name ?? "",
    setId: opts?.setId ?? null,
  });
}
