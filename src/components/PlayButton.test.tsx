import { render, screen, fireEvent } from "@testing-library/react";
import { vi } from "vitest";
import PlayButton from "./PlayButton";
import type { Game, Platform } from "../lib/api";

vi.mock("../lib/api", () => ({ api: {} }));

const entry = (platform: Platform, installed: boolean) => ({
  platform, platform_id: "1", title: "X", installed, install_path: null, size_bytes: null,
  last_played: null, playtime_min: null, launch_uri: "u", install_uri: null,
});
const game = (entries: Game["entries"], preferred: Platform | null = null): Game => ({
  id: 1, title: "X", norm_title: "x", cover_path: null, preferred, last_launched: null, entries,
});

test("installed multi-platform shows Oyna · Steam", () => {
  render(<PlayButton game={game([entry("steam", true), entry("epic", true)])} onLaunch={vi.fn()} onChoose={vi.fn()} />);
  expect(screen.getByRole("button", { name: "Oyna · Steam" })).toBeInTheDocument();
});

test("menu choice calls onChoose", () => {
  const onChoose = vi.fn();
  render(<PlayButton game={game([entry("steam", true), entry("epic", true)])} onLaunch={vi.fn()} onChoose={onChoose} />);
  fireEvent.click(screen.getByRole("button", { name: "Platform seç" }));
  fireEvent.click(screen.getByRole("menuitem", { name: /Epic/ }));
  expect(onChoose).toHaveBeenCalledWith("epic");
});

test("menu marks current preference", () => {
  render(<PlayButton game={game([entry("steam", true), entry("epic", true)], "epic")} onLaunch={vi.fn()} onChoose={vi.fn()} />);
  fireEvent.click(screen.getByRole("button", { name: "Platform seç" }));
  expect(screen.getByRole("menuitem", { name: /Epic ✓/ })).toBeInTheDocument();
});

test("not installed shows Kur · Steam and launches", () => {
  const onLaunch = vi.fn();
  render(<PlayButton game={game([entry("steam", false)])} onLaunch={onLaunch} onChoose={vi.fn()} />);
  fireEvent.click(screen.getByRole("button", { name: "Kur · Steam" }));
  expect(onLaunch).toHaveBeenCalledWith("steam");
});

test("single platform has no dropdown", () => {
  render(<PlayButton game={game([entry("steam", true)])} onLaunch={vi.fn()} onChoose={vi.fn()} />);
  expect(screen.queryByRole("button", { name: "Platform seç" })).toBeNull();
});
