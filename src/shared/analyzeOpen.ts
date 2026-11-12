/**
 * Handoff from Lap Library → Analyze. Survives feature remounts when the shell
 * swaps the active outlet.
 */

export type AnalyzeOpen =
  | { kind: "empty" }
  | { kind: "session"; sessionId: number; candidateLapId: number | null; openSeq: number }
  | {
      kind: "selection";
      lapIds: number[];
      track: string;
      name: string;
      setId: number | null;
      openSeq: number;
    };

export type LibraryBag = {
  lapIds: number[];
  track: string;
  name: string;
  setId: number | null;
};

let analyzeOpen: AnalyzeOpen = { kind: "empty" };
let libraryBag: LibraryBag = { lapIds: [], track: "", name: "", setId: null };
let openSeq = 0;

const analyzeListeners = new Set<() => void>();
const bagListeners = new Set<() => void>();

function emit(listeners: Set<() => void>) {
  for (const listener of listeners) listener();
}

function nextOpenSeq(): number {
  openSeq += 1;
  return openSeq;
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
  setAnalyzeOpen({ kind: "session", sessionId, candidateLapId, openSeq: nextOpenSeq() });
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
    openSeq: nextOpenSeq(),
  });
}
