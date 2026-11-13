import { useCallback, useEffect, useMemo, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api, type EntryKey, type Game, type Settings } from "./lib/api";
import { counts, view } from "./lib/library";
import { useLibrary } from "./state/useLibrary";
import Sidebar from "./components/Sidebar";
import Stage from "./components/Stage";
import SearchBar from "./components/SearchBar";
import GameGrid from "./components/GameGrid";
import SettingsDialog from "./components/SettingsDialog";
import MergePicker from "./components/MergePicker";
import AddGameDialog from "./components/AddGameDialog";
import ContextMenu from "./components/ContextMenu";

export default function App() {
  const [prefs, setPrefs] = useState<Record<number, EntryKey>>({});
  const lib = useLibrary(useCallback(() => setPrefs({}), []));
  const { games, selectedId, select, filter, setFilter, query, setQuery, sort, setSort, warnings, rescanning, loaded, rescan, rescanLabeled, scanLabel, applyReport } = lib;
  const [error, setError] = useState<string | null>(null);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [showSettings, setShowSettings] = useState(false);
  const [ctx, setCtx] = useState<{ game: Game; x: number; y: number } | null>(null);
  const [mergeFor, setMergeFor] = useState<Game | null>(null);
  const [localDlg, setLocalDlg] = useState<{ id: number; title: string; exe: string } | "new" | null>(null);

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
  const closeLocal = useCallback(() => setLocalDlg(null), []);

  const visible = useMemo(() => view(games, filter, query, sort), [games, filter, query, sort]);
  const selected = visible.find((g) => g.id === selectedId) ?? visible[0] ?? null;
  useEffect(() => {
    if (selected && selected.id !== selectedId) select(selected.id);
  }, [selected, selectedId, select]);

  const game = selected && prefs[selected.id] ? { ...selected, preferred: prefs[selected.id], default_entry: prefs[selected.id] } : selected;
  const notice = [...(error ? [error] : []), ...warnings].join(" · ");

  const launch = ([p, id]: EntryKey) => {
    if (!game) return;
    setError(null);
    api.launch(game.id, p, id).catch((e) => setError(String(e)));
  };
  const choose = async (key: EntryKey) => {
    if (!game) return;
    try {
      await api.setPreferred(game.id, key[0], key[1]);
      setPrefs((s) => ({ ...s, [game.id]: key }));
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
  const editLocal = (g: Game) => {
    const e = g.entries.find((x) => x.platform === "local");
    if (e) setLocalDlg({ id: Number(e.platform_id), title: e.title, exe: e.launch_uri });
  };
  const removeLocal = async (g: Game) => {
    const e = g.entries.find((x) => x.platform === "local");
    if (!e) return;
    setError(null);
    try {
      applyReport(await api.removeLocalGame(Number(e.platform_id)));
    } catch (err) {
      setError(String(err));
    }
  };
  const pickCover = async (g: Game) => {
    setError(null);
    try {
      const f = await open({ filters: [{ name: "Görsel", extensions: ["png", "jpg", "jpeg", "webp"] }] });
      if (typeof f === "string") await api.setCover(g.id, f);
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
      <Sidebar filter={filter} counts={counts(games)} onFilter={setFilter} onSettings={openSettings} onRescan={rescan} onAdd={() => setLocalDlg("new")} rescanning={rescanning} />
      <main>
        {rescanning && <span className="scan" role="status">{scanLabel ?? "Taranıyor…"}</span>}
        <Stage game={game} onLaunch={launch} onChoose={choose} onSplit={split} onEdit={editLocal} onRemove={removeLocal} />
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
      {showSettings && settings !== null && <SettingsDialog settings={settings} onScanned={applyReport} onAccountsChanged={(l) => (l ? rescanLabeled(l) : rescan())} onSaved={async () => { loadSettings(); await rescan(); }} onClose={closeSettings} />}
      {localDlg && <AddGameDialog edit={localDlg === "new" ? undefined : localDlg} onDone={applyReport} onClose={closeLocal} />}
      {ctx && <ContextMenu x={ctx.x} y={ctx.y} onClose={closeCtx} items={[{ label: "Şununla birleştir…", onSelect: () => setMergeFor(ctx.game) }, { label: "Kapak seç…", onSelect: () => void pickCover(ctx.game) }]} />}
      {mergeFor && <MergePicker source={mergeFor} games={games} onPick={(g) => merge(mergeFor, g)} onClose={closeMerge} />}
    </div>
  );
}
