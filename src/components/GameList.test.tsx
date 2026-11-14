import { render, screen, fireEvent } from "@testing-library/react";
import { vi } from "vitest";
import GameList from "./GameList";
import type { Game, Platform } from "../lib/api";

vi.mock("../lib/api", () => ({ api: {} }));
vi.mock("./Cover", () => ({ default: () => null }));
vi.mock("@tanstack/react-virtual", () => ({
  useVirtualizer: ({ count, estimateSize }: { count: number; estimateSize: () => number }) => ({
    getTotalSize: () => count * estimateSize(),
    getVirtualItems: () => Array.from({ length: count }, (_, index) => ({ index, key: index, start: index * estimateSize() })),
    measure: () => {},
  }),
}));

const entry = (platform: Platform, id: string, o: Partial<Game["entries"][number]> = {}) => ({
  platform, platform_id: id, title: "X", installed: true, install_path: null, size_bytes: null,
  last_played: null, playtime_min: null, launch_uri: "u", install_uri: null, ...o,
});
const game = (id: number, title: string, entries: Game["entries"]): Game => ({
  id, title, norm_title: title.toLowerCase(), cover_path: null, preferred: null, default_entry: null, last_launched: null, entries,
});

test("row shows title, badges (Steam first), installed state and summed playtime", () => {
  const g = game(1, "Hades", [entry("epic", "2", { playtime_min: 60 }), entry("steam", "1", { playtime_min: 660 })]);
  const { container } = render(<GameList games={[g]} selectedId={null} onSelect={vi.fn()} onContext={vi.fn()} onLaunch={vi.fn()} />);
  expect(screen.getByText("Hades")).toBeInTheDocument();
  expect(Array.from(container.querySelectorAll(".pg b")).map((b) => b.textContent)).toEqual(["STE", "EPI"]);
  expect(screen.getByText("Kurulu")).toBeInTheDocument();
  expect(screen.getByText("12 sa")).toBeInTheDocument();
});

test("zero playtime shows a dash and uninstalled shows Kurulu değil with Kur button", () => {
  const g = game(1, "Hades", [entry("steam", "1", { installed: false })]);
  render(<GameList games={[g]} selectedId={null} onSelect={vi.fn()} onContext={vi.fn()} onLaunch={vi.fn()} />);
  expect(screen.getByText("—")).toBeInTheDocument();
  expect(screen.getByText("Kurulu değil")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: /^Kur\b/ })).toBeInTheDocument();
});

test("installed game has an Oyna button that launches without selecting", () => {
  const g = game(1, "Hades", [entry("steam", "1")]);
  const onLaunch = vi.fn(), onSelect = vi.fn();
  render(<GameList games={[g]} selectedId={null} onSelect={onSelect} onContext={vi.fn()} onLaunch={onLaunch} />);
  fireEvent.click(screen.getByRole("button", { name: /^Oyna\b/ }));
  expect(onLaunch).toHaveBeenCalledWith(g);
  expect(onSelect).not.toHaveBeenCalled();
});

test("click selects and right-click opens the context menu", () => {
  const g = game(7, "Hades", [entry("steam", "1")]);
  const onSelect = vi.fn(), onContext = vi.fn();
  render(<GameList games={[g]} selectedId={null} onSelect={onSelect} onContext={onContext} onLaunch={vi.fn()} />);
  const row = screen.getByText("Hades").closest(".lrow")!;
  fireEvent.click(row);
  expect(onSelect).toHaveBeenCalledWith(7);
  fireEvent.contextMenu(row, { clientX: 10, clientY: 20 });
  expect(onContext).toHaveBeenCalledWith(g, 10, 20);
});

test("local entry with a missing exe has a disabled Exe bulunamadı button", () => {
  const g = game(1, "Mod", [entry("local", "1", { installed: false })]);
  const onLaunch = vi.fn();
  render(<GameList games={[g]} selectedId={null} onSelect={vi.fn()} onContext={vi.fn()} onLaunch={onLaunch} />);
  const b = screen.getByRole("button", { name: /^Exe bulunamadı/ });
  expect(b).toBeDisabled();
  fireEvent.click(b);
  expect(onLaunch).not.toHaveBeenCalled();
});
