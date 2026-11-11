import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { vi } from "vitest";
import ScanDialog from "./ScanDialog";
import { api } from "../lib/api";

vi.mock("../lib/api", () => ({ api: { scanCandidates: vi.fn(), addLocalGames: vi.fn() } }));

const report = { games: 2, warnings: [] };
beforeEach(() => vi.clearAllMocks());

test("adds only the ticked candidates, with edited names", async () => {
  vi.mocked(api.scanCandidates).mockResolvedValue([
    { title: "Hades", exe_path: "C:\G\Hades\Hades.exe" },
    { title: "Celeste", exe_path: "C:\G\Celeste\Celeste.exe" },
  ]);
  vi.mocked(api.addLocalGames).mockResolvedValue(report);
  const onDone = vi.fn(), onClose = vi.fn();
  render(<ScanDialog onDone={onDone} onClose={onClose} />);
  const boxes = await screen.findAllByRole("checkbox");
  expect(boxes).toHaveLength(2);
  boxes.forEach((b) => expect(b).toBeChecked());
  fireEvent.click(boxes[1]);
  fireEvent.change(screen.getByDisplayValue("Hades"), { target: { value: "Hades II" } });
  fireEvent.click(screen.getByRole("button", { name: "Seçilenleri ekle" }));
  await waitFor(() => expect(onClose).toHaveBeenCalled());
  expect(api.addLocalGames).toHaveBeenCalledWith([{ title: "Hades II", exe_path: "C:\G\Hades\Hades.exe" }]);
  expect(onDone).toHaveBeenCalledWith(report);
});

test("shows the empty message when nothing is found", async () => {
  vi.mocked(api.scanCandidates).mockResolvedValue([]);
  render(<ScanDialog onDone={vi.fn()} onClose={vi.fn()} />);
  expect(await screen.findByText("Bulunamadı — Oyun ekle ile elle ekleyebilirsin.")).toBeInTheDocument();
});
