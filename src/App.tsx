import { useCallback, useEffect, useMemo, useState } from "react";
import { api, type Game, type Platform, type Settings } from "./lib/api";
import { counts, view } from "./lib/library";
import { useLibrary } from "./state/useLibrary";
import Sidebar from "./components/Sidebar";
import Stage from "./components/Stage";
import SearchBar from "./components/SearchBar";
import GameGrid from "./components/GameGrid";
import SettingsDialog from "./components/SettingsDialog";
import MergePicker from "./components/MergePicker";
import ContextMenu from "./components/ContextMenu";

export default function App() {
  const lib = useLibrary();
  const { games, selectedId, select, filter, setFilter, query, setQuery, sort, setSort, warnings, rescanning, loaded, rescan } = lib;
  const [prefs, setPrefs] = useState<Record<number, Platform>>({});
  const [error, setError] = useState<string | null>(null);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [showSettings, setShowSettings] = useState(false);
  const [ctx, setCtx] = useState<{ game: Game; x: number; y: number } | null>(null);
  const [mergeFor, setMergeFor] = useState<Game | null>(null);

  const loadSettings = useCallback(() => {
    Promise.resolve()
      .then(() => api.getSettings())
      .then(setSettings)
      .catch(() => {});
  }, []);
  useEffect(loadSettings, [loadSettings]);
  const closeSettings = useCallback(() => setShowSettings(false), []);
  const closeCtx = useCallback(() => setCtx(null), []);
  const closeMerge = useCallback(() => setMergeFor(null), []);

  const visible = useMemo(() => view(games, filter, query, sort), [games, filter, query, sort]);
  const selected = visible.find((g) => g.id === selectedId) ?? visible[0] ?? null;
  useEffect(() => {
    if (selected && selected.id !== selectedId) select(selected.id);
  }, [selected, selectedId, select]);

  const game = selected && prefs[selected.id] ? { ...selected, preferred: prefs[selected.id] } : selected;
  const notice = [...(error ? [error] : []), ...warnings].join(" · ");

  const launch = (p: Platform) => {
    if (!game) return;
    setError(null);
    api.launch(game.id, p).catch((e) => setError(String(e)));
  };
  const choose = async (p: Platform) => {
    if (!game) return;
    try {
      await api.setPreferred(game.id, p);
      setPrefs((s) => ({ ...s, [game.id]: p }));
    } catch (e) {
      setError(String(e));
    }
  };

  const split = async (g: Game) => {
    setError(null);
    try {
      for (const e of g.entries.slice(1)) await api.split([g.entries[0].platform, g.entries[0].platform_id], [e.platform, e.platform_id]);
    } catch (e) {
      setError(String(e));
    }
  };
  const merge = async (a: Game, b: Game) => {
    setMergeFor(null);
    setError(null);
    try {
      await api.merge([a.entries[0].platform, a.entries[0].platform_id], [b.entries[0].platform, b.entries[0].platform_id]);
    } catch (e) {
      setError(String(e));
    }
  };
  const openSettings = () => {
    if (settings === null) loadSettings();
    setShowSettings(true);
  };

  const empty = loaded && games.length === 0 && !rescanning;
  const nudge = settings !== null && !settings.steam_api_key && filter !== "installed";

  return (
    <div className="app">
      <Sidebar filter={filter} counts={counts(games)} onFilter={setFilter} onSettings={openSettings} onRescan={rescan} rescanning={rescanning} />
      <main>
        {rescanning && <span className="scan" role="status">Taranıyor…</span>}
        <Stage game={game} onLaunch={launch} onChoose={choose} onSplit={split} />
        {notice && <p className="notice">{notice}</p>}
        <SearchBar query={query} onQuery={setQuery} sort={sort} onSort={setSort} />
        {empty ? (
          <div className="gempty">
            <p>Hiç oyun bulunamadı. Steam, Epic ya da Ubisoft Connect kurulu mu?</p>
            <button className="primary" onClick={() => void rescan()}>Yeniden tara</button>
          </div>
        ) : (
          <GameGrid
            games={visible}
            selectedId={game?.id ?? null}
            onSelect={select}
            nudge={nudge}
            onNudge={openSettings}
            onContext={(g, x, y) => setCtx({ game: g, x, y })}
          />
        )}
      </main>
      {showSettings && settings !== null && <SettingsDialog settings={settings} onSaved={async () => { loadSettings(); await rescan(); }} onClose={closeSettings} />}
      {ctx && <ContextMenu x={ctx.x} y={ctx.y} onClose={closeCtx} items={[{ label: "Şununla birleştir…", onSelect: () => setMergeFor(ctx.game) }]} />}
      {mergeFor && <MergePicker source={mergeFor} games={games} onPick={(g) => merge(mergeFor, g)} onClose={closeMerge} />}
    </div>
  );
}
