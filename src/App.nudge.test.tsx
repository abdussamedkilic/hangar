import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { vi } from "vitest";
import App from "./App";
import { api } from "./lib/api";

vi.mock("./lib/api", () => ({
  api: {
    getSettings: vi.fn(),
    getAccounts: vi.fn(),
    getUiPrefs: vi.fn().mockResolvedValue({ view: "grid", tab: "", installed: false, filter: "", sidebar: "open" }),
    getSteamAdvanced: vi.fn().mockResolvedValue({ api_key: null, steam_id: null }),
    getGogSource: vi.fn().mockResolvedValue("galaxy"),
    getScanFolders: vi.fn().mockResolvedValue([]),
    listThemes: vi.fn().mockResolvedValue({ active: "", names: [] }),
  },
}));
vi.mock("./components/GameGrid", () => ({
  default: ({ nudge, onNudge }: { nudge?: boolean; onNudge: () => void }) => (nudge ? <button onClick={onNudge}>Steam'e bağlan</button> : <div />),
}));
vi.mock("./state/useLibrary", () => {
  const game = { id: 1, title: "A", norm_title: "a", cover_path: null, preferred: null, last_launched: null, entries: [{ platform: "steam", platform_id: "1", title: "A", installed: true, launch_uri: "", last_played: null }] };
  const lib = {
    games: [game], selectedId: 1, select: vi.fn(), tab: { kind: "all" }, setTab: vi.fn(), filter: { kind: "none" }, setFilter: vi.fn(), savedFilters: [], reloadFilters: vi.fn(), sidebar: "open", setSidebar: vi.fn(),
    collections: [], reloadCollections: vi.fn(), query: "", setQuery: vi.fn(), sort: "recent", setSort: vi.fn(), warnings: [], rescanning: false, loaded: true, rescan: vi.fn(),
  };
  return { useLibrary: () => lib };
});

beforeEach(() => {
  vi.mocked(api.getSettings).mockResolvedValue({ steam_api_key: "", steam_id: null });
});

test("nudge shows when Steam is not connected and opens Settings", async () => {
  vi.mocked(api.getAccounts).mockResolvedValue([{ platform: "epic", state: "connected", display_name: null }]);
  render(<App />);
  fireEvent.click(await screen.findByRole("button", { name: "Steam'e bağlan" }));
  expect(await screen.findByRole("dialog", { name: "Ayarlar" })).toBeInTheDocument();
});

test("nudge is hidden when Steam is connected", async () => {
  vi.mocked(api.getAccounts).mockResolvedValue([{ platform: "steam", state: "connected", display_name: "x" }]);
  render(<App />);
  await screen.findByText("Hangar");
  await new Promise((r) => setTimeout(r, 20));
  expect(screen.queryByRole("button", { name: "Steam'e bağlan" })).toBeNull();
});

test("nudge goes away after an API key is saved in Settings", async () => {
  vi.mocked(api.getAccounts).mockResolvedValue([{ platform: "epic", state: "connected", display_name: null }]);
  render(<App />);
  fireEvent.click(await screen.findByRole("button", { name: "Steam'e bağlan" }));
  await screen.findByRole("dialog", { name: "Ayarlar" });
  vi.mocked(api.getSettings).mockResolvedValue({ steam_api_key: "KEY", steam_id: null });
  fireEvent.click(screen.getByRole("button", { name: "Kapat" }));
  await waitFor(() => expect(screen.queryByRole("button", { name: "Steam'e bağlan" })).toBeNull());
});
