import type { Game, Platform } from "./api";
export type { Game, Platform, PlatformEntry } from "./api";

export type Filter = "all" | "installed" | Platform;
export type Sort = "recent" | "title" | "playtime";

const PLATFORMS: Platform[] = ["steam", "epic", "ubisoft", "gog", "riot", "local"];
const SUFFIXES = [
  "ultimate edition", "definitive edition", "game of the year edition", "goty",
  "deluxe edition", "complete edition", "remastered", "director's cut",
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

export function defaultPlatform(g: Game): Platform {
  if (g.preferred && g.entries.some((e) => e.platform === g.preferred)) return g.preferred;
  return PLATFORMS.find((p) => g.entries.some((e) => e.platform === p)) ?? g.entries[0].platform;
}

function lastActive(g: Game): number | null {
  const xs = [g.last_launched, ...g.entries.map((e) => e.last_played)].filter((x): x is number => x != null);
  return xs.length ? Math.max(...xs) : null;
}

const playtime = (g: Game) => g.entries.reduce((s, e) => s + (e.playtime_min ?? 0), 0);

const cmp = (x: string, y: string): number => (x < y ? -1 : x > y ? 1 : 0);

export function view(games: Game[], f: Filter, q: string, s: Sort): Game[] {
  const nq = normalize(q);
  const out = games
    .map((g) => ({ g, n: normalize(g.title) }))
    .filter(
      ({ g, n }) =>
        (f === "all" || (f === "installed" ? isInstalled(g) : g.entries.some((e) => e.platform === f))) && n.includes(nq),
    );
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

export function counts(games: Game[]): Record<Filter, number> {
  const c: Record<Filter, number> = { all: games.length, installed: games.filter(isInstalled).length, steam: 0, epic: 0, ubisoft: 0, gog: 0, riot: 0, local: 0 };
  for (const g of games) for (const p of PLATFORMS) if (g.entries.some((e) => e.platform === p)) c[p]++;
  return c;
}
