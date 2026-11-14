import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { vi } from "vitest";
import FilterEditor from "./FilterEditor";

test("FilterEditor saves rule", async () => {
  const onSave = vi.fn().mockResolvedValue(undefined);
  const onClose = vi.fn();
  render(<FilterEditor collections={[{ id: 4, name: "Favoriler", entries: [] }]} onSave={onSave} onClose={onClose} />);
  expect(screen.getByRole("button", { name: "Kaydet" })).toBeDisabled();
  fireEvent.change(screen.getByLabelText("Ad"), { target: { value: " Bitmemiş " } });
  fireEvent.click(screen.getByLabelText("Steam"));
  fireEvent.click(screen.getByLabelText("GOG"));
  fireEvent.change(screen.getByLabelText("Kurulu"), { target: { value: "yes" } });
  fireEvent.change(screen.getByLabelText("Oynanmış"), { target: { value: "no" } });
  fireEvent.change(screen.getByLabelText("Sürüm"), { target: { value: "original" } });
  fireEvent.change(screen.getByLabelText("Koleksiyon"), { target: { value: "4" } });
  fireEvent.change(screen.getByLabelText("Koleksiyon durumu"), { target: { value: "out" } });
  expect(screen.getByText(/Oynama süresi yalnız Steam'de var/)).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Kaydet" }));
  await waitFor(() => expect(onSave).toHaveBeenCalled());
  expect(onSave).toHaveBeenCalledWith("Bitmemiş", { stores: ["steam", "gog"], installed: "yes", played: "no", edition: "original", collection: { id: 4, in: false } });
  await waitFor(() => expect(onClose).toHaveBeenCalled());
});

test("FilterEditor edit prefills and shows save errors", async () => {
  const onSave = vi.fn().mockRejectedValue("boom");
  render(<FilterEditor initial={{ id: 1, name: "X", rule: { installed: "no" } }} collections={[]} onSave={onSave} onClose={vi.fn()} />);
  expect(screen.getByLabelText("Ad")).toHaveValue("X");
  expect(screen.getByLabelText("Kurulu")).toHaveValue("no");
  fireEvent.click(screen.getByRole("button", { name: "Kaydet" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("boom");
});
