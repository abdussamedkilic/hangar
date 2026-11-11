import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { vi } from "vitest";
import SettingsDialog from "./SettingsDialog";
import { api } from "../lib/api";
import { open } from "@tauri-apps/plugin-dialog";

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));
vi.mock("../lib/api", () => ({ api: { saveSettings: vi.fn(), getScanFolders: vi.fn(), setScanFolders: vi.fn(), scanCandidates: vi.fn() } }));

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(api.getScanFolders).mockResolvedValue([]);
  vi.mocked(api.setScanFolders).mockResolvedValue(undefined);
});

test("saves entered values then rescans and closes", async () => {
  vi.mocked(api.saveSettings).mockResolvedValue(undefined);
  const onClose = vi.fn(), onSaved = vi.fn().mockResolvedValue(undefined);
  render(<SettingsDialog settings={{ steam_api_key: null, steam_id: "765" }} onSaved={onSaved} onClose={onClose} />);
  expect(screen.getByPlaceholderText("Otomatik bulunur")).toHaveValue("765");
  fireEvent.change(screen.getByLabelText(/Steam API anahtarı/), { target: { value: "KEY" } });
  fireEvent.click(screen.getByRole("button", { name: "Kaydet ve tara" }));
  await waitFor(() => expect(onClose).toHaveBeenCalled());
  expect(api.saveSettings).toHaveBeenCalledWith({ steam_api_key: "KEY", steam_id: "765" });
  expect(onSaved).toHaveBeenCalled();
});

test("shows Kaydediliyor… while pending and error inline on failure", async () => {
  let fail!: (e: string) => void;
  vi.mocked(api.saveSettings).mockReturnValue(new Promise((_, rej) => (fail = rej)));
  render(<SettingsDialog settings={{ steam_api_key: null, steam_id: null }} onSaved={vi.fn()} onClose={vi.fn()} />);
  fireEvent.click(screen.getByRole("button", { name: "Kaydet ve tara" }));
  expect(await screen.findByRole("button", { name: "Kaydediliyor…" })).toBeDisabled();
  fail("boom");
  expect(await screen.findByRole("alert")).toHaveTextContent("boom");
  expect(screen.getByRole("button", { name: "Kaydet ve tara" })).toBeEnabled();
});

test("adds and removes scan folders, persisting each change", async () => {
  vi.mocked(api.getScanFolders).mockResolvedValue(["D:\Games"]);
  vi.mocked(open).mockResolvedValue("E:\More");
  render(<SettingsDialog settings={{ steam_api_key: null, steam_id: null }} onSaved={vi.fn()} onClose={vi.fn()} />);
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
  render(<SettingsDialog settings={{ steam_api_key: null, steam_id: null }} onSaved={vi.fn()} onClose={onClose} />);
  await screen.findByText("D:\Games");
  fireEvent.click(screen.getByRole("button", { name: "Klasörleri tara" }));
  await screen.findByRole("dialog", { name: "Klasör taraması" });
  fireEvent.keyDown(document, { key: "Escape" });
  await waitFor(() => expect(screen.queryByRole("dialog", { name: "Klasör taraması" })).not.toBeInTheDocument());
  expect(screen.getByRole("dialog", { name: "Ayarlar" })).toBeInTheDocument();
  expect(onClose).not.toHaveBeenCalled();
});
