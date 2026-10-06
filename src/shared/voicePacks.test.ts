import { describe, expect, it } from "vitest";
import type { VoicePackStatus } from "./types";
import { coachTestMessage, packLabel } from "./voicePacks";

const pack = (patch: Partial<VoicePackStatus>): VoicePackStatus => ({
  id: "mine",
  name: "Mine",
  author: "",
  kind: "user",
  readOnly: false,
  dir: "",
  spotter: { recorded: 0, total: 38 },
  engineer: { recorded: 0, total: 61 },
  missing: [],
  customBeep: false,
  ...patch,
});

describe("packLabel", () => {
  it("marks bundled and folder packs", () => {
    expect(packLabel(pack({}))).toBe("Mine");
    expect(packLabel(pack({ kind: "bundled", readOnly: true, name: "Default" }))).toBe(
      "Default (bundled)",
    );
    expect(packLabel(pack({ kind: "folder" }))).toBe("Mine (folder)");
  });
});

describe("coachTestMessage", () => {
  it("is silent when everything played", () => {
    expect(coachTestMessage({ text: "lap", missing: [] })).toBeNull();
  });

  it("lists a few missing keys", () => {
    const msg = coachTestMessage({ text: "", missing: ["lap", "n12", "n1", "minute", "n20", "n9"] });
    expect(msg).toContain("Skipped 6 clip(s)");
    expect(msg).toContain("lap, n12, n1, minute and 2 more");
  });
});
