import { useCallback, useEffect, useMemo, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api, type Collection, type EntryKey, type Game, type Settings } from "./lib/api";
import { currentEntry, inCollection, installedCount, tabCounts, view } from "./lib/library";
import { useLibrary } from "./state/useLibrary";
import { useTheme } from "./state/useTheme";
import Sidebar from "./components/Sidebar";
import TabStrip from "./components/TabStrip";
import Stage from "./components/Stage";
import SearchBar from "./components/SearchBar";
import GameGrid from "./components/GameGrid";
import GameList from "./components/GameList";
import SettingsDialog from "./components/SettingsDialog";
import MergePicker from "./components/MergePicker";
import AddGameDialog from "./components/AddGameDialog";
import ContextMenu, { type MenuItem } from "./components/ContextMenu";

export default function App() {
  const [prefs, setPrefs] = useState<Record<number, EntryKey>>({});
  const lib = useLibrary(useCallback(() => setPrefs({}), []));
  const { games, selectedId, select, tab, setTab, installedOnly, setInstalledOnly, viewMode, setViewMode, collections, reloadCollections, query, setQuery, sort, setSort, warnings, rescanning, loaded, rescan, rescanLabeled, scanLabel, applyReport } = lib;
  const [error, setError] = useState<string | null>(null);
  const [themeWarnings, setThemeWarnings] = useState<string[]>([]);
  const theme = useTheme(setThemeWarnings);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [showSettings, setShowSettings] = useState(false);
  const [ctx, setCtx] = useState<{ game: Game; x: number; y: number } | null>(null);
  const [mergeFor, setMergeFor] = useState<Game | null>(null);
  const [tabCtx, setTabCtx] = useState<{ col: Collection; x: number; y: number } | null>(null);
  const [renamingId, setRenamingId] = useState<number | null>(null);
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
  const closeTabCtx = useCallback(() => setTabCtx(null), []);
  const cancelRename = useCallback(() => setRenamingId(null), []);

  const counts = useMemo(() => tabCounts(games, installedOnly, collections), [games, installedOnly, collections]);
  // Tab presence ignores "Kurulu" so toggling it never removes a store tab or resets ui.tab.
  const presence = useMemo(() => tabCounts(games, false, collections), [games, collections]);
  const instCount = useMemo(() => installedCount(games, tab, collections), [games, tab, collections]);
  const visible = useMemo(() => view(games, tab, installedOnly, collections, query, sort), [games, tab, installedOnly, collections, query, sort]);

  // A tab that vanished (no games / deleted collection) falls back to "Tümü".
  useEffect(() => {
    if (!loaded || games.length === 0) return;
    const gone =
      (tab.kind === "platform" && presence.platform[tab.platform] === 0) ||
      (tab.kind === "collection" && !collections.some((c) => c.id === tab.id));
    if (gone) setTab({ kind: "all" });
  }, [loaded, games.length, tab, presence, collections, setTab]);
  const selected = visible.find((g) => g.id === selectedId) ?? visible[0] ?? null;
  useEffect(() => {
    if (selected && selected.id !== selectedId) select(selected.id);
  }, [selected, selectedId, select]);

  const game = selected && prefs[selected.id] ? { ...selected, preferred: prefs[selected.id], default_entry: prefs[selected.id] } : selected;
  const notice = [...(error ? [error] : []), ...themeWarnings, ...warnings].join(" · ");

  const launch = ([p, id]: EntryKey) => {
    if (!game) return;
    setError(null);
    api.launch(game.id, p, id).catch((e) => setError(String(e)));
  };
  const launchGame = (g: Game) => {
    const e = currentEntry(prefs[g.id] ? { ...g, default_entry: prefs[g.id] } : g);
    setError(null);
    api.launch(g.id, e.platform, e.platform_id).catch((err) => setError(String(err)));
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
  const guard = async (f: () => Promise<void>) => {
    setError(null);
    try {
      await f();
    } catch (e) {
      setError(String(e));
    }
  };
  const createCollection = (name: string) =>
    guard(async () => {
      const id = await api.createCollection(name);
      await reloadCollections();
      setTab({ kind: "collection", id });
    });
  const renameCollection = (id: number, name: string) =>
    guard(async () => {
      setRenamingId(null);
      await api.renameCollection(id, name);
      await reloadCollections();
    });
  const deleteCollection = (c: Collection) => {
    if (!window.confirm(`“${c.name}” koleksiyonu silinsin mi? Oyunlar kütüphanede kalır.`)) return;
    void guard(async () => {
      await api.deleteCollection(c.id);
      if (tab.kind === "collection" && tab.id === c.id) setTab({ kind: "all" });
      await reloadCollections();
    });
  };
  const cardEntries = (g: Game): EntryKey[] => g.entries.map((e) => [e.platform, e.platform_id]);
  const cardMenu = (g: Game): MenuItem[] => [
    { label: "Şununla birleştir…", onSelect: () => setMergeFor(g) },
    { label: "Kapak seç…", onSelect: () => void pickCover(g) },
    {
      label: "Koleksiyona ekle",
      items: [
        ...collections.map((c) => {
          const checked = inCollection(g, c);
          return {
            label: c.name,
            checked,
            onSelect: () => void guard(async () => { await api.setInCollection(c.id, cardEntries(g), !checked); await reloadCollections(); }),
          };
        }),
        {
          label: "Yeni koleksiyon…",
          onSelect: () => {
            const name = window.prompt("Koleksiyon adı")?.trim();
            if (!name) return;
            void guard(async () => {
              const id = await api.createCollection(name);
              await api.setInCollection(id, cardEntries(g), true);
              await reloadCollections();
            });
          },
        },
      ],
    },
  ];
  const openSettings = () => {
    if (settings === null) loadSettings();
    setShowSettings(true);
  };

  const empty = loaded && games.length === 0 && !rescanning;
  const nudge = settings !== null && !settings.steam_api_key && !installedOnly;

  return (
    <div className="app">
      <Sidebar installedOnly={installedOnly} installedCount={instCount} onToggleInstalled={() => setInstalledOnly(!installedOnly)} onSettings={openSettings} onRescan={rescan} onAdd={() => setLocalDlg("new")} rescanning={rescanning} />
      <main>
        {rescanning && <span className="scan" role="status">{scanLabel ?? "Taranıyor…"}</span>}
        <Stage game={game} onLaunch={launch} onChoose={choose} onSplit={split} onEdit={editLocal} onRemove={removeLocal} />
        {notice && <p className="notice">{notice}</p>}
        <TabStrip
          presence={presence}
          tab={tab}
          counts={counts}
          collections={collections}
          installedOnly={installedOnly}
          renamingId={renamingId}
          onTab={setTab}
          onClearInstalled={() => setInstalledOnly(false)}
          onCreate={createCollection}
          onRename={renameCollection}
          onRenameCancel={cancelRename}
          onTabContext={(col, x, y) => setTabCtx({ col, x, y })}
          viewMode={viewMode}
          onViewMode={setViewMode}
        />
        <SearchBar query={query} onQuery={setQuery} sort={sort} onSort={setSort} />
        {empty ? (
          <div className="gempty">
            <p>Hiç oyun bulunamadı. Steam, Epic, GOG, Ubisoft ya da Riot kurulu mu? Elle eklemek için “Oyun ekle”yi kullan.</p>
            <button className="primary" onClick={() => void rescan()}>Yeniden tara</button>
          </div>
        ) : viewMode === "list" ? (
          <GameList games={visible} selectedId={game?.id ?? null} onSelect={select} onContext={(g, x, y) => setCtx({ game: g, x, y })} onLaunch={launchGame} />
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
      {showSettings && settings !== null && <SettingsDialog settings={settings} onResetTheme={theme.reset} onScanned={applyReport} onAccountsChanged={(l) => (l ? rescanLabeled(l) : rescan())} onSaved={async () => { loadSettings(); await rescan(); }} onClose={closeSettings} />}
      {localDlg && <AddGameDialog edit={localDlg === "new" ? undefined : localDlg} onDone={applyReport} onClose={closeLocal} />}
      {ctx && <ContextMenu x={ctx.x} y={ctx.y} onClose={closeCtx} items={cardMenu(ctx.game)} />}
      {tabCtx && <ContextMenu x={tabCtx.x} y={tabCtx.y} onClose={closeTabCtx} items={[{ label: "Yeniden adlandır", onSelect: () => setRenamingId(tabCtx.col.id) }, { label: "Sil", onSelect: () => deleteCollection(tabCtx.col) }]} />}
      {mergeFor && <MergePicker source={mergeFor} games={games} onPick={(g) => merge(mergeFor, g)} onClose={closeMerge} />}
    </div>
  );
}
