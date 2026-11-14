import { render, screen, fireEvent } from "@testing-library/react";
import { vi } from "vitest";
import GameCard from "./GameCard";
import type { Game, Platform } from "../lib/api";

vi.mock("../lib/api", () => ({ api: {} }));
vi.mock("./Cover", () => ({ default: () => null }));

const entry = (platform: Platform, id: string, copy: string | null = null) => ({
  platform, platform_id: id, title: "X", installed: false, install_path: null, size_bytes: null,
  last_played: null, playtime_min: null, launch_uri: "u", install_uri: null, copy,
});
const card = (entries: Game["entries"]): Game => ({
  id: 1, title: "X", norm_title: "x", cover_path: null, preferred: null, default_entry: null, last_launched: null, entries,
});

test("two GOG copies show one GOG badge, tooltip lists both", () => {
  render(<GameCard game={card([entry("steam", "1"), entry("epic", "2"), entry("gog", "3"), entry("gog", "4", "Prime")])} selected={false} onSelect={vi.fn()} />);
  expect(screen.getAllByText("GOG")).toHaveLength(1);
  expect(screen.getByTitle("Steam, Epic, GOG, GOG (Prime)")).toBeInTheDocument();
});

test("five stores show three badges and +2", () => {
  render(<GameCard game={card([entry("local", "1"), entry("riot", "2"), entry("ubisoft", "3"), entry("gog", "4"), entry("epic", "5"), entry("steam", "6")].slice(1))} selected={false} onSelect={vi.fn()} />);
  expect(screen.getAllByText(/^(STE|EPI|GOG|UBI|RIO|LOC)$/)).toHaveLength(3);
  expect(screen.getByText("+2")).toBeInTheDocument();
});

test("badge span keeps title but has no aria-label", () => {
  const { container } = render(<GameCard game={card([entry("steam", "1"), entry("epic", "2")])} selected={false} onSelect={vi.fn()} />);
  const pg = container.querySelector(".pg")!;
  expect(pg).toHaveAttribute("title");
  expect(pg).not.toHaveAttribute("aria-label");
});

test("dragstart puts the game id in dataTransfer; click still selects", () => {
  const onSelect = vi.fn();
  render(<GameCard game={{ ...card([entry("steam", "1")]), id: 42 }} selected={false} onSelect={onSelect} />);
  const btn = screen.getByRole("button");
  expect(btn).toHaveAttribute("draggable", "true");
  const setData = vi.fn();
  fireEvent.dragStart(btn, { dataTransfer: { setData, effectAllowed: "" } });
  expect(setData).toHaveBeenCalledWith("application/x-hangar-game", "42");
  fireEvent.click(btn);
  expect(onSelect).toHaveBeenCalled();
});
