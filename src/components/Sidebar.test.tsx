import { render, screen, fireEvent } from "@testing-library/react";
import { vi } from "vitest";
import Sidebar from "./Sidebar";

const saved = [{ id: 3, name: "Bitmemişler", rule: {} }];
function setup(over: Partial<React.ComponentProps<typeof Sidebar>> = {}) {
  const p = {
    collapsed: false, onToggleCollapsed: vi.fn(), filter: { kind: "none" } as const, savedFilters: saved, counts: { installed: 5, "3": 2 },
    onFilter: vi.fn(), onNewFilter: vi.fn(), onFilterContext: vi.fn(), onAdd: vi.fn(), onRescan: vi.fn(), onSettings: vi.fn(), rescanning: false, ...over,
  };
  render(<Sidebar {...p} />);
  return p;
}

test("collapsed sidebar keeps all actions reachable", () => {
  const p = setup({ collapsed: true });
  for (const [name, fn] of [["Oyun ekle", p.onAdd], ["Yenile", p.onRescan], ["Ayarlar", p.onSettings], ["Yeni filtre", p.onNewFilter], ["Menüyü genişlet", p.onToggleCollapsed]] as const) {
    const b = screen.getByRole("button", { name });
    expect(b).toHaveAttribute("title", name);
    fireEvent.click(b);
    expect(fn).toHaveBeenCalled();
  }
  fireEvent.click(screen.getByRole("button", { name: "Bitmemişler" }));
  expect(p.onFilter).toHaveBeenCalledWith({ kind: "saved", id: 3 });
  expect(screen.getByText("B")).toBeInTheDocument();
});

test("active filter toggles off when clicked again", () => {
  const p = setup({ filter: { kind: "saved", id: 3 } });
  const b = screen.getByRole("button", { name: "Bitmemişler" });
  expect(b).toHaveAttribute("aria-pressed", "true");
  fireEvent.click(b);
  expect(p.onFilter).toHaveBeenCalledWith({ kind: "none" });
});

test("Kurulu is builtin and has no context menu", () => {
  const p = setup();
  fireEvent.contextMenu(screen.getByRole("button", { name: "Kurulu" }));
  expect(p.onFilterContext).not.toHaveBeenCalled();
  fireEvent.contextMenu(screen.getByRole("button", { name: "Bitmemişler" }));
  expect(p.onFilterContext).toHaveBeenCalledWith(saved[0], expect.any(Number), expect.any(Number));
});

// @ts-expect-error node types are not installed; vitest runs under node
import { readFileSync } from "node:fs";
const css: string = readFileSync("src/styles/app.css", "utf8");
test("grp never scrolls horizontally; collapsed rail hides scrollbar", () => {
  expect(css).toMatch(/\.grp\{[^}]*overflow-x:hidden/);
  expect(css).toMatch(/\.collapsed \.grp\{[^}]*scrollbar-width:none/);
});
