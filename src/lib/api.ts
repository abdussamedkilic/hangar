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
}

export interface Game {
  id: number;
  title: string;
  norm_title: string;
  cover_path: string | null;
  preferred: Platform | null;
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

export const api = {
  getLibrary: () => invoke<Game[]>("get_library"),
  rescan: () => invoke<ScanReport>("rescan"),
  launch: (gameId: number, platform?: Platform) => invoke<string>("launch", { gameId, platform }),
  setPreferred: (gameId: number, platform: Platform) => invoke<void>("set_preferred", { gameId, platform }),
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
  merge: (a: EntryKey, b: EntryKey) => invoke<ScanReport>("merge_games", { a, b }),
};
