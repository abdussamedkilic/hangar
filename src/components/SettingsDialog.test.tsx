import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { vi } from "vitest";
import SettingsDialog from "./SettingsDialog";
import { api } from "../lib/api";
import { open } from "@tauri-apps/plugin-dialog";

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn(), ask: vi.fn() }));
vi.mock("../lib/api", () => ({ api: { getUiPrefs: vi.fn(), getSteamAdvanced: vi.fn(), getGogSource: vi.fn(), setUiPref: vi.fn(), openThemeFile: vi.fn(), listThemes: vi.fn(), setActiveTheme: vi.fn(), createTheme: vi.fn(), deleteTheme: vi.fn(), getScanFolders: vi.fn(), setScanFolders: vi.fn(), scanCandidates: vi.fn(), getAccounts: vi.fn() } }));

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(api.getScanFolders).mockResolvedValue([]);
  vi.mocked(api.setScanFolders).mockResolvedValue(undefined);
  vi.mocked(api.getAccounts).mockResolvedValue([]);
  vi.mocked(api.listThemes).mockResolvedValue({ active: "", names: [] });
  vi.mocked(api.setActiveTheme).mockResolvedValue(undefined);
  vi.mocked(api.deleteTheme).mockResolvedValue(undefined);
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
    expect(document.getElementById(p.getAttribute("aria-controls")!)).not.toBeNull(); // no dangling aria-controls
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
  expect(screen.getByRole("button", { name: "Dosyayı aç" })).toBeInTheDocument();
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

const openCustom = () => fireEvent.click(screen.getByRole("tab", { name: "Özelleştir" }));

test("shows active theme in select", async () => {
  vi.mocked(api.listThemes).mockResolvedValue({ active: "Gece", names: ["Gece", "Açık"] });
  render(<SettingsDialog sidebar="open" onClose={vi.fn()} />);
  openCustom();
  const sel = screen.getByLabelText("Tema") as HTMLSelectElement;
  await waitFor(() => expect(sel.value).toBe("Gece"));
  expect([...sel.options].map((o) => o.textContent)).toEqual(["Varsayılan", "Gece", "Açık"]);
});

test("switching theme calls api and onThemeChanged", async () => {
  vi.mocked(api.listThemes).mockResolvedValue({ active: "Gece", names: ["Gece"] });
  const onThemeChanged = vi.fn().mockResolvedValue(undefined);
  render(<SettingsDialog sidebar="open" onThemeChanged={onThemeChanged} onClose={vi.fn()} />);
  openCustom();
  const sel = screen.getByLabelText("Tema") as HTMLSelectElement;
  await waitFor(() => expect(sel.value).toBe("Gece"));
  fireEvent.change(sel, { target: { value: "" } });
  await waitFor(() => expect(onThemeChanged).toHaveBeenCalledTimes(1));
  expect(api.setActiveTheme).toHaveBeenCalledWith("");
});

test("new theme saves and selects", async () => {
  vi.mocked(api.createTheme).mockResolvedValue("Mor");
  render(<SettingsDialog sidebar="open" onClose={vi.fn()} />);
  openCustom();
  fireEvent.click(screen.getByRole("button", { name: "Yeni tema" }));
  fireEvent.change(screen.getByLabelText("Tema adı"), { target: { value: "Mor" } });
  vi.mocked(api.listThemes).mockResolvedValue({ active: "Mor", names: ["Mor"] });
  fireEvent.click(screen.getByRole("button", { name: "Kaydet" }));
  await waitFor(() => expect(api.createTheme).toHaveBeenCalledWith("Mor"));
  await waitFor(() => expect((screen.getByLabelText("Tema") as HTMLSelectElement).value).toBe("Mor"));
  expect(screen.queryByLabelText("Tema adı")).toBeNull();
});

test("create error is shown", async () => {
  vi.mocked(api.createTheme).mockRejectedValue("Bu adda bir tema zaten var");
  render(<SettingsDialog sidebar="open" onClose={vi.fn()} />);
  openCustom();
  fireEvent.click(screen.getByRole("button", { name: "Yeni tema" }));
  fireEvent.change(screen.getByLabelText("Tema adı"), { target: { value: "Gece" } });
  fireEvent.click(screen.getByRole("button", { name: "Kaydet" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("Bu adda bir tema zaten var");
  expect(screen.getByLabelText("Tema adı")).toBeInTheDocument();
});

test("Escape in the name input cancels without closing the dialog", () => {
  const onClose = vi.fn();
  render(<SettingsDialog sidebar="open" onClose={onClose} />);
  openCustom();
  fireEvent.click(screen.getByRole("button", { name: "Yeni tema" }));
  fireEvent.keyDown(screen.getByLabelText("Tema adı"), { key: "Escape" });
  expect(screen.queryByLabelText("Tema adı")).toBeNull();
  expect(onClose).not.toHaveBeenCalled();
});

test("default theme disables open and delete", () => {
  render(<SettingsDialog sidebar="open" onClose={vi.fn()} />);
  openCustom();
  expect(screen.getByRole("button", { name: "Dosyayı aç" })).toBeDisabled();
  expect(screen.getByRole("button", { name: "Sil" })).toBeDisabled();
});

test("delete asks and switches", async () => {
  vi.mocked(api.listThemes).mockResolvedValue({ active: "Gece", names: ["Gece"] });
  vi.spyOn(window, "confirm").mockResolvedValue(true as never);
  const onThemeChanged = vi.fn().mockResolvedValue(undefined);
  render(<SettingsDialog sidebar="open" onThemeChanged={onThemeChanged} onClose={vi.fn()} />);
  openCustom();
  await waitFor(() => expect(screen.getByRole("button", { name: "Sil" })).toBeEnabled());
  fireEvent.click(screen.getByRole("button", { name: "Sil" }));
  await waitFor(() => expect(api.deleteTheme).toHaveBeenCalledWith("Gece"));
  await waitFor(() => expect(onThemeChanged).toHaveBeenCalled());
});

test("stale theme error clears after a later successful action", async () => {
  vi.mocked(api.listThemes).mockResolvedValue({ active: "Gece", names: ["Gece"] });
  vi.mocked(api.createTheme).mockRejectedValueOnce("Bu adda bir tema zaten var");
  render(<SettingsDialog sidebar="open" onClose={vi.fn()} />);
  openCustom();
  fireEvent.click(screen.getByRole("button", { name: "Yeni tema" }));
  fireEvent.change(screen.getByLabelText("Tema adı"), { target: { value: "Gece" } });
  fireEvent.click(screen.getByRole("button", { name: "Kaydet" }));
  expect(await screen.findByRole("alert")).toBeInTheDocument();
  fireEvent.change(screen.getByLabelText("Tema"), { target: { value: "" } });
  await waitFor(() => expect(screen.queryByRole("alert")).toBeNull());
});

test("delete does nothing when the async confirm resolves false", async () => {
  vi.mocked(api.listThemes).mockResolvedValue({ active: "Gece", names: ["Gece"] });
  const confirm = vi.spyOn(window, "confirm").mockResolvedValue(false as never);
  render(<SettingsDialog sidebar="open" onThemeChanged={vi.fn().mockResolvedValue(undefined)} onClose={vi.fn()} />);
  openCustom();
  await waitFor(() => expect(screen.getByRole("button", { name: "Sil" })).toBeEnabled());
  vi.mocked(api.deleteTheme).mockClear();
  fireEvent.click(screen.getByRole("button", { name: "Sil" }));
  await waitFor(() => expect(confirm).toHaveBeenCalled());
  await new Promise((r) => setTimeout(r, 20));
  expect(api.deleteTheme).not.toHaveBeenCalled();
});
