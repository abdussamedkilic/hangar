import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, type Collection, type Game } from "../lib/api";
import { NONE, PLATFORMS, parseTab, tabKey, view, type ActiveFilter, type FilterRule, type Platform, type SavedFilter, type Sort, type Tab } from "../lib/library";

function parseRule(json: string): FilterRule {
  let raw: unknown;
  try {
    raw = JSON.parse(json);
  } catch {
    return {};
  }
  if (!raw || typeof raw !== "object" || Array.isArray(raw)) return {};
  const o = raw as Record<string, unknown>;
  const r: FilterRule = {};
  if (Array.isArray(o.stores)) r.stores = o.stores.filter((p): p is Platform => PLATFORMS.includes(p as Platform));
  if (o.installed === "yes" || o.installed === "no") r.installed = o.installed;
  if (o.played === "yes" || o.played === "no") r.played = o.played;
  if (o.edition === "original" || o.edition === "special") r.edition = o.edition;
  const c = o.collection as { id?: unknown; in?: unknown } | null;
  if (c && typeof c === "object" && typeof c.id === "number" && typeof c.in === "boolean") r.collection = { id: c.id, in: c.in };
  return r;
}

const toSaved = (fs: { id: number; name: string; rule: string }[]): SavedFilter[] =>
  fs.map((f) => ({ id: f.id, name: f.name, rule: parseRule(f.rule) }));

function filterKey(f: ActiveFilter): string {
  return f.kind === "none" ? "" : f.kind === "installed" ? "installed" : String(f.id);
}


/** `onUpdated` runs on every `library-updated` payload (whose games already carry fresh preferences). */
export function useLibrary(onUpdated?: () => void) {
  const [games, setGames] = useState<Game[]>([]);
  const [selectedId, select] = useState<number | null>(null);
  const [tab, setTabState] = useState<Tab>({ kind: "all" });
  const [filter, setFilterState] = useState<ActiveFilter>(NONE);
  const [savedFilters, setSavedFilters] = useState<SavedFilter[]>([]);
  const [sidebar, setSidebarState] = useState<"open" | "collapsed">("open");
  const [viewMode, setViewModeState] = useState<"grid" | "list">("grid");
  const [collections, setCollections] = useState<Collection[]>([]);
  const [query, setQuery] = useState("");
  const [sort, setSort] = useState<Sort>("recent");
  const [warnings, setWarnings] = useState<string[]>([]);
  const [rescanning, setRescanning] = useState(false);
  const [loaded, setLoaded] = useState(false);
  const mounted = useRef(true);
  const current = useRef({ query, sort, selectedId });
  current.current = { query, sort, selectedId };
  const updated = useRef(onUpdated);
  updated.current = onUpdated;

  const rescan = useCallback(async (force?: boolean) => {
    setRescanning(true);
    try {
      const r = await api.rescan(force !== false);
      if (mounted.current) setWarnings(r.warnings);
    } catch (e) {
      if (mounted.current) setWarnings([String(e)]);
    } finally {
      if (mounted.current) setRescanning(false);
    }
  }, []);

  // Window focus: pick up new GOG purchases (Galaxy DB newer than the last scan), at most once per 10 s.
  const busy = useRef(false);
  busy.current = rescanning;
  useEffect(() => {
    let last = 0;
    const onFocus = async () => {
      const now = Date.now();
      if (busy.current || now - last < 10_000) return;
      last = now;
      try {
        if (await api.galaxyChangedSinceScan()) await rescan(false);
      } catch {
        /* focus check is best effort */
      }
    };
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, [rescan]);

  const [scanLabel, setScanLabel] = useState<string | null>(null);
  const rescanLabeled = useCallback(
    async (label: string) => {
      setScanLabel(label);
      try {
        await rescan(false);
      } finally {
        if (mounted.current) setScanLabel(null);
      }
    },
    [rescan],
  );

  const setTab = useCallback((t: Tab) => {
    setTabState(t);
    api.setUiPref("ui.tab", tabKey(t)).catch(() => {});
  }, []);
  const setFilter = useCallback((f: ActiveFilter) => {
    setFilterState(f);
    api.setUiPref("ui.filter", filterKey(f)).catch(() => {});
  }, []);
  const setSidebar = useCallback((s: "open" | "collapsed") => {
    setSidebarState(s);
    api.setUiPref("ui.sidebar", s).catch(() => {});
  }, []);
  const reloadFilters = useCallback(async () => {
    try {
      const fs = toSaved(await api.getFilters());
      if (mounted.current) setSavedFilters(fs);
    } catch {
      /* keep the last list */
    }
  }, []);
  const setViewMode = useCallback((m: "grid" | "list") => {
    setViewModeState(m);
    api.setUiPref("ui.view", m).catch(() => {});
  }, []);
  const reloadCollections = useCallback(async () => {
    const cs = await api.getCollections();
    if (mounted.current) setCollections(cs);
  }, []);

  const applyReport = useCallback((r: { warnings: string[] }) => setWarnings(r.warnings), []);

  useEffect(() => {
    mounted.current = true;
    let alive = true;
    const unlisten: Array<() => void> = [];
    (async () => {
      const [u1, u2] = await Promise.all([
        listen<Game[]>("library-updated", (e) =>
          {
            if (!alive) return;
            updated.current?.();
            setGames((prev) => {
              const known = new Map(prev.map((g) => [g.id, g.cover_path]));
              return e.payload.map((g) => (g.cover_path == null && known.get(g.id) ? { ...g, cover_path: known.get(g.id)! } : g));
            });
          },
        ),
        listen<{ id: number; path: string }>("cover-ready", (e) =>
          alive && setGames((gs) => gs.map((g) => (g.id === e.payload.id ? { ...g, cover_path: e.payload.path } : g))),
        ),
      ]);
      unlisten.push(u1, u2);
      if (!alive) return unlisten.forEach((u) => u());
      let t: Tab = { kind: "all" };
      let f: ActiveFilter = NONE;
      let saved: SavedFilter[] = [];
      let vm: "grid" | "list" = "grid";
      let sb: "open" | "collapsed" = "open";
      let cols: Collection[] = [];
      try {
        cols = await api.getCollections();
        const p = await api.getUiPrefs();
        t = parseTab(p.tab, cols);
        saved = toSaved(await api.getFilters().catch(() => []));
        if (p.filter === "installed") f = { kind: "installed" };
        else if (p.filter) {
          const id = Number(p.filter);
          if (saved.some((s) => s.id === id)) f = { kind: "saved", id };
        } else if (p.installed) {
          f = { kind: "installed" };
          api.setUiPref("ui.filter", "installed").catch(() => {});
          api.setUiPref("ui.installed", "0").catch(() => {});
        }
        vm = p.view === "list" ? "list" : "grid";
        sb = p.sidebar === "collapsed" ? "collapsed" : "open";
      } catch {
        /* prefs are best effort */
      }
      if (!alive) return;
      setCollections(cols);
      setTabState(t);
      setFilterState(f);
      setSavedFilters(saved);
      setViewModeState(vm);
      setSidebarState(sb);
      try {
        const initial = await api.getLibrary();
        if (!alive) return;
        setGames(initial);
        const c = current.current;
        if (c.selectedId === null) select(view(initial, t, f, saved, cols, c.query, c.sort)[0]?.id ?? null);
      } catch (e) {
        setWarnings([String(e)]);
      }
      if (alive) setLoaded(true);
      await rescan(false);
    })();
    return () => {
      alive = false;
      mounted.current = false;
      unlisten.forEach((u) => u());
    };
  }, [rescan]);

  return { games, selectedId, select, tab, setTab, filter, setFilter, savedFilters, reloadFilters, sidebar, setSidebar, viewMode, setViewMode, collections, reloadCollections, query, setQuery, sort, setSort, warnings, rescanning, loaded, rescan, rescanLabeled, scanLabel, applyReport };
}
