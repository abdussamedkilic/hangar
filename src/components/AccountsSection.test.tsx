import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { vi } from "vitest";
import AccountsSection from "./AccountsSection";
import { api } from "../lib/api";
import { ask } from "@tauri-apps/plugin-dialog";

vi.mock("@tauri-apps/plugin-dialog", () => ({ ask: vi.fn() }));
vi.mock("../lib/api", () => ({ api: { getAccounts: vi.fn(), connectAccount: vi.fn(), disconnectAccount: vi.fn(), getGogSource: vi.fn(), setGogSource: vi.fn(), getSteamAdvanced: vi.fn(), setSteamAdvanced: vi.fn() } }));

const disc = { platform: "epic" as const, state: "disconnected" as const, display_name: null };
const conn = { platform: "epic" as const, state: "connected" as const, display_name: "oyuncu" };

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(api.getAccounts).mockResolvedValue([disc]);
  vi.mocked(api.getGogSource).mockResolvedValue("galaxy");
  vi.mocked(api.getSteamAdvanced).mockResolvedValue({ api_key: null, steam_id: null });
  vi.mocked(api.setGogSource).mockResolvedValue(undefined);
  vi.mocked(api.setSteamAdvanced).mockResolvedValue(undefined);
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
  expect(ask).toHaveBeenCalledWith("Epic bağlantısı silinecek ve Epic'teki oturum kapatılacak; kurulu oyunlar kalır.", expect.anything());
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

const ubiDisc = { platform: "ubisoft" as const, state: "disconnected" as const, display_name: null };
const ubiConn = { platform: "ubisoft" as const, state: "connected" as const, display_name: "ubioyuncu" };

test("renders a row per account", async () => {
  vi.mocked(api.getAccounts).mockResolvedValue([disc, ubiDisc]);
  render(<AccountsSection onChanged={vi.fn()} />);
  expect(await screen.findByText("Epic Games")).toBeInTheDocument();
  expect(screen.getByText("Ubisoft Connect")).toBeInTheDocument();
  expect(screen.getAllByRole("button", { name: "Bağlan" })).toHaveLength(2);
  expect(screen.getByText("Giriş Ubisoft'un kendi sayfasında yapılır; Hangar şifreni görmez.")).toBeInTheDocument();
  expect(screen.getByText("Giriş Epic'in kendi sayfasında yapılır; Hangar şifreni görmez.")).toBeInTheDocument();
});

test("connects ubisoft only", async () => {
  vi.mocked(api.getAccounts).mockResolvedValue([disc, ubiDisc]);
  vi.mocked(api.connectAccount).mockResolvedValue(ubiConn);
  const onChanged = vi.fn().mockResolvedValue(undefined);
  render(<AccountsSection onChanged={onChanged} />);
  const buttons = await screen.findAllByRole("button", { name: "Bağlan" });
  fireEvent.click(buttons[1]);
  await waitFor(() => expect(onChanged).toHaveBeenCalledWith("Ubisoft oyunları yükleniyor…"));
  expect(api.connectAccount).toHaveBeenCalledWith("ubisoft");
  expect(await screen.findByText("Bağlı · ubioyuncu")).toBeInTheDocument();
  expect(screen.getAllByRole("button", { name: "Bağlan" })).toHaveLength(1);
});

test("disconnects ubisoft with its own confirm text", async () => {
  vi.mocked(api.getAccounts).mockResolvedValue([disc, ubiConn]);
  vi.mocked(ask).mockResolvedValueOnce(true);
  vi.mocked(api.disconnectAccount).mockResolvedValue(true);
  const onChanged = vi.fn().mockResolvedValue(undefined);
  render(<AccountsSection onChanged={onChanged} />);
  fireEvent.click(await screen.findByRole("button", { name: "Bağlantıyı kes" }));
  await waitFor(() => expect(onChanged).toHaveBeenCalledWith());
  expect(ask).toHaveBeenCalledWith("Ubisoft bağlantısı silinecek ve Ubisoft'taki oturum kapatılacak; kurulu oyunlar kalır.", expect.anything());
  expect(api.disconnectAccount).toHaveBeenCalledWith("ubisoft");
});

const steamDisc = { platform: "steam" as const, state: "disconnected" as const, display_name: null };
const gogDisc = { platform: "gog" as const, state: "disconnected" as const, display_name: null };

test("shows four rows in order steam, epic, gog, ubisoft", async () => {
  vi.mocked(api.getAccounts).mockResolvedValue([steamDisc, disc, gogDisc, ubiDisc]);
  render(<AccountsSection onChanged={vi.fn()} />);
  await screen.findByText("Steam");
  const labels = ["Steam", "Epic Games", "GOG", "Ubisoft Connect"].map((t) => screen.getByText(t));
  for (let i = 0; i < labels.length - 1; i++) {
    expect(labels[i].compareDocumentPosition(labels[i + 1]) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  }
});

test("steam advanced saves key and id", async () => {
  vi.mocked(api.getAccounts).mockResolvedValue([steamDisc]);
  const onChanged = vi.fn().mockResolvedValue(undefined);
  render(<AccountsSection onChanged={onChanged} />);
  fireEvent.click(await screen.findByText("Gelişmiş"));
  expect(screen.getByText("Anahtar girilirse giriş yerine anahtar kullanılır.")).toBeInTheDocument();
  fireEvent.change(screen.getByLabelText("API anahtarı"), { target: { value: "KEY123" } });
  fireEvent.change(screen.getByLabelText("SteamID"), { target: { value: "76561198000000001" } });
  fireEvent.click(screen.getByRole("button", { name: "Kaydet" }));
  await waitFor(() => expect(api.setSteamAdvanced).toHaveBeenCalledWith("KEY123", "76561198000000001"));
});

test("gog source select calls setGogSource and reveals connect", async () => {
  vi.mocked(api.getAccounts).mockResolvedValue([gogDisc]);
  render(<AccountsSection onChanged={vi.fn().mockResolvedValue(undefined)} />);
  const sel = await screen.findByLabelText("Kaynak");
  expect(screen.queryByRole("button", { name: "Bağlan" })).toBeNull();
  fireEvent.change(sel, { target: { value: "account" } });
  await waitFor(() => expect(api.setGogSource).toHaveBeenCalledWith("account"));
  expect(await screen.findByRole("button", { name: "Bağlan" })).toBeInTheDocument();
});

test("steam and gog disconnect confirms do not claim a server logout", async () => {
  for (const [platform, text] of [
    ["steam", "Steam bağlantısı silinecek; oturum bu bilgisayarda biter, kurulu oyunlar kalır."],
    ["gog", "GOG bağlantısı silinecek; oturum bu bilgisayarda biter, kurulu oyunlar kalır."],
  ] as const) {
    vi.mocked(ask).mockClear();
    vi.mocked(ask).mockResolvedValueOnce(false);
    vi.mocked(api.getAccounts).mockResolvedValue([{ platform, state: "connected", display_name: "x" }]);
    const { unmount } = render(<AccountsSection onChanged={vi.fn()} />);
    fireEvent.click(await screen.findByRole("button", { name: "Bağlantıyı kes" }));
    await waitFor(() => expect(ask).toHaveBeenCalledWith(text, expect.anything()));
    unmount();
  }
});

test("gog connected shows disconnect even with galaxy source", async () => {
  vi.mocked(api.getAccounts).mockResolvedValue([{ platform: "gog", state: "connected", display_name: "x" }]);
  render(<AccountsSection onChanged={vi.fn()} />);
  expect(await screen.findByRole("button", { name: "Bağlantıyı kes" })).toBeInTheDocument();
});
