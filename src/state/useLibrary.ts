import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, type Collection, type Game } from "../lib/api";
import { parseTab, tabKey, view, type Sort, type Tab } from "../lib/library";


/** `onUpdated` runs on every `library-updated` payload (whose games already carry fresh preferences). */
export function useLibrary(onUpdated?: () => void) {
  const [games, setGames] = useState<Game[]>([]);
  const [selectedId, select] = useState<number | null>(null);
  const [tab, setTabState] = useState<Tab>({ kind: "all" });
  const [installedOnly, setInstalledOnlyState] = useState(false);
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

  const rescan = useCallback(async () => {
    setRescanning(true);
    try {
      const r = await api.rescan();
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
        if (await api.galaxyChangedSinceScan()) await rescan();
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
        await rescan();
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
  const setInstalledOnly = useCallback((b: boolean) => {
    setInstalledOnlyState(b);
    api.setUiPref("ui.installed", b ? "1" : "0").catch(() => {});
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
      let inst = false;
      let vm: "grid" | "list" = "grid";
      let cols: Collection[] = [];
      try {
        cols = await api.getCollections();
        const p = await api.getUiPrefs();
        t = parseTab(p.tab, cols);
        inst = p.installed;
        vm = p.view === "list" ? "list" : "grid";
      } catch {
        /* prefs are best effort */
      }
      if (!alive) return;
      setCollections(cols);
      setTabState(t);
      setInstalledOnlyState(inst);
      setViewModeState(vm);
      try {
        const initial = await api.getLibrary();
        if (!alive) return;
        setGames(initial);
        const c = current.current;
        if (c.selectedId === null) select(view(initial, t, inst, cols, c.query, c.sort)[0]?.id ?? null);
      } catch (e) {
        setWarnings([String(e)]);
      }
      if (alive) setLoaded(true);
      await rescan();
    })();
    return () => {
      alive = false;
      mounted.current = false;
      unlisten.forEach((u) => u());
    };
  }, [rescan]);

  return { games, selectedId, select, tab, setTab, installedOnly, setInstalledOnly, viewMode, setViewMode, collections, reloadCollections, query, setQuery, sort, setSort, warnings, rescanning, loaded, rescan, rescanLabeled, scanLabel, applyReport };
}
