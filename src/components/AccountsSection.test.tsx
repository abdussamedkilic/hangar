import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { vi } from "vitest";
import AccountsSection from "./AccountsSection";
import { api } from "../lib/api";
import { ask } from "@tauri-apps/plugin-dialog";

vi.mock("@tauri-apps/plugin-dialog", () => ({ ask: vi.fn() }));
vi.mock("../lib/api", () => ({ api: { getAccounts: vi.fn(), connectAccount: vi.fn(), disconnectAccount: vi.fn() } }));

const disc = { platform: "epic" as const, state: "disconnected" as const, display_name: null };
const conn = { platform: "epic" as const, state: "connected" as const, display_name: "oyuncu" };

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(api.getAccounts).mockResolvedValue([disc]);
});

test("connects and triggers labeled rescan", async () => {
  vi.mocked(api.connectAccount).mockResolvedValue(conn);
  const onChanged = vi.fn().mockResolvedValue(undefined);
  render(<AccountsSection onChanged={onChanged} />);
  fireEvent.click(await screen.findByRole("button", { name: "Bağlan" }));
  await waitFor(() => expect(onChanged).toHaveBeenCalledWith("Epic oyunları yükleniyor…"));
  expect(api.connectAccount).toHaveBeenCalledWith("epic");
  expect(await screen.findByText("Bağlı · oyuncu")).toBeInTheDocument();
});

test("cancel is silent", async () => {
  vi.mocked(api.connectAccount).mockRejectedValue("cancelled");
  const onChanged = vi.fn();
  render(<AccountsSection onChanged={onChanged} />);
  fireEvent.click(await screen.findByRole("button", { name: "Bağlan" }));
  await screen.findByRole("button", { name: "Bağlan" });
  expect(screen.queryByRole("alert")).toBeNull();
  expect(onChanged).not.toHaveBeenCalled();
});

test("other error shows alert", async () => {
  vi.mocked(api.connectAccount).mockRejectedValue("Giriş zaman aşımına uğradı");
  render(<AccountsSection onChanged={vi.fn()} />);
  fireEvent.click(await screen.findByRole("button", { name: "Bağlan" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("Giriş zaman aşımına uğradı");
});

test("disconnect asks first", async () => {
  vi.mocked(api.getAccounts).mockResolvedValue([conn]);
  vi.mocked(ask).mockResolvedValueOnce(false);
  const onChanged = vi.fn().mockResolvedValue(undefined);
  render(<AccountsSection onChanged={onChanged} />);
  fireEvent.click(await screen.findByRole("button", { name: "Bağlantıyı kes" }));
  await waitFor(() => expect(ask).toHaveBeenCalledTimes(1));
  expect(api.disconnectAccount).not.toHaveBeenCalled();
  vi.mocked(ask).mockResolvedValueOnce(true);
  vi.mocked(api.disconnectAccount).mockResolvedValue(true);
  fireEvent.click(screen.getByRole("button", { name: "Bağlantıyı kes" }));
  await waitFor(() => expect(onChanged).toHaveBeenCalledWith());
  expect(api.disconnectAccount).toHaveBeenCalledWith("epic");
});

test("server revoke failure is shown", async () => {
  vi.mocked(api.getAccounts).mockResolvedValue([conn]);
  vi.mocked(ask).mockResolvedValue(true);
  vi.mocked(api.disconnectAccount).mockResolvedValue(false);
  render(<AccountsSection onChanged={vi.fn().mockResolvedValue(undefined)} />);
  fireEvent.click(await screen.findByRole("button", { name: "Bağlantıyı kes" }));
  expect(await screen.findByRole("status")).toHaveTextContent("Epic'teki oturum kapatılamadı; Epic hesap ayarlarından kapatabilirsin.");
});

test("expired shows reconnect", async () => {
  vi.mocked(api.getAccounts).mockResolvedValue([{ ...conn, state: "expired" }]);
  render(<AccountsSection onChanged={vi.fn()} />);
  expect(await screen.findByRole("button", { name: "Yeniden bağlan" })).toBeInTheDocument();
  expect(screen.getByText("Süresi doldu")).toBeInTheDocument();
});

test("expired can also disconnect after asking", async () => {
  vi.mocked(api.getAccounts).mockResolvedValue([{ ...conn, state: "expired" }]);
  vi.mocked(ask).mockResolvedValueOnce(true);
  vi.mocked(api.disconnectAccount).mockResolvedValue(true);
  const onChanged = vi.fn().mockResolvedValue(undefined);
  render(<AccountsSection onChanged={onChanged} />);
  fireEvent.click(await screen.findByRole("button", { name: "Bağlantıyı kes" }));
  await waitFor(() => expect(onChanged).toHaveBeenCalledWith());
  expect(ask).toHaveBeenCalledTimes(1);
  expect(api.disconnectAccount).toHaveBeenCalledWith("epic");
  expect(api.connectAccount).not.toHaveBeenCalled();
});
