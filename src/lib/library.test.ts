import { describe, expect, it } from "vitest";
import { counts, currentEntry, isInstalled, normalize, view, type Game, type Platform, type PlatformEntry } from "./library";

const entry = (platform: Platform, o: Partial<PlatformEntry> = {}): PlatformEntry => ({
  platform, platform_id: "1", title: "t", installed: false, install_path: null, size_bytes: null,
  last_played: null, playtime_min: null, launch_uri: "", install_uri: null, ...o,
});
let n = 0;
const game = (title: string, entries: PlatformEntry[], o: Partial<Game> = {}): Game => ({
  id: ++n, title, norm_title: normalize(title), cover_path: null, preferred: null, last_launched: null, entries, ...o,
});

describe("normalize (parity with Rust)", () => {
  it("matches Rust examples", () => {
    expect(normalize("İstanbul Kıyamet")).toBe("istanbul kiyamet");
    expect(normalize("Hades®")).toBe("hades");
    expect(normalize("Control: Ultimate Edition")).toBe("control");
    expect(normalize("Baldur's Gate 3")).toBe("baldur s gate 3");
    expect(normalize("Cyberpunk 2077")).toBe("cyberpunk 2077");
    expect(normalize("DEATH STRANDING™")).toBe(normalize("Death Stranding"));
    expect(normalize("Doom - Director's Cut")).toBe("doom");
    expect(normalize("Game of the Year Edition")).toBe("game of the year edition");
    expect(normalize("BioShock™ Remastered - Amazon Prime")).toBe("bioshock");
    expect(normalize("Hades - Amazon Prime")).toBe("hades");
    expect(normalize("Baldur's Gate Enhanced Edition")).toBe("baldur s gate");
    expect(normalize("BioShock 2")).toBe("bioshock 2");
  });
});

describe("view", () => {
  const a = game("Alpha", [entry("steam", { installed: true })]);
  const b = game("Beta", [entry("epic")]);
  const c = game("İstanbul", [entry("ubisoft", { installed: true })]);
  const all = [a, b, c];

  it("filters", () => {
    expect(view(all, "installed", "", "title")).toEqual([a, c]);
    expect(view(all, "epic", "", "title")).toEqual([b]);
    expect(view(all, "all", "", "title")).toHaveLength(3);
  });
  it("searches Turkish", () => {
    expect(view(all, "all", "ıstanbul", "title")).toEqual([c]);
    expect(view(all, "all", "ISTANBUL", "title")).toEqual([c]);
    expect(view(all, "all", "", "title")).toHaveLength(3);
  });
  it("sorts recent by max(last_played, last_launched), nulls last", () => {
    const x = game("X", [entry("steam", { last_played: 100 })]);
    const y = game("Y", [entry("steam", { last_played: 50 })], { last_launched: 200 });
    const z = game("Z", [entry("steam")]);
    const w = game("W", [entry("steam")]);
    expect(view([z, x, w, y], "all", "", "recent").map((g) => g.title)).toEqual(["Y", "X", "W", "Z"]);
  });
  it("sorts by playtime desc", () => {
    const p = game("P", [entry("steam", { playtime_min: 10 }), entry("epic", { playtime_min: 5 })]);
    const q = game("Q", [entry("steam", { playtime_min: 12 })]);
    const r = game("R", [entry("steam")]);
    expect(view([r, q, p], "all", "", "playtime").map((g) => g.title)).toEqual(["P", "Q", "R"]);
  });
});

describe("currentEntry / isInstalled / counts", () => {
  it("uses default_entry, else the first entry", () => {
    const g = game("a", [entry("steam", { platform_id: "1" }), entry("epic", { platform_id: "2" })]);
    expect(currentEntry(g).platform).toBe("steam");
    expect(currentEntry({ ...g, default_entry: ["epic", "2"] }).platform).toBe("epic");
    expect(currentEntry({ ...g, default_entry: ["epic", "zzz"] }).platform).toBe("steam");
  });
  it("isInstalled", () => {
    expect(isInstalled(game("a", [entry("steam"), entry("epic", { installed: true })]))).toBe(true);
    expect(isInstalled(game("a", [entry("steam")]))).toBe(false);
  });
  it("counts includes zero platforms", () => {
    const gs = [game("a", [entry("steam", { installed: true })]), game("b", [entry("steam"), entry("epic")])];
    expect(counts(gs)).toEqual({ all: 2, installed: 1, steam: 2, epic: 1, ubisoft: 0, gog: 0, riot: 0, local: 0 });
  });
});
