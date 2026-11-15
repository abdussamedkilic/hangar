import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { vi } from "vitest";
import App from "./App";
import { api } from "./lib/api";

vi.mock("./lib/api", () => ({
  api: {
    getSettings: vi.fn().mockResolvedValue({ steam_api_key: "k", steam_id: null }),
    getAccounts: vi.fn().mockResolvedValue([]),
    getUiPrefs: vi.fn().mockResolvedValue({ view: "grid", tab: "", installed: false, filter: "", sidebar: "open" }),
    deleteCollection: vi.fn().mockResolvedValue(undefined),
  },
}));
vi.mock("./components/TabStrip", () => ({
  default: ({ onTabContext, collections }: { onTabContext: (c: unknown, x: number, y: number) => void; collections: unknown[] }) => (
    <button onClick={() => onTabContext(collections[0], 0, 0)}>ctx</button>
  ),
}));
vi.mock("./state/useLibrary", () => {
  const lib = {
    games: [], selectedId: null, select: vi.fn(), tab: { kind: "all" }, setTab: vi.fn(), filter: { kind: "none" }, setFilter: vi.fn(), savedFilters: [], reloadFilters: vi.fn(), sidebar: "open", setSidebar: vi.fn(),
    collections: [{ id: 7, name: "Yeni", entries: [] }],
    reloadCollections: vi.fn(), query: "", setQuery: vi.fn(), sort: "recent", setSort: vi.fn(), warnings: [], rescanning: false, loaded: true, rescan: vi.fn(),
  };
  return { useLibrary: () => lib };
});

async function deleteViaMenu(answer: boolean) {
  // tauri-plugin-dialog replaces window.confirm with an async function
  const confirm = vi.fn().mockResolvedValue(answer);
  vi.stubGlobal("confirm", confirm);
  render(<App />);
  fireEvent.click(await screen.findByText("ctx"));
  fireEvent.click(await screen.findByText("Sil"));
  await waitFor(() => expect(confirm).toHaveBeenCalled());
}

afterEach(() => {
  vi.unstubAllGlobals();
  vi.clearAllMocks();
});

test("collection delete does nothing when the async confirm resolves false", async () => {
  await deleteViaMenu(false);
  await new Promise((r) => setTimeout(r, 20));
  expect(api.deleteCollection).not.toHaveBeenCalled();
});

test("collection delete runs when the async confirm resolves true", async () => {
  await deleteViaMenu(true);
  await waitFor(() => expect(api.deleteCollection).toHaveBeenCalledWith(7));
});
