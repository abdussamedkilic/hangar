import { useEffect, useState } from "react";
import { ask } from "@tauri-apps/plugin-dialog";
import { api, type AccountStatus } from "../lib/api";

interface Props {
  onChanged: (label?: string) => Promise<void>;
}

const STATE_TEXT = { disconnected: "Bağlı değil", expired: "Süresi doldu" } as const;
const BUTTON_TEXT = { disconnected: "Bağlan", connected: "Bağlantıyı kes", expired: "Yeniden bağlan" } as const;

export default function AccountsSection({ onChanged }: Props) {
  const [acct, setAcct] = useState<AccountStatus | null>(null);
  const [waiting, setWaiting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [note, setNote] = useState<string | null>(null);

  const refresh = async () => {
    const all = await api.getAccounts();
    setAcct(all.find((a) => a.platform === "epic") ?? null);
  };

  useEffect(() => {
    refresh().catch((err) => setError(String(err)));
  }, []);

  const connect = async () => {
    setError(null);
    setNote(null);
    setWaiting(true);
    try {
      setAcct(await api.connectAccount("epic"));
      setWaiting(false);
      await onChanged("Epic oyunları yükleniyor…");
    } catch (err) {
      setWaiting(false);
      if (String(err) !== "cancelled") setError(String(err));
    }
  };

  const disconnect = async () => {
    setError(null);
    setNote(null);
    try {
      const yes = await ask("Epic bağlantısı silinecek ve Epic'teki oturum kapatılacak; kurulu oyunlar kalır.", { title: "Bağlantıyı kes", kind: "warning" });
      if (!yes) return;
      const revoked = await api.disconnectAccount("epic");
      if (!revoked) setNote("Epic'teki oturum kapatılamadı; Epic hesap ayarlarından kapatabilirsin.");
      await refresh();
      await onChanged();
    } catch (err) {
      setError(String(err));
    }
  };

  if (!acct) return error ? <p className="err" role="alert">{error}</p> : null;

  return (
    <div className="fld">
      <span>Epic Games</span>
      <div className="row">
        <span>{acct.state === "connected" ? `Bağlı · ${acct.display_name ?? ""}` : STATE_TEXT[acct.state]}</span>
        <button type="button" className="ghost" disabled={waiting} onClick={acct.state === "connected" ? disconnect : connect}>
          {waiting ? "Epic penceresi açık…" : BUTTON_TEXT[acct.state]}
        </button>
        {acct.state === "expired" && (
          <button type="button" className="ghost" disabled={waiting} onClick={disconnect}>
            Bağlantıyı kes
          </button>
        )}
      </div>
      {note && <p role="status">{note}</p>}
      {error && <p className="err" role="alert">{error}</p>}
      <small>Giriş Epic'in kendi sayfasında yapılır; Hangar şifreni görmez.</small>
      <small>Bağlantı belgesi yalnızca bu bilgisayarda, şifreli tutulur.</small>
    </div>
  );
}
