import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, type Game } from "../lib/api";
import { view, type Filter, type Sort } from "../lib/library";

/** `onUpdated` runs on every `library-updated` payload (whose games already carry fresh preferences). */
export function useLibrary(onUpdated?: () => void) {
  const [games, setGames] = useState<Game[]>([]);
  const [selectedId, select] = useState<number | null>(null);
  const [filter, setFilter] = useState<Filter>("all");
  const [query, setQuery] = useState("");
  const [sort, setSort] = useState<Sort>("recent");
  const [warnings, setWarnings] = useState<string[]>([]);
  const [rescanning, setRescanning] = useState(false);
  const [loaded, setLoaded] = useState(false);
  const mounted = useRef(true);
  const current = useRef({ filter, query, sort, selectedId });
  current.current = { filter, query, sort, selectedId };
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
      try {
        const initial = await api.getLibrary();
        if (!alive) return;
        setGames(initial);
        const c = current.current;
        if (c.selectedId === null) select(view(initial, c.filter, c.query, c.sort)[0]?.id ?? null);
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

  return { games, selectedId, select, filter, setFilter, query, setQuery, sort, setSort, warnings, rescanning, loaded, rescan, rescanLabeled, scanLabel, applyReport };
}
