import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { vi } from "vitest";
import App from "./App";
import { api } from "./lib/api";
import { open } from "@tauri-apps/plugin-dialog";

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));
vi.mock("./lib/api", () => ({
  api: {
    getSettings: vi.fn().mockResolvedValue({ steam_api_key: "k", steam_id: null }),
    getScanFolders: vi.fn().mockResolvedValue([]),
    listThemes: vi.fn().mockResolvedValue({ active: "", names: [] }),
    setCover: vi.fn(),
  },
}));
vi.mock("./components/GameGrid", () => ({
  default: ({ games, onContext }: { games: { id: number; title: string }[]; onContext: (g: unknown, x: number, y: number) => void }) => (
    <div>{games.map((g) => <button key={g.id} onContextMenu={() => onContext(g, 1, 1)}>{g.title}</button>)}</div>
  ),
}));
vi.mock("./state/useLibrary", () => ({
  useLibrary: () => ({
    games: [{ id: 7, title: "Hades", norm_title: "hades", cover_path: null, preferred: null, last_launched: null, entries: [{ platform: "local", platform_id: "1", title: "Hades", installed: true, install_path: null, size_bytes: null, last_played: null, playtime_min: null, launch_uri: "C:\\H.exe", install_uri: null }] }],
    selectedId: 7, select: vi.fn(), tab: { kind: "all" }, setTab: vi.fn(), filter: { kind: "none" }, setFilter: vi.fn(), savedFilters: [], reloadFilters: vi.fn(), collections: [], reloadCollections: vi.fn(), query: "", setQuery: vi.fn(),
    sort: "recent", setSort: vi.fn(), warnings: [], rescanning: false, loaded: true, rescan: vi.fn(), applyReport: vi.fn(),
  }),
}));

test("Kapak seç… picks an image and calls api.setCover", async () => {
  vi.mocked(open).mockResolvedValue("C:\\x\\a.png");
  vi.mocked(api.setCover).mockResolvedValue("p");
  render(<App />);
  fireEvent.contextMenu(screen.getByRole("button", { name: "Hades" }));
  fireEvent.click(screen.getByRole("menuitem", { name: "Kapak seç…" }));
  await waitFor(() => expect(api.setCover).toHaveBeenCalledWith(7, "C:\\x\\a.png"));
  expect(open).toHaveBeenCalledWith({ filters: [{ name: "Görsel", extensions: ["png", "jpg", "jpeg", "webp"] }] });
});
