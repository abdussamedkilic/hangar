import { renderHook, act } from "@testing-library/react";
import { vi } from "vitest";

const changed = vi.hoisted(() => vi.fn());
const rescan = vi.hoisted(() => vi.fn());
const setUiPref = vi.hoisted(() => vi.fn().mockResolvedValue(undefined));
const getUiPrefs = vi.hoisted(() => vi.fn());
const getFilters = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn().mockResolvedValue(() => {}) }));
vi.mock("../lib/api", () => ({
  api: { getLibrary: vi.fn().mockResolvedValue([]), rescan, galaxyChangedSinceScan: changed, setUiPref, getUiPrefs, getFilters, getCollections: vi.fn().mockResolvedValue([]) },
}));

import { useLibrary } from "./useLibrary";

beforeEach(() => {
  setUiPref.mockClear();
  getUiPrefs.mockReset().mockResolvedValue({ view: "grid", tab: "all", installed: false, filter: "", sidebar: "open" });
  getFilters.mockReset().mockResolvedValue([]);
});

test("focus rescans only when Galaxy changed, at most once per 10 s", async () => {
  rescan.mockResolvedValue({ games: 0, warnings: [] });
  renderHook(() => useLibrary());
  await act(async () => {});
  rescan.mockClear();
  vi.useFakeTimers({ toFake: ["Date"] });
  vi.setSystemTime(100_000);

  changed.mockResolvedValue(false);
  await act(async () => { window.dispatchEvent(new Event("focus")); });
  expect(changed).toHaveBeenCalledTimes(1);
  expect(rescan).not.toHaveBeenCalled();

  changed.mockResolvedValue(true);
  vi.setSystemTime(105_000);
  await act(async () => { window.dispatchEvent(new Event("focus")); });
  expect(changed).toHaveBeenCalledTimes(1); // debounced

  vi.setSystemTime(111_000);
  await act(async () => { window.dispatchEvent(new Event("focus")); });
  expect(rescan).toHaveBeenCalledTimes(1);
  vi.useRealTimers();
});

test("setTab and setFilter persist ui prefs", async () => {
  rescan.mockResolvedValue({ games: 0, warnings: [] });
  const { result } = renderHook(() => useLibrary());
  await act(async () => {});
  act(() => result.current.setTab({ kind: "platform", platform: "steam" }));
  act(() => result.current.setFilter({ kind: "installed" }));
  expect(setUiPref).toHaveBeenCalledWith("ui.tab", "p:steam");
  expect(setUiPref).toHaveBeenCalledWith("ui.filter", "installed");
  act(() => result.current.setFilter({ kind: "saved", id: 4 }));
  expect(setUiPref).toHaveBeenCalledWith("ui.filter", "4");
  act(() => result.current.setFilter({ kind: "none" }));
  expect(setUiPref).toHaveBeenCalledWith("ui.filter", "");
  expect(result.current.tab).toEqual({ kind: "platform", platform: "steam" });
});

test("migrates ui.installed=1 to ui.filter=installed when ui.filter is empty", async () => {
  rescan.mockResolvedValue({ games: 0, warnings: [] });
  getUiPrefs.mockResolvedValue({ view: "grid", tab: "all", installed: true, filter: "", sidebar: "open" });
  const { result } = renderHook(() => useLibrary());
  await act(async () => {});
  expect(result.current.filter).toEqual({ kind: "installed" });
  expect(setUiPref).toHaveBeenCalledWith("ui.filter", "installed");
  expect(setUiPref).toHaveBeenCalledWith("ui.installed", "0");
  // later launch: filter cleared, installed already reset -> stays none
  getUiPrefs.mockResolvedValue({ view: "grid", tab: "all", installed: false, filter: "", sidebar: "open" });
  const again = renderHook(() => useLibrary());
  await act(async () => {});
  expect(again.result.current.filter).toEqual({ kind: "none" });
});

test("invalid rule fields are dropped, valid ones kept", async () => {
  rescan.mockResolvedValue({ games: 0, warnings: [] });
  getFilters.mockResolvedValue([
    { id: 1, name: "X", rule: '{"stores":["steam","bogus"],"installed":"maybe","played":"no","edition":5,"collection":{"id":"1","in":true}}' },
    { id: 2, name: "Y", rule: "[1]" },
  ]);
  const { result } = renderHook(() => useLibrary());
  await act(async () => {});
  expect(result.current.savedFilters.map((f) => f.rule)).toEqual([{ stores: ["steam"], played: "no" }, {}]);
});

test("ui.filter pointing at a deleted filter becomes none; existing one is kept", async () => {
  rescan.mockResolvedValue({ games: 0, warnings: [] });
  getFilters.mockResolvedValue([{ id: 2, name: "E", rule: '{"played":"yes"}' }]);
  getUiPrefs.mockResolvedValue({ view: "grid", tab: "all", installed: false, filter: "7", sidebar: "open" });
  const a = renderHook(() => useLibrary());
  await act(async () => {});
  expect(a.result.current.filter).toEqual({ kind: "none" });
  getUiPrefs.mockResolvedValue({ view: "grid", tab: "all", installed: false, filter: "2", sidebar: "open" });
  const b = renderHook(() => useLibrary());
  await act(async () => {});
  expect(b.result.current.filter).toEqual({ kind: "saved", id: 2 });
  expect(b.result.current.savedFilters[0].rule).toEqual({ played: "yes" });
});

test("setViewMode persists ui.view", async () => {
  rescan.mockResolvedValue({ games: 0, warnings: [] });
  const { result } = renderHook(() => useLibrary());
  await act(async () => {});
  act(() => result.current.setViewMode("list"));
  expect(setUiPref).toHaveBeenCalledWith("ui.view", "list");
  expect(result.current.viewMode).toBe("list");
});
