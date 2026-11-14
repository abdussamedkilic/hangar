import { render, screen, fireEvent, act } from "@testing-library/react";
import { vi } from "vitest";
import App from "./App";

const rescan = vi.hoisted(() => vi.fn());

vi.mock("./lib/api", () => ({ api: { getSettings: vi.fn().mockResolvedValue({ steam_api_key: "k", steam_id: null }), getScanFolders: vi.fn().mockResolvedValue([]), getUiPrefs: vi.fn().mockResolvedValue({ view: "grid", tab: "", installed: false, filter: "", sidebar: "open" }), getAccounts: vi.fn().mockResolvedValue([]), rescan: vi.fn().mockResolvedValue({ games: 0, warnings: [] }) } }));

vi.mock("./state/useLibrary", () => ({
  useLibrary: () => ({
    games: [], selectedId: null, select: vi.fn(), tab: { kind: "all" }, setTab: vi.fn(), filter: { kind: "none" }, setFilter: vi.fn(), savedFilters: [], reloadFilters: vi.fn(), sidebar: "open", setSidebar: vi.fn(), collections: [], reloadCollections: vi.fn(), query: "", setQuery: vi.fn(),
    sort: "recent", setSort: vi.fn(), warnings: [], rescanning: false, loaded: true, rescan,
  }),
}));

test("renders Hangar shell and empty state", () => {
  render(<App />);
  expect(screen.getByText("Hangar")).toBeInTheDocument();
  expect(screen.getByText("Bu filtreyle eşleşen oyun yok.")).toBeInTheDocument();
});

test("empty library shows message and Yeniden tara rescans", async () => {
  render(<App />);
  expect(screen.getByText(/Hiç oyun bulunamadı/)).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Yeniden tara" }));
  expect(rescan).toHaveBeenCalled();
});

test("Yenile sidebar button rescans", async () => {
  rescan.mockClear();
  render(<App />);
  await act(async () => {});
  fireEvent.click(screen.getByRole("button", { name: "Yenile" }));
  expect(rescan).toHaveBeenCalledWith(true);
});

test("Ayarlar opens even when getSettings failed; it retries on open", async () => {
  const { api } = await import("./lib/api");
  vi.mocked(api.getSettings).mockRejectedValueOnce("fail");
  render(<App />);
  fireEvent.click(screen.getByRole("button", { name: "Ayarlar" }));
  expect(await screen.findByRole("dialog", { name: "Ayarlar" })).toBeInTheDocument();
  expect(vi.mocked(api.getSettings).mock.calls.length).toBeGreaterThanOrEqual(2);
});
