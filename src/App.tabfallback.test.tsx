import { render } from "@testing-library/react";
import { vi } from "vitest";
import App from "./App";

const h = vi.hoisted(() => ({ setTab: vi.fn(), games: [] as unknown[], installedOnly: false }));

vi.mock("./lib/api", () => ({ api: { getSettings: vi.fn().mockResolvedValue({ steam_api_key: "k", steam_id: null }), getScanFolders: vi.fn().mockResolvedValue([]) } }));
vi.mock("./state/useLibrary", () => ({
  useLibrary: () => ({
    games: h.games, selectedId: null, select: vi.fn(), tab: { kind: "platform", platform: "ubisoft" }, setTab: h.setTab, installedOnly: h.installedOnly, setInstalledOnly: vi.fn(),
    collections: [], reloadCollections: vi.fn(), query: "", setQuery: vi.fn(), sort: "recent", setSort: vi.fn(), warnings: [], rescanning: false, loaded: true, rescan: vi.fn(), applyReport: vi.fn(),
  }),
}));

const ubi = (installed: boolean) => ({
  id: 1, title: "AC", norm_title: "ac", cover_path: null, preferred: null, default_entry: null, last_launched: null,
  entries: [{ platform: "ubisoft", platform_id: "1", title: "AC", installed, install_path: null, size_bytes: null, last_played: null, playtime_min: null, launch_uri: "u", install_uri: null }],
});

beforeEach(() => h.setTab.mockClear());

test("Kurulu filter does not reset a store tab that only has uninstalled games", () => {
  h.games = [ubi(false)];
  h.installedOnly = true;
  render(<App />);
  expect(h.setTab).not.toHaveBeenCalled();
});

test("empty games (failed startup load) never reset the tab", () => {
  h.games = [];
  h.installedOnly = false;
  render(<App />);
  expect(h.setTab).not.toHaveBeenCalled();
});

test("a store tab with no games at all falls back to Tümü", () => {
  h.games = [{ ...ubi(true), entries: [{ ...ubi(true).entries[0], platform: "steam" }] }];
  h.installedOnly = false;
  render(<App />);
  expect(h.setTab).toHaveBeenCalledWith({ kind: "all" });
});
