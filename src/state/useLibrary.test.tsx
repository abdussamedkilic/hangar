import { renderHook, act } from "@testing-library/react";
import { vi } from "vitest";

const changed = vi.hoisted(() => vi.fn());
const rescan = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn().mockResolvedValue(() => {}) }));
vi.mock("../lib/api", () => ({
  api: { getLibrary: vi.fn().mockResolvedValue([]), rescan, galaxyChangedSinceScan: changed },
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
