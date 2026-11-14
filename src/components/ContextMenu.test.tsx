import { render, screen, fireEvent } from "@testing-library/react";
import { vi } from "vitest";
import ContextMenu, { clampMenu } from "./ContextMenu";

test("clampMenu keeps menu inside viewport", () => {
  expect(clampMenu(790, 590, 200, 150, 800, 600)).toEqual({ x: 590, y: 440 });
  expect(clampMenu(10, 20, 200, 150, 800, 600)).toEqual({ x: 10, y: 20 });
  expect(clampMenu(10, 20, 900, 700, 800, 600)).toEqual({ x: 0, y: 0 });
});

test("submenu item toggles", () => {
  const onSelect = vi.fn();
  const onClose = vi.fn();
  render(
    <ContextMenu
      x={10}
      y={10}
      onClose={onClose}
      items={[{ label: "Koleksiyona ekle", items: [{ label: "Favoriler", checked: true, onSelect }, { label: "Diğer", checked: false, onSelect: () => {} }] }]}
    />,
  );
  fireEvent.click(screen.getByText(/Koleksiyona ekle/));
  const fav = screen.getByRole("menuitemcheckbox", { name: /Favoriler/ });
  expect(fav).toHaveAttribute("aria-checked", "true");
  expect(screen.getByRole("menuitemcheckbox", { name: /Diğer/ })).toHaveAttribute("aria-checked", "false");
  fireEvent.click(fav);
  expect(onSelect).toHaveBeenCalled();
  expect(onClose).toHaveBeenCalled();
});
