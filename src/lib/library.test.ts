import { describe, expect, it } from "vitest";
import { currentEntry, installedCount, isInstalled, matchesRule, normalize, parseTab, tabCounts, tabKey, view, type Game, type Platform, type PlatformEntry, type Tab, type FilterRule, type SavedFilter, type ActiveFilter } from "./library";
import type { Collection } from "./api";

const NONE: ActiveFilter = { kind: "none" };
const INST: ActiveFilter = { kind: "installed" };
const ALL: Tab = { kind: "all" };
const plat = (platform: Platform): Tab => ({ kind: "platform", platform });

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
    expect(normalize("Warbreeds - Amazon Luna")).toBe(normalize("WarBreeds"));
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
    expect(view(all, ALL, INST, [], [], "", "title")).toEqual([a, c]);
    expect(view(all, plat("epic"), NONE, [], [], "", "title")).toEqual([b]);
    expect(view(all, ALL, NONE, [], [], "", "title")).toHaveLength(3);
  });
  it("searches Turkish", () => {
    expect(view(all, ALL, NONE, [], [], "ıstanbul", "title")).toEqual([c]);
    expect(view(all, ALL, NONE, [], [], "ISTANBUL", "title")).toEqual([c]);
    expect(view(all, ALL, NONE, [], [], "", "title")).toHaveLength(3);
  });
  it("sorts recent by max(last_played, last_launched), nulls last", () => {
    const x = game("X", [entry("steam", { last_played: 100 })]);
    const y = game("Y", [entry("steam", { last_played: 50 })], { last_launched: 200 });
    const z = game("Z", [entry("steam")]);
    const w = game("W", [entry("steam")]);
    expect(view([z, x, w, y], ALL, NONE, [], [], "", "recent").map((g) => g.title)).toEqual(["Y", "X", "W", "Z"]);
  });
  it("sorts by playtime desc", () => {
    const p = game("P", [entry("steam", { playtime_min: 10 }), entry("epic", { playtime_min: 5 })]);
    const q = game("Q", [entry("steam", { playtime_min: 12 })]);
    const r = game("R", [entry("steam")]);
    expect(view([r, q, p], ALL, NONE, [], [], "", "playtime").map((g) => g.title)).toEqual(["P", "Q", "R"]);
  });
});

describe("currentEntry / isInstalled", () => {
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
});

describe("tabs", () => {
  it("ubisoft tab + installed excludes steam-installed merged card", () => {
    const w = game("Watch_Dogs", [entry("steam", { installed: true }), entry("ubisoft")]);
    expect(view([w], plat("ubisoft"), INST, [], [], "", "title")).toEqual([]);
    expect(view([w], plat("steam"), INST, [], [], "", "title")).toHaveLength(1);
    expect(view([w], ALL, INST, [], [], "", "title")).toHaveLength(1);
  });
  it("merged card counts in both store tabs", () => {
    const c = tabCounts([game("W", [entry("steam"), entry("ubisoft")])], NONE, [], []);
    expect(c.platform.steam).toBe(1);
    expect(c.platform.ubisoft).toBe(1);
  });
  it("collection tab filters by any entry", () => {
    const m = game("M", [entry("steam", { platform_id: "1" }), entry("epic", { platform_id: "2" })]);
    const o = game("O", [entry("steam", { platform_id: "9" })]);
    const cols: Collection[] = [{ id: 5, name: "x", entries: [["epic", "2"]] }];
    expect(view([m, o], { kind: "collection", id: 5 }, NONE, [], cols, "", "title")).toEqual([m]);
  });
  it("parseTab unknown collection falls back to all", () => {
    expect(parseTab("c:7", [])).toEqual(ALL);
    expect(parseTab("p:xbox", [])).toEqual(ALL);
    expect(parseTab(null, [])).toEqual(ALL);
    expect(parseTab("p:gog", [])).toEqual(plat("gog"));
    expect(parseTab("c:7", [{ id: 7, name: "n", entries: [] }])).toEqual({ kind: "collection", id: 7 });
    expect(tabKey({ kind: "collection", id: 12 })).toBe("c:12");
    expect(tabKey(plat("steam"))).toBe("p:steam");
    expect(tabKey(ALL)).toBe("all");
  });
  it("tabCounts respects installedOnly", () => {
    const gs = [game("a", [entry("steam", { installed: true })]), game("b", [entry("steam"), entry("epic")])];
    const cols: Collection[] = [{ id: 1, name: "c", entries: [["steam", "1"]] }];
    const c = tabCounts(gs, INST, [], cols);
    expect(c.all).toBe(1);
    expect(c.platform).toEqual({ steam: 1, epic: 0, ubisoft: 0, gog: 0, riot: 0, local: 0 });
    expect(c.collection[1]).toBe(1);
    expect(installedCount(gs, ALL, cols)).toBe(1);
  });
  it("installedCount on a platform tab counts only that platform's install", () => {
    const wd = game("Watch_Dogs", [entry("steam", { platform_id: "s", installed: true }), entry("ubisoft", { platform_id: "u" })]);
    expect(installedCount([wd], plat("steam"), [])).toBe(1);
    expect(installedCount([wd], plat("ubisoft"), [])).toBe(0);
  });
  it("installedCount on a collection tab", () => {
    const a = game("a", [entry("steam", { platform_id: "1", installed: true })]);
    const b = game("b", [entry("steam", { platform_id: "2", installed: true })]);
    const c = game("c", [entry("steam", { platform_id: "3" })]);
    const cols: Collection[] = [{ id: 7, name: "k", entries: [["steam", "1"], ["steam", "3"]] }];
    expect(installedCount([a, b, c], { kind: "collection", id: 7 }, cols)).toBe(1);
  });
});

describe("matchesRule", () => {
  const m = (g: Game, r: FilterRule, t: Tab = ALL, cols: Collection[] = []) => matchesRule(g, r, t, cols);
  it("played rule counts steam playtime and hangar launches", () => {
    expect(m(game("A", [entry("steam", { playtime_min: 5 })]), { played: "yes" })).toBe(true);
    expect(m(game("B", [entry("epic")], { last_launched: 9 }), { played: "yes" })).toBe(true);
    expect(m(game("C", [entry("epic", { last_played: 3 })]), { played: "yes" })).toBe(true);
    const never = game("D", [entry("steam", { playtime_min: 0 })]);
    expect(m(never, { played: "yes" })).toBe(false);
    expect(m(never, { played: "no" })).toBe(true);
  });
  it("stores rule is OR", () => {
    const g = game("A", [entry("steam")]);
    expect(m(g, { stores: ["epic", "steam"] })).toBe(true);
    expect(m(g, { stores: ["epic", "gog"] })).toBe(false);
    expect(m(g, { stores: [] })).toBe(true);
  });
  it("rule with deleted collection ignores criterion", () => {
    const g = game("A", [entry("steam")]);
    expect(m(g, { collection: { id: 99, in: true } })).toBe(true);
    const cols: Collection[] = [{ id: 1, name: "X", entries: [["steam", "1"]] }];
    expect(m(g, { collection: { id: 1, in: true } }, ALL, cols)).toBe(true);
    expect(m(g, { collection: { id: 1, in: false } }, ALL, cols)).toBe(false);
  });
  it("edition special vs original", () => {
    const sp = game("A", [entry("steam", { edition: "Remastered" })]);
    const og = game("B", [entry("steam")]);
    expect(m(sp, { edition: "special" })).toBe(true);
    expect(m(og, { edition: "special" })).toBe(false);
    expect(m(og, { edition: "original" })).toBe(true);
    expect(m(sp, { edition: "original" })).toBe(false);
  });
  it("installed rule respects store tab", () => {
    const w = game("W", [entry("steam", { installed: true }), entry("ubisoft")]);
    expect(m(w, { installed: "yes" }, plat("ubisoft"))).toBe(false);
    expect(m(w, { installed: "yes" }, plat("steam"))).toBe(true);
    expect(m(w, { installed: "no" }, plat("ubisoft"))).toBe(true);
  });
  it("saved filter drives view/tabCounts; deleted id means none", () => {
    const a = game("Alpha", [entry("steam", { installed: true })]);
    const b = game("Beta", [entry("epic")]);
    const saved: SavedFilter[] = [{ id: 3, name: "E", rule: { stores: ["epic"] } }];
    expect(view([a, b], ALL, { kind: "saved", id: 3 }, saved, [], "", "title")).toEqual([b]);
    expect(view([a, b], ALL, { kind: "saved", id: 9 }, saved, [], "", "title")).toHaveLength(2);
    expect(tabCounts([a, b], { kind: "saved", id: 3 }, saved, []).all).toBe(1);
  });
});
