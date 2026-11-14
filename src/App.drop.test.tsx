import { render, screen, fireEvent } from "@testing-library/react";
import { vi } from "vitest";
import App from "./App";
import { api } from "./lib/api";

vi.mock("./lib/api", () => ({
  api: {
    getSettings: vi.fn().mockResolvedValue({ steam_api_key: "k", steam_id: null }),
    getAccounts: vi.fn().mockResolvedValue([]),
    getUiPrefs: vi.fn().mockResolvedValue({ view: "grid", tab: "", installed: false, filter: "", sidebar: "open" }),
    setInCollection: vi.fn().mockResolvedValue(undefined),
  },
}));
vi.mock("./components/TabStrip", () => ({
  default: ({ onDropGame }: { onDropGame: (c: number, g: number) => void }) => (
    <div>
      <button onClick={() => onDropGame(7, 1)}>drop-new</button>
      <button onClick={() => onDropGame(8, 1)}>drop-dup</button>
    </div>
  ),
}));
vi.mock("./state/useLibrary", () => {
  const game = { id: 1, title: "A", norm_title: "a", cover_path: null, preferred: null, last_launched: null, entries: [{ platform: "steam", platform_id: "1", title: "A", installed: true, launch_uri: "", last_played: null }] };
  const lib = {
    games: [game], selectedId: 1, select: vi.fn(), tab: { kind: "all" }, setTab: vi.fn(), filter: { kind: "none" }, setFilter: vi.fn(), savedFilters: [], reloadFilters: vi.fn(), sidebar: "open", setSidebar: vi.fn(),
    collections: [{ id: 7, name: "Yeni", entries: [] }, { id: 8, name: "Var", entries: [["steam", "1"]] }],
    reloadCollections: vi.fn(), query: "", setQuery: vi.fn(), sort: "recent", setSort: vi.fn(), warnings: [], rescanning: false, loaded: true, rescan: vi.fn(),
  };
  return { useLibrary: () => lib };
});

test("dropping a game reports added vs already in collection", async () => {
  render(<App />);
  fireEvent.click(await screen.findByText("drop-dup"));
  expect(await screen.findByText(/A zaten Var içinde/)).toBeInTheDocument();
  expect(api.setInCollection).not.toHaveBeenCalled();
  fireEvent.click(screen.getByText("drop-new"));
  expect(await screen.findByText(/A Yeni koleksiyonuna eklendi/)).toBeInTheDocument();
  expect(api.setInCollection).toHaveBeenCalledWith(7, [["steam", "1"]], true);
});
