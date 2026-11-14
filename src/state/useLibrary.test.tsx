import { renderHook, act } from "@testing-library/react";
import { vi } from "vitest";

const changed = vi.hoisted(() => vi.fn());
const rescan = vi.hoisted(() => vi.fn());
const setUiPref = vi.hoisted(() => vi.fn().mockResolvedValue(undefined));

vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn().mockResolvedValue(() => {}) }));
vi.mock("../lib/api", () => ({
  api: { getLibrary: vi.fn().mockResolvedValue([]), rescan, galaxyChangedSinceScan: changed, setUiPref },
}));

import { useLibrary } from "./useLibrary";

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

test("setTab and setInstalledOnly persist ui prefs", async () => {
  rescan.mockResolvedValue({ games: 0, warnings: [] });
  const { result } = renderHook(() => useLibrary());
  await act(async () => {});
  act(() => result.current.setTab({ kind: "platform", platform: "steam" }));
  act(() => result.current.setInstalledOnly(true));
  expect(setUiPref).toHaveBeenCalledWith("ui.tab", "p:steam");
  expect(setUiPref).toHaveBeenCalledWith("ui.installed", "1");
  expect(result.current.tab).toEqual({ kind: "platform", platform: "steam" });
});

test("setViewMode persists ui.view", async () => {
  rescan.mockResolvedValue({ games: 0, warnings: [] });
  const { result } = renderHook(() => useLibrary());
  await act(async () => {});
  act(() => result.current.setViewMode("list"));
  expect(setUiPref).toHaveBeenCalledWith("ui.view", "list");
  expect(result.current.viewMode).toBe("list");
});
