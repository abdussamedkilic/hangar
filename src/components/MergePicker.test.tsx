import { render, screen } from "@testing-library/react";
import { vi } from "vitest";
import MergePicker from "./MergePicker";
import type { Game, Platform } from "../lib/api";

vi.mock("../lib/api", () => ({ api: {} }));

const g = (id: number, title: string, platforms: Platform[]): Game => ({
  id, title, norm_title: title.toLowerCase(), cover_path: null, preferred: null, default_entry: null, last_launched: null,
  entries: platforms.map((p, i) => ({
    platform: p, platform_id: `${id}-${i}`, title, installed: false, install_path: null, size_bytes: null,
    last_played: null, playtime_min: null, launch_uri: "u", install_uri: null,
  })),
});

test("candidates show full title and store badges", () => {
  render(<MergePicker source={g(1, "Src", ["steam"])} games={[g(1, "Src", ["steam"]), g(2, "Mafia III: Definitive Edition", ["gog", "steam", "gog"])]} onPick={vi.fn()} onClose={vi.fn()} />);
  expect(screen.getByText("Mafia III: Definitive Edition")).toBeInTheDocument();
  expect(screen.getByText("STE GOG")).toBeInTheDocument();
});
