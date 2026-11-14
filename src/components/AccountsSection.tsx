import { useEffect, useState } from "react";
import { ask } from "@tauri-apps/plugin-dialog";
import { api, type AccountStatus } from "../lib/api";

interface Props {
  onChanged: (label?: string) => Promise<void>;
}

const STATE_TEXT = { disconnected: "Bağlı değil", expired: "Süresi doldu" } as const;
const BUTTON_TEXT = { disconnected: "Bağlan", connected: "Bağlantıyı kes", expired: "Yeniden bağlan" } as const;

const NAMES: Record<string, string> = { epic: "Epic", ubisoft: "Ubisoft" };
const OWN: Record<string, string> = { epic: "Epic'in", ubisoft: "Ubisoft'un" };
const LOC: Record<string, string> = { epic: "Epic'teki", ubisoft: "Ubisoft'taki" };
const LABELS: Record<string, string> = { epic: "Epic Games", ubisoft: "Ubisoft Connect" };

interface RowProps {
  acct: AccountStatus;
  onAccount: (a: AccountStatus) => void;
  refresh: () => Promise<void>;
  onChanged: Props["onChanged"];
}

function AccountRow({ acct, onAccount, refresh, onChanged }: RowProps) {
  const [waiting, setWaiting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [note, setNote] = useState<string | null>(null);
  const name = NAMES[acct.platform] ?? acct.platform;

  const connect = async () => {
    setError(null);
    setNote(null);
    setWaiting(true);
    try {
      onAccount(await api.connectAccount(acct.platform));
      setWaiting(false);
      await onChanged(`${name} oyunları yükleniyor…`);
    } catch (err) {
      setWaiting(false);
      if (String(err) !== "cancelled") setError(String(err));
    }
  };

  const disconnect = async () => {
    setError(null);
    setNote(null);
    try {
      const yes = await ask(`${name} bağlantısı silinecek ve ${LOC[acct.platform] ?? name} oturum kapatılacak; kurulu oyunlar kalır.`, { title: "Bağlantıyı kes", kind: "warning" });
      if (!yes) return;
      const revoked = await api.disconnectAccount(acct.platform);
      if (!revoked) setNote(`${LOC[acct.platform] ?? name} oturum kapatılamadı; ${name} hesap ayarlarından kapatabilirsin.`);
      await refresh();
      await onChanged();
    } catch (err) {
      setError(String(err));
    }
  };

  return (
    <div className="fld">
      <span>{LABELS[acct.platform] ?? name}</span>
      <div className="row">
        <span>{acct.state === "connected" ? `Bağlı · ${acct.display_name ?? ""}` : STATE_TEXT[acct.state]}</span>
        <button type="button" className="ghost" disabled={waiting} onClick={acct.state === "connected" ? disconnect : connect}>
          {waiting ? `${name} penceresi açık…` : BUTTON_TEXT[acct.state]}
        </button>
        {acct.state === "expired" && (
          <button type="button" className="ghost" disabled={waiting} onClick={disconnect}>
            Bağlantıyı kes
          </button>
        )}
      </div>
      {note && <p role="status">{note}</p>}
      {error && <p className="err" role="alert">{error}</p>}
      <small>Giriş {OWN[acct.platform] ?? name} kendi sayfasında yapılır; Hangar şifreni görmez.</small>
      <small>Bağlantı belgesi yalnızca bu bilgisayarda, şifreli tutulur.</small>
    </div>
  );
}

export default function AccountsSection({ onChanged }: Props) {
  const [accts, setAccts] = useState<AccountStatus[]>([]);
  const [error, setError] = useState<string | null>(null);

  const refresh = async () => {
    setAccts(await api.getAccounts());
  };

  useEffect(() => {
    refresh().catch((err) => setError(String(err)));
  }, []);

  const update = (a: AccountStatus) => setAccts((all) => all.map((x) => (x.platform === a.platform ? a : x)));

  if (accts.length === 0) return error ? <p className="err" role="alert">{error}</p> : null;

  return (
    <>
      {accts.map((a) => (
        <AccountRow key={a.platform} acct={a} onAccount={update} refresh={refresh} onChanged={onChanged} />
      ))}
    </>
  );
}
