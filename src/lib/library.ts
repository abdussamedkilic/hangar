import type { Collection, Game, Platform, PlatformEntry } from "./api";
export type { Game, Platform, PlatformEntry } from "./api";

export type Tab = { kind: "all" } | { kind: "platform"; platform: Platform } | { kind: "collection"; id: number };
export type Sort = "recent" | "title" | "playtime";

const PLATFORMS: Platform[] = ["steam", "epic", "ubisoft", "gog", "riot", "local"];
const SUFFIXES = [
  "ultimate edition", "definitive edition", "game of the year edition", "goty",
  "deluxe edition", "complete edition", "remastered", "director's cut",
  "enhanced edition", "special edition", "gold edition", "anniversary edition", "amazon prime", "amazon luna",
];
const TR: Record<string, string> = { "ı": "i", "ş": "s", "ğ": "g", "ü": "u", "ö": "o", "ç": "c" };

function clean(s: string): string {
  let out = "";
  for (const c of s.toLowerCase()) {
    if (c === "̇" || c === "™" || c === "®" || c === "©") continue;
    if (TR[c]) out += TR[c];
    else if (/[\p{Alphabetic}\p{N}]/u.test(c)) out += c;
    else out += " ";
  }
  return out.split(/\s+/).filter(Boolean).join(" ");
}

const CLEAN_SUFFIXES = SUFFIXES.map(clean);

/** Mirrors Rust `merge::normalize`. */
export function normalize(title: string): string {
  let t = clean(title);
  for (;;) {
    const s = CLEAN_SUFFIXES.find((s) => t.endsWith(s) && t.slice(0, t.length - s.length).endsWith(" "));
    if (s === undefined) break;
    t = t.slice(0, t.length - s.length).trim();
  }
  return t;
}

export const isInstalled = (g: Game): boolean => g.entries.some((e) => e.installed);

export function currentEntry(g: Game): PlatformEntry {
  const d = g.default_entry;
  return (d && g.entries.find((e) => e.platform === d[0] && e.platform_id === d[1])) || g.entries[0];
}

function lastActive(g: Game): number | null {
  const xs = [g.last_launched, ...g.entries.map((e) => e.last_played)].filter((x): x is number => x != null);
  return xs.length ? Math.max(...xs) : null;
}

const playtime = (g: Game) => g.entries.reduce((s, e) => s + (e.playtime_min ?? 0), 0);

const cmp = (x: string, y: string): number => (x < y ? -1 : x > y ? 1 : 0);

export function tabKey(t: Tab): string {
  return t.kind === "all" ? "all" : t.kind === "platform" ? `p:${t.platform}` : `c:${t.id}`;
}

export function parseTab(s: string | null, cols: Collection[]): Tab {
  if (s?.startsWith("p:")) {
    const p = s.slice(2) as Platform;
    if (PLATFORMS.includes(p)) return { kind: "platform", platform: p };
  } else if (s?.startsWith("c:")) {
    const id = Number(s.slice(2));
    if (Number.isInteger(id) && cols.some((c) => c.id === id)) return { kind: "collection", id };
  }
  return { kind: "all" };
}

export function inCollection(g: Game, c: Collection): boolean {
  return g.entries.some((e) => c.entries.some(([p, id]) => p === e.platform && id === e.platform_id));
}

export function installedIn(g: Game, t: Tab): boolean {
  return t.kind === "platform" ? g.entries.some((e) => e.platform === t.platform && e.installed) : isInstalled(g);
}

function inTab(g: Game, t: Tab, cols: Collection[]): boolean {
  if (t.kind === "all") return true;
  if (t.kind === "platform") return g.entries.some((e) => e.platform === t.platform);
  const c = cols.find((c) => c.id === t.id);
  return c !== undefined && inCollection(g, c);
}

export function view(games: Game[], t: Tab, installedOnly: boolean, cols: Collection[], q: string, s: Sort): Game[] {
  const nq = normalize(q);
  const out = games
    .map((g) => ({ g, n: normalize(g.title) }))
    .filter(({ g, n }) => inTab(g, t, cols) && (!installedOnly || installedIn(g, t)) && n.includes(nq));
  const byTitle = (a: { n: string }, b: { n: string }) => cmp(a.n, b.n);
  out.sort((a, b) => {
    if (s === "title") return byTitle(a, b);
    if (s === "playtime") return playtime(b.g) - playtime(a.g) || byTitle(a, b);
    const x = lastActive(a.g), y = lastActive(b.g);
    if (x === y) return byTitle(a, b);
    if (x === null) return 1;
    if (y === null) return -1;
    return y - x;
  });
  return out.map(({ g }) => g);
}

export function tabCounts(games: Game[], installedOnly: boolean, cols: Collection[]) {
  const n = (t: Tab) => games.filter((g) => inTab(g, t, cols) && (!installedOnly || installedIn(g, t))).length;
  const platform = {} as Record<Platform, number>;
  for (const p of PLATFORMS) platform[p] = n({ kind: "platform", platform: p });
  const collection: Record<number, number> = {};
  for (const c of cols) collection[c.id] = n({ kind: "collection", id: c.id });
  return { all: n({ kind: "all" }), platform, collection };
}

export function installedCount(games: Game[], t: Tab, cols: Collection[]): number {
  return games.filter((g) => inTab(g, t, cols) && installedIn(g, t)).length;
}
