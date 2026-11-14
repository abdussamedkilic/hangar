import { render, screen, fireEvent } from "@testing-library/react";
import { vi } from "vitest";
import TabStrip from "./TabStrip";

const counts = { all: 9, platform: { steam: 5, epic: 0, ubisoft: 2, gog: 0, riot: 0, local: 0 }, collection: { 1: 0, 2: 3 } };
const cols = [{ id: 1, name: "Boş", entries: [] }, { id: 2, name: "AC", entries: [] }];
const setup = (o: Partial<React.ComponentProps<typeof TabStrip>> = {}) => {
  const p: React.ComponentProps<typeof TabStrip> = { tab: { kind: "all" }, counts, collections: cols, installedOnly: false, onTab: vi.fn(), onClearInstalled: vi.fn(), onCreate: vi.fn().mockResolvedValue(undefined), onTabContext: vi.fn(), ...o };
  render(<TabStrip {...p} />);
  return p;
};

test("order: Tümü, stores with games, separator, collections, +", () => {
  setup();
  const names = [...document.querySelectorAll("[role=tab], [role=separator], .tab-add")].map((e) =>
    e.getAttribute("role") === "separator" ? "|" : e.getAttribute("aria-label") === "Koleksiyon ekle" ? "+" : e.textContent,
  );
  expect(names).toEqual(["Tümü9", "Steam5", "Ubisoft2", "|", "Boş0", "AC3", "+"]);
});

test("aria-selected marks the active tab", () => {
  setup({ tab: { kind: "platform", platform: "steam" } });
  expect(screen.getByRole("tab", { selected: true })).toHaveTextContent("Steam");
});

test("+ opens a textbox: Enter creates, Esc cancels, empty Enter does nothing", () => {
  const p = setup();
  fireEvent.click(screen.getByRole("button", { name: "Koleksiyon ekle" }));
  const box = screen.getByRole("textbox");
  fireEvent.keyDown(box, { key: "Enter" });
  expect(p.onCreate).not.toHaveBeenCalled();
  fireEvent.change(box, { target: { value: "AC" } });
  fireEvent.keyDown(box, { key: "Enter" });
  expect(p.onCreate).toHaveBeenCalledWith("AC");
  expect(screen.queryByRole("textbox")).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Koleksiyon ekle" }));
  fireEvent.keyDown(screen.getByRole("textbox"), { key: "Escape" });
  expect(screen.queryByRole("textbox")).toBeNull();
  expect(p.onCreate).toHaveBeenCalledTimes(1);
});

test("installedOnly shows a clearable Kurulu chip", () => {
  const p = setup({ installedOnly: true });
  fireEvent.click(screen.getByRole("button", { name: "Kurulu ✕" }));
  expect(p.onClearInstalled).toHaveBeenCalled();
});

test("collection tab right-click reports the collection", () => {
  const p = setup();
  fireEvent.contextMenu(screen.getByRole("tab", { name: /AC/ }));
  expect(p.onTabContext).toHaveBeenCalledWith(cols[1], expect.any(Number), expect.any(Number));
});

test("renamingId shows an input that commits via onRename", () => {
  const onRename = vi.fn().mockResolvedValue(undefined);
  setup({ renamingId: 2, onRename });
  const box = screen.getByRole("textbox", { name: "Yeni ad" });
  fireEvent.change(box, { target: { value: "Yeni" } });
  fireEvent.keyDown(box, { key: "Enter" });
  expect(onRename).toHaveBeenCalledWith(2, "Yeni");
});

test("presence keeps a store tab whose installed-aware count is 0", () => {
  const inst = { ...counts, platform: { ...counts.platform, ubisoft: 0 } };
  setup({ counts: inst, presence: counts, installedOnly: true });
  expect(screen.getByRole("tab", { name: "Ubisoft 0" })).toBeInTheDocument();
});

const MIME = "application/x-hangar-game";
const dt = (data: Record<string, string>) => ({ types: Object.keys(data), getData: (k: string) => data[k] ?? "", dropEffect: "none" });

test("dropping a game onto a collection tab calls onDropGame; store tab ignores it", () => {
  const onDropGame = vi.fn();
  setup({ collections: [{ id: 7, name: "Favoriler", entries: [] }], onDropGame });
  const col = screen.getByRole("tab", { name: /Favoriler/ });
  fireEvent.dragOver(col, { dataTransfer: dt({ [MIME]: "42" }) });
  expect(col).toHaveClass("drop-ok");
  fireEvent.drop(col, { dataTransfer: dt({ [MIME]: "42" }) });
  expect(onDropGame).toHaveBeenCalledWith(7, 42);
  expect(col).not.toHaveClass("drop-ok");
  const store = screen.getByRole("tab", { name: /Steam/ });
  fireEvent.dragOver(store, { dataTransfer: dt({ [MIME]: "42" }) });
  fireEvent.drop(store, { dataTransfer: dt({ [MIME]: "42" }) });
  expect(onDropGame).toHaveBeenCalledTimes(1);
  expect(store).not.toHaveClass("drop-ok");
});

test("foreign drag types are not accepted on collection tabs", () => {
  const onDropGame = vi.fn();
  setup({ collections: [{ id: 7, name: "Favoriler", entries: [] }], onDropGame });
  const col = screen.getByRole("tab", { name: /Favoriler/ });
  fireEvent.dragOver(col, { dataTransfer: dt({ "text/plain": "42" }) });
  fireEvent.drop(col, { dataTransfer: dt({ "text/plain": "42" }) });
  expect(col).not.toHaveClass("drop-ok");
  expect(onDropGame).not.toHaveBeenCalled();
});
