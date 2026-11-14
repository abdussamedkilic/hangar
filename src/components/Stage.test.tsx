import { render, screen, fireEvent } from "@testing-library/react";
import { vi } from "vitest";
import Stage from "./Stage";
import type { Game } from "../lib/api";

vi.mock("../lib/api", () => ({ api: {} }));
vi.mock("@tauri-apps/api/core", () => ({ convertFileSrc: (p: string) => p }));

const local: Game = {
  id: 1, title: "Benim Oyunum", norm_title: "benim oyunum", cover_path: null, preferred: null, default_entry: null, last_launched: null,
  entries: [{ platform: "local", platform_id: "1", title: "Benim Oyunum", installed: true, install_path: null, size_bytes: null, last_played: null, playtime_min: null, launch_uri: "u", install_uri: null }],
};

function remove(answer: boolean) {
  const onRemove = vi.fn();
  const confirm = vi.spyOn(window, "confirm").mockReturnValue(answer);
  render(<Stage game={local} onLaunch={vi.fn()} onChoose={vi.fn()} onEdit={vi.fn()} onRemove={onRemove} />);
  fireEvent.click(screen.getByRole("button", { name: "Diğer işlemler" }));
  fireEvent.click(screen.getByRole("menuitem", { name: "Kaldır" }));
  return { onRemove, confirm };
}

afterEach(() => vi.restoreAllMocks());

test("Kaldır asks first and removes on confirm", () => {
  const { onRemove, confirm } = remove(true);
  expect(confirm).toHaveBeenCalledWith("“Benim Oyunum” Hangar'dan kaldırılsın mı? Oyun dosyaları silinmez.");
  expect(onRemove).toHaveBeenCalledWith(local);
});

test("Kaldır does nothing when the confirm is cancelled", () => {
  const { onRemove } = remove(false);
  expect(onRemove).not.toHaveBeenCalled();
});
