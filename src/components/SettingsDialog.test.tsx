import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { vi } from "vitest";
import SettingsDialog from "./SettingsDialog";
import { api } from "../lib/api";
import { open } from "@tauri-apps/plugin-dialog";

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn(), ask: vi.fn() }));
vi.mock("../lib/api", () => ({ api: { getUiPrefs: vi.fn(), getSteamAdvanced: vi.fn(), getGogSource: vi.fn(), setUiPref: vi.fn(), openThemeFile: vi.fn(), getScanFolders: vi.fn(), setScanFolders: vi.fn(), scanCandidates: vi.fn(), getAccounts: vi.fn() } }));

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(api.getScanFolders).mockResolvedValue([]);
  vi.mocked(api.setScanFolders).mockResolvedValue(undefined);
  vi.mocked(api.getAccounts).mockResolvedValue([]);
  vi.mocked(api.getSteamAdvanced).mockResolvedValue({ api_key: null, steam_id: null });
  vi.mocked(api.getGogSource).mockResolvedValue("galaxy");
});

test("has two vertical tabs, Hesaplar selected by default with account rows", async () => {
  vi.mocked(api.getAccounts).mockResolvedValue([
    { platform: "steam", state: "disconnected", display_name: null },
    { platform: "epic", state: "disconnected", display_name: null },
    { platform: "gog", state: "disconnected", display_name: null },
    { platform: "ubisoft", state: "disconnected", display_name: null },
  ] as never);
  render(<SettingsDialog sidebar="open" onClose={vi.fn()} />);
  const tabs = screen.getAllByRole("tab");
  expect(tabs.map((t) => t.textContent)).toEqual(["Hesaplar", "Özelleştir"]);
  expect(screen.getByRole("tab", { name: "Hesaplar" })).toHaveAttribute("aria-selected", "true");
  expect(await screen.findByText("Steam")).toBeInTheDocument();
  expect(screen.getByText("Epic Games")).toBeInTheDocument();
  expect(screen.getByText("GOG")).toBeInTheDocument();
  expect(screen.getByText("Ubisoft Connect")).toBeInTheDocument();
  expect(screen.queryByLabelText(/Steam API anahtarı/)).toBeNull();
  expect(screen.getByText("Oyun klasörleri")).not.toBeVisible();
  for (const p of screen.getAllByRole("tab")) {
    expect(document.getElementById(p.getAttribute("aria-controls")!)).not.toBeNull(); // sarkan aria-controls yok
  }
});

test("arrow keys move between tabs", () => {
  render(<SettingsDialog sidebar="open" onClose={vi.fn()} />);
  fireEvent.keyDown(screen.getByRole("tab", { name: "Hesaplar" }), { key: "ArrowDown" });
  expect(screen.getByRole("tab", { name: "Özelleştir" })).toHaveAttribute("aria-selected", "true");
  expect(screen.getByRole("tabpanel")).toBeInTheDocument();
});

test("Özelleştir has theme, menu and folder sections; menu choice persists ui.sidebar", async () => {
  const onSidebar = vi.fn();
  render(<SettingsDialog sidebar="collapsed" onSidebar={onSidebar} onClose={vi.fn()} />);
  fireEvent.click(screen.getByRole("tab", { name: "Özelleştir" }));
  expect(screen.getByRole("button", { name: "Tema dosyasını aç" })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Varsayılana dön" })).toBeInTheDocument();
  expect(screen.getByText("Oyun klasörleri")).toBeInTheDocument();
  expect(screen.getByText("Açılışta menü")).toBeInTheDocument();
  expect(screen.getByRole("radio", { name: "Daraltılmış" })).toHaveAttribute("aria-checked", "true");
  fireEvent.click(screen.getByRole("radio", { name: "Açık" }));
  expect(onSidebar).toHaveBeenCalledWith("open");
});

test("Kapat closes the dialog", () => {
  const onClose = vi.fn();
  render(<SettingsDialog sidebar="open" onClose={onClose} />);
  fireEvent.click(screen.getByRole("button", { name: "Kapat" }));
  expect(onClose).toHaveBeenCalled();
});

test("adds and removes scan folders, persisting each change", async () => {
  vi.mocked(api.getScanFolders).mockResolvedValue(["D:\Games"]);
  vi.mocked(open).mockResolvedValue("E:\More");
  render(<SettingsDialog sidebar="open" onClose={vi.fn()} />);
  fireEvent.click(screen.getByRole("tab", { name: "Özelleştir" }));
  expect(await screen.findByText("D:\Games")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Klasör ekle" }));
  expect(await screen.findByText("E:\More")).toBeInTheDocument();
  expect(open).toHaveBeenCalledWith({ directory: true });
  expect(api.setScanFolders).toHaveBeenLastCalledWith(["D:\Games", "E:\More"]);
  fireEvent.click(screen.getAllByRole("button", { name: "Kaldır" })[0]);
  await waitFor(() => expect(screen.queryByText("D:\Games")).not.toBeInTheDocument());
  expect(api.setScanFolders).toHaveBeenLastCalledWith(["E:\More"]);
});

test("Escape in the scan dialog leaves Settings open", async () => {
  vi.mocked(api.getScanFolders).mockResolvedValue(["D:\Games"]);
  vi.mocked(api.scanCandidates).mockResolvedValue([]);
  const onClose = vi.fn();
  render(<SettingsDialog sidebar="open" onClose={onClose} />);
  fireEvent.click(screen.getByRole("tab", { name: "Özelleştir" }));
  await screen.findByText("D:\Games");
  fireEvent.click(screen.getByRole("button", { name: "Klasörleri tara" }));
  await screen.findByRole("dialog", { name: "Klasör taraması" });
  fireEvent.keyDown(document, { key: "Escape" });
  await waitFor(() => expect(screen.queryByRole("dialog", { name: "Klasör taraması" })).not.toBeInTheDocument());
  expect(screen.getByRole("dialog", { name: "Ayarlar" })).toBeInTheDocument();
  expect(onClose).not.toHaveBeenCalled();
});
