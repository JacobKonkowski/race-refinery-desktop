import { describe, expect, it } from "vitest";
import type { VoicePhrase } from "../../shared/types";
import {
  adjacentKey,
  filterPhrases,
  groupPhrases,
  nextMissingKey,
  optionalNumber,
  parseLapTime,
} from "./voiceStudio";

const phrase = (key: string, tier: VoicePhrase["tier"], category: string): VoicePhrase => ({
  key,
  prompt: key,
  tier,
  category,
});

const PHRASES = [
  phrase("flag_green", "spotter", "flags"),
  phrase("flag_red", "spotter", "flags"),
  phrase("pits_open", "spotter", "race"),
  phrase("n0", "engineer", "numbers"),
  phrase("n1", "engineer", "numbers"),
];

describe("filterPhrases", () => {
  it("filters by tier and by missing clips", () => {
    const missing = new Set(["flag_red", "n1"]);
    expect(filterPhrases(PHRASES, "spotter", missing)).toHaveLength(3);
    expect(filterPhrases(PHRASES, "engineer", missing).map((p) => p.key)).toEqual(["n0", "n1"]);
    expect(filterPhrases(PHRASES, "missing", missing).map((p) => p.key)).toEqual([
      "flag_red",
      "n1",
    ]);
    expect(filterPhrases(PHRASES, "all", missing)).toBe(PHRASES);
  });
});

describe("groupPhrases", () => {
  it("groups consecutive tier + category runs", () => {
    const groups = groupPhrases(PHRASES);
    expect(groups.map((g) => [g.category, g.phrases.length])).toEqual([
      ["flags", 2],
      ["race", 1],
      ["numbers", 2],
    ]);
  });
});

describe("nextMissingKey", () => {
  it("moves forward and wraps around", () => {
    const missing = new Set(["flag_green", "n0"]);
    expect(nextMissingKey(PHRASES, missing, "flag_green")).toBe("n0");
    expect(nextMissingKey(PHRASES, missing, "n0")).toBe("flag_green");
    expect(nextMissingKey(PHRASES, missing, "n1")).toBe("flag_green");
  });

  it("finishes when nothing else is missing", () => {
    expect(nextMissingKey(PHRASES, new Set(["pits_open"]), "pits_open")).toBeNull();
    expect(nextMissingKey(PHRASES, new Set(), "pits_open")).toBeNull();
  });
});

describe("adjacentKey", () => {
  it("steps and clamps", () => {
    expect(adjacentKey(PHRASES, "flag_red", 1)).toBe("pits_open");
    expect(adjacentKey(PHRASES, "flag_green", -1)).toBe("flag_green");
    expect(adjacentKey(PHRASES, "n1", 1)).toBe("n1");
    expect(adjacentKey([], "x", 1)).toBeNull();
  });
});

describe("parseLapTime", () => {
  it("accepts m:ss.sss and plain seconds", () => {
    expect(parseLapTime("1:29.452")).toBe(89_452);
    expect(parseLapTime("42.3")).toBe(42_300);
    expect(parseLapTime(" 2:05 ")).toBe(125_000);
  });

  it("rejects junk", () => {
    expect(parseLapTime("")).toBeNull();
    expect(parseLapTime("1:75.0")).toBeNull();
    expect(parseLapTime("fast")).toBeNull();
  });
});

describe("optionalNumber", () => {
  it("treats blank as unset", () => {
    expect(optionalNumber("")).toBeUndefined();
    expect(optionalNumber("abc")).toBeUndefined();
    expect(optionalNumber("-0.3")).toBe(-0.3);
  });
});
