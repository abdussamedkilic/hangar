import { render, screen, fireEvent, waitFor } from "@testing-library/react";
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
  const confirm = vi.spyOn(window, "confirm").mockResolvedValue(answer as never);
  render(<Stage game={local} onLaunch={vi.fn()} onChoose={vi.fn()} onEdit={vi.fn()} onRemove={onRemove} />);
  fireEvent.click(screen.getByRole("button", { name: "Diğer işlemler" }));
  fireEvent.click(screen.getByRole("menuitem", { name: "Kaldır" }));
  return { onRemove, confirm };
}

afterEach(() => vi.restoreAllMocks());

test("Kaldır asks first and removes on confirm", async () => {
  const { onRemove, confirm } = remove(true);
  await waitFor(() => expect(onRemove).toHaveBeenCalledWith(local));
  expect(confirm).toHaveBeenCalledWith("“Benim Oyunum” Hangar'dan kaldırılsın mı? Oyun dosyaları silinmez.");
  expect(onRemove).toHaveBeenCalledWith(local);
});

test("Kaldır does nothing when the confirm is cancelled", async () => {
  const { onRemove, confirm } = remove(false);
  await waitFor(() => expect(confirm).toHaveBeenCalled());
  await new Promise((r) => setTimeout(r, 20));
  expect(onRemove).not.toHaveBeenCalled();
});
