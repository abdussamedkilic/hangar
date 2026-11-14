import { invoke } from "@tauri-apps/api/core";

export type Platform = "steam" | "epic" | "ubisoft" | "gog" | "riot" | "local";

export interface PlatformEntry {
  platform: Platform;
  platform_id: string;
  title: string;
  installed: boolean;
  install_path: string | null;
  size_bytes: number | null;
  last_played: number | null;
  playtime_min: number | null;
  launch_uri: string;
  install_uri: string | null;
  launch_args?: string[];
  edition?: string | null;
  copy?: string | null;
}

export interface Game {
  id: number;
  title: string;
  norm_title: string;
  cover_path: string | null;
  preferred: EntryKey | null;
  default_entry?: EntryKey | null;
  last_launched: number | null;
  entries: PlatformEntry[];
}

export interface ScanReport {
  games: number;
  warnings: string[];
}

export interface Settings {
  steam_api_key: string | null;
  steam_id: string | null;
}

export interface Candidate {
  title: string;
  exe_path: string;
}

export type EntryKey = [Platform, string];
export interface Collection { id: number; name: string; entries: EntryKey[] }
export interface ThemeRead { colors: Record<string, string>; warnings: string[] }
export interface UiPrefs { view: string; tab: string; installed: boolean }

export interface AccountStatus {
  platform: Platform;
  state: "disconnected" | "connected" | "expired";
  display_name: string | null;
}

export const api = {
  getLibrary: () => invoke<Game[]>("get_library"),
  rescan: () => invoke<ScanReport>("rescan"),
  galaxyChangedSinceScan: () => invoke<boolean>("galaxy_changed_since_scan"),
  launch: (gameId: number, platform?: Platform, platformId?: string) => invoke<string>("launch", { gameId, platform, platformId }),
  setPreferred: (gameId: number, platform: Platform, platformId?: string) => invoke<void>("set_preferred", { gameId, platform, platformId }),
  setCover: (gameId: number, path: string) => invoke<string>("set_cover", { gameId, path }),
  getSettings: () => invoke<Settings>("get_settings"),
  saveSettings: (s: Settings) => invoke<void>("save_settings", { steamApiKey: s.steam_api_key, steamId: s.steam_id }),
  split: (a: EntryKey, b: EntryKey) => invoke<ScanReport>("split_game", { a, b }),
  addLocalGame: (title: string, exePath: string) => invoke<ScanReport>("add_local_game", { title, exePath }),
  updateLocalGame: (id: number, title: string, exePath: string) => invoke<ScanReport>("update_local_game", { id, title, exePath }),
  removeLocalGame: (id: number) => invoke<ScanReport>("remove_local_game", { id }),
  getScanFolders: () => invoke<string[]>("get_scan_folders"),
  setScanFolders: (folders: string[]) => invoke<void>("set_scan_folders", { folders }),
  scanCandidates: () => invoke<Candidate[]>("scan_candidates"),
  addLocalGames: (items: Candidate[]) => invoke<ScanReport>("add_local_games", { items }),
  getAccounts: () => invoke<AccountStatus[]>("get_accounts"),
  connectAccount: (platform: Platform) => invoke<AccountStatus>("connect_account", { platform }),
  disconnectAccount: (platform: Platform) => invoke<boolean>("disconnect_account", { platform }),
  merge: (a: EntryKey, b: EntryKey) => invoke<ScanReport>("merge_games", { a, b }),
  getCollections: () => invoke<Collection[]>("get_collections"),
  createCollection: (name: string) => invoke<number>("create_collection", { name }),
  renameCollection: (id: number, name: string) => invoke<void>("rename_collection", { id, name }),
  deleteCollection: (id: number) => invoke<void>("delete_collection", { id }),
  setInCollection: (id: number, entries: EntryKey[], member: boolean) => invoke<void>("set_in_collection", { id, entries, member }),
  getTheme: () => invoke<ThemeRead>("get_theme"),
  openThemeFile: () => invoke<void>("open_theme_file"),
  resetTheme: () => invoke<ThemeRead>("reset_theme"),
  getUiPrefs: () => invoke<UiPrefs>("get_ui_prefs"),
  setUiPref: (key: string, value: string) => invoke<void>("set_ui_pref", { key, value }),
};

if (import.meta.env.DEV) (window as unknown as { hangarProbe: () => Promise<string> }).hangarProbe = () => invoke<string>("debug_epic_probe");
if (import.meta.env.DEV) (window as unknown as { hangarUbiProbe: () => Promise<string> }).hangarUbiProbe = () => invoke<string>("debug_ubisoft_probe");
if (import.meta.env.DEV) (window as unknown as { hangarUbiQuery: (body: string) => Promise<string> }).hangarUbiQuery = (body) => invoke<string>("debug_ubisoft_graphql", { body });
if (import.meta.env.DEV) (window as unknown as { hangarUbiDemux: () => Promise<string> }).hangarUbiDemux = () => invoke<string>("debug_ubisoft_demux");
