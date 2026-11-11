import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { vi } from "vitest";
import SettingsDialog from "./SettingsDialog";
import { api } from "../lib/api";

vi.mock("../lib/api", () => ({ api: { saveSettings: vi.fn() } }));

beforeEach(() => vi.clearAllMocks());

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
