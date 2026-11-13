import { render, screen, fireEvent } from "@testing-library/react";
import { vi } from "vitest";
import PlayButton from "./PlayButton";
import type { Game, Platform, PlatformEntry } from "../lib/api";

vi.mock("../lib/api", () => ({ api: {} }));

const entry = (platform: Platform, installed: boolean, o: Partial<PlatformEntry> = {}): PlatformEntry => ({
  platform, platform_id: "1", title: "X", installed, install_path: null, size_bytes: null,
  last_played: null, playtime_min: null, launch_uri: "u", install_uri: null, ...o,
});
const game = (entries: Game["entries"], default_entry: Game["default_entry"] = null): Game => ({
  id: 1, title: "X", norm_title: "x", cover_path: null, preferred: null, default_entry, last_launched: null, entries,
});
const open = () => fireEvent.click(screen.getByRole("button", { name: "Sürüm ve platform seç" }));

const bio = () => game([
  entry("steam", false, { platform_id: "409710" }),
  entry("epic", false, { platform_id: "e1" }),
  entry("gog", false, { platform_id: "1207658" }),
  entry("gog", false, { platform_id: "1157510446", copy: "Prime", title: "BioShock Remastered - Amazon Prime" }),
]);

test("installed multi-platform shows Oyna · Steam", () => {
  render(<PlayButton game={game([entry("steam", true), entry("epic", true)])} onLaunch={vi.fn()} onChoose={vi.fn()} />);
  expect(screen.getByRole("button", { name: "Oyna · Steam" })).toBeInTheDocument();
});

test("menu choice calls onChoose with the entry key", () => {
  const onChoose = vi.fn();
  render(<PlayButton game={game([entry("steam", true), entry("epic", true, { platform_id: "9" })])} onLaunch={vi.fn()} onChoose={onChoose} />);
  open();
  fireEvent.click(screen.getByRole("menuitemradio", { name: /Epic/ }));
  expect(onChoose).toHaveBeenCalledWith(["epic", "9"]);
});

test("menu checks the default_entry row by platform and id", () => {
  render(<PlayButton game={game([entry("steam", true), entry("epic", true)], ["epic", "1"])} onLaunch={vi.fn()} onChoose={vi.fn()} />);
  open();
  expect(screen.getByRole("menuitemradio", { name: /Epic/ })).toHaveAttribute("aria-checked", "true");
  expect(screen.getByRole("menuitemradio", { name: /Steam/ })).toHaveAttribute("aria-checked", "false");
});

test("not installed shows Kur · Steam and launches with key", () => {
  const onLaunch = vi.fn();
  render(<PlayButton game={game([entry("steam", false, { platform_id: "7" })])} onLaunch={onLaunch} onChoose={vi.fn()} />);
  fireEvent.click(screen.getByRole("button", { name: "Kur · Steam" }));
  expect(onLaunch).toHaveBeenCalledWith(["steam", "7"]);
});

test("single platform has no dropdown", () => {
  render(<PlayButton game={game([entry("steam", true)])} onLaunch={vi.fn()} onChoose={vi.fn()} />);
  expect(screen.queryByRole("button", { name: "Sürüm ve platform seç" })).toBeNull();
});

test("BioShock Remastered rows tell the copies apart; choosing Prime passes its key", () => {
  const onChoose = vi.fn();
  render(<PlayButton game={bio()} onLaunch={vi.fn()} onChoose={onChoose} />);
  open();
  const rows = screen.getAllByRole("menuitemradio").map((r) => r.textContent!.replace(" ✓", ""));
  expect(rows).toEqual(["Kur · Steam", "Kur · Epic", "Kur · GOG", "Kur · GOG (Prime)"]);
  fireEvent.click(screen.getByRole("menuitemradio", { name: "Kur · GOG (Prime)" }));
  expect(onChoose).toHaveBeenCalledWith(["gog", "1157510446"]);
});

test("edition is shown only when the card mixes editions", () => {
  const g = game([entry("steam", true, { edition: "Remastered" }), entry("gog", true, { edition: null })]);
  render(<PlayButton game={g} onLaunch={vi.fn()} onChoose={vi.fn()} />);
  expect(screen.getByRole("button", { name: "Oyna · Steam · Remastered" })).toBeInTheDocument();
});

test("identical rows get the store title as a second line", () => {
  const g = game([entry("gog", true, { platform_id: "1", title: "A" }), entry("gog", true, { platform_id: "2", title: "B" })]);
  render(<PlayButton game={g} onLaunch={vi.fn()} onChoose={vi.fn()} />);
  open();
  expect(screen.getByText("A")).toBeInTheDocument();
  expect(screen.getByText("B")).toBeInTheDocument();
});

test("only the store part is marked lang=en, not the Turkish verb", () => {
  render(<PlayButton game={game([entry("steam", true)])} onLaunch={vi.fn()} onChoose={vi.fn()} />);
  const btn = screen.getByRole("button", { name: "Oyna · Steam" });
  expect(btn.querySelector("[lang='en']")?.textContent).toBe("Steam");
  expect(btn.querySelector("span")?.getAttribute("lang")).toBeNull();
});

test("identical copies are numbered and chosen by their own id", () => {
  const onChoose = vi.fn();
  render(<PlayButton game={game([
    entry("gog", false, { platform_id: "1", title: "Mafia II" }),
    entry("gog", false, { platform_id: "2", title: "Mafia II" }),
  ])} onLaunch={vi.fn()} onChoose={onChoose} />);
  open();
  expect(screen.getAllByRole("menuitemradio").map((b) => b.textContent)).toEqual(expect.arrayContaining([expect.stringContaining("Kur · GOG · 2. kopya")]));
  fireEvent.click(screen.getByRole("menuitemradio", { name: /2\. kopya/ }));
  expect(onChoose).toHaveBeenCalledWith(["gog", "2"]);
});
