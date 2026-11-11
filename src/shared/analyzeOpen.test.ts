import { afterEach, describe, expect, it } from "vitest";
import {
  getAnalyzeOpen,
  getLibraryBag,
  openSelectionInAnalyze,
  openSessionInAnalyze,
  setAnalyzeOpen,
  setLibraryBag,
} from "./analyzeOpen";

afterEach(() => {
  setAnalyzeOpen({ kind: "empty" });
  setLibraryBag({ lapIds: [], track: "", name: "", setId: null });
});

describe("analyzeOpen handoff", () => {
  it("opens a session for Analyze", () => {
    openSessionInAnalyze(12, 34);
    expect(getAnalyzeOpen()).toEqual({
      kind: "session",
      sessionId: 12,
      candidateLapId: 34,
    });
  });

  it("opens a selection bag for Analyze", () => {
    openSelectionInAnalyze([1, 2, 3], "Spa", { name: "Race pace", setId: 9 });
    expect(getAnalyzeOpen()).toEqual({
      kind: "selection",
      lapIds: [1, 2, 3],
      track: "Spa",
      name: "Race pace",
      setId: 9,
    });
  });

  it("keeps the library bag independent of Analyze open", () => {
    setLibraryBag({ lapIds: [7], track: "Monza", name: "Test", setId: 1 });
    openSessionInAnalyze(99);
    expect(getLibraryBag().lapIds).toEqual([7]);
    expect(getAnalyzeOpen().kind).toBe("session");
  });
});
