import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { vi } from "vitest";
import AddGameDialog from "./AddGameDialog";
import { api } from "../lib/api";
import { open } from "@tauri-apps/plugin-dialog";

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));
vi.mock("../lib/api", () => ({ api: { addLocalGame: vi.fn(), updateLocalGame: vi.fn() } }));

const report = { games: 1, warnings: [] };
beforeEach(() => vi.clearAllMocks());

test("choosing an exe pre-fills the name and Ekle adds the game", async () => {
  vi.mocked(open).mockResolvedValue("C:\\Games\\Hades\\Hades.exe");
  vi.mocked(api.addLocalGame).mockResolvedValue(report);
  const onDone = vi.fn(), onClose = vi.fn();
  render(<AddGameDialog onDone={onDone} onClose={onClose} />);
  fireEvent.click(screen.getByRole("button", { name: "Exe seç" }));
  await waitFor(() => expect(screen.getByLabelText("Ad")).toHaveValue("Hades"));
  fireEvent.click(screen.getByRole("button", { name: "Ekle" }));
  await waitFor(() => expect(onClose).toHaveBeenCalled());
  expect(open).toHaveBeenCalledWith({ filters: [{ name: "Oyun", extensions: ["exe"] }] });
  expect(api.addLocalGame).toHaveBeenCalledWith("Hades", "C:\\Games\\Hades\\Hades.exe");
  expect(onDone).toHaveBeenCalledWith(report);
});

test("Ekle is disabled while the name is empty", async () => {
  vi.mocked(open).mockResolvedValue("C:\\Games\\Hades\\Hades.exe");
  render(<AddGameDialog onDone={vi.fn()} onClose={vi.fn()} />);
  expect(screen.getByRole("button", { name: "Ekle" })).toBeDisabled();
  fireEvent.click(screen.getByRole("button", { name: "Exe seç" }));
  await waitFor(() => expect(screen.getByLabelText("Ad")).toHaveValue("Hades"));
  expect(screen.getByRole("button", { name: "Ekle" })).toBeEnabled();
  fireEvent.change(screen.getByLabelText("Ad"), { target: { value: "  " } });
  expect(screen.getByRole("button", { name: "Ekle" })).toBeDisabled();
});

test("edit mode is prefilled and calls update", async () => {
  vi.mocked(api.updateLocalGame).mockResolvedValue(report);
  const onClose = vi.fn();
  render(<AddGameDialog edit={{ id: 5, title: "Hades", exe: "C:\\H.exe" }} onDone={vi.fn()} onClose={onClose} />);
  expect(screen.getByLabelText("Ad")).toHaveValue("Hades");
  fireEvent.click(screen.getByRole("button", { name: "Kaydet" }));
  await waitFor(() => expect(onClose).toHaveBeenCalled());
  expect(api.updateLocalGame).toHaveBeenCalledWith(5, "Hades", "C:\\H.exe");
});
