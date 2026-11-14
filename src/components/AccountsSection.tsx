import { useEffect, useState } from "react";
import { ask } from "@tauri-apps/plugin-dialog";
import { api, type AccountStatus } from "../lib/api";

interface Props {
  onChanged: (label?: string) => Promise<void>;
}

const STATE_TEXT = { disconnected: "Bağlı değil", expired: "Süresi doldu" } as const;
const BUTTON_TEXT = { disconnected: "Bağlan", connected: "Bağlantıyı kes", expired: "Yeniden bağlan" } as const;

const NAMES: Record<string, string> = { steam: "Steam", epic: "Epic", gog: "GOG", ubisoft: "Ubisoft" };
const OWN: Record<string, string> = { steam: "Steam'in", epic: "Epic'in", gog: "GOG'un", ubisoft: "Ubisoft'un" };
const LOC: Record<string, string> = { steam: "Steam'deki", epic: "Epic'teki", gog: "GOG'daki", ubisoft: "Ubisoft'taki" };
const LABELS: Record<string, string> = { steam: "Steam", epic: "Epic Games", gog: "GOG", ubisoft: "Ubisoft Connect" };

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
  const [gogSource, setGogSourceState] = useState<"galaxy" | "account">("galaxy");
  const [apiKey, setApiKey] = useState("");
  const [steamId, setSteamId] = useState("");
  const [advNote, setAdvNote] = useState<string | null>(null);
  const isGog = acct.platform === "gog";
  const isSteam = acct.platform === "steam";

  useEffect(() => {
    if (isGog) api.getGogSource().then(setGogSourceState).catch(() => {});
    if (isSteam)
      api
        .getSteamAdvanced()
        .then((a) => {
          setApiKey(a.api_key ?? "");
          setSteamId(a.steam_id ?? "");
        })
        .catch(() => {});
  }, [isGog, isSteam]);

  const changeSource = async (v: "galaxy" | "account") => {
    setError(null);
    try {
      await api.setGogSource(v);
      setGogSourceState(v);
      await onChanged();
    } catch (err) {
      setError(String(err));
    }
  };

  const saveAdvanced = async () => {
    setError(null);
    setAdvNote(null);
    try {
      await api.setSteamAdvanced(apiKey.trim() || null, steamId.trim() || null);
      setAdvNote("Kaydedildi.");
      await onChanged();
    } catch (err) {
      setError(String(err));
    }
  };

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
      const local = acct.platform === "steam" || acct.platform === "gog";
      const msg = local
        ? `${name} bağlantısı silinecek; oturum bu bilgisayarda biter, kurulu oyunlar kalır.`
        : `${name} bağlantısı silinecek ve ${LOC[acct.platform] ?? name} oturum kapatılacak; kurulu oyunlar kalır.`;
      const yes = await ask(msg, { title: "Bağlantıyı kes", kind: "warning" });
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
        {(!isGog || gogSource === "account" || acct.state === "connected") && (
          <button type="button" className="ghost" disabled={waiting} onClick={acct.state === "connected" ? disconnect : connect}>
            {waiting ? `${name} penceresi açık…` : BUTTON_TEXT[acct.state]}
          </button>
        )}
        {isGog && (
          <label>
            Kaynak
            <select value={gogSource} onChange={(e) => void changeSource(e.target.value as "galaxy" | "account")}>
              <option value="galaxy">Galaxy veritabanı</option>
              <option value="account">GOG hesabı</option>
            </select>
          </label>
        )}
        {acct.state === "expired" && (
          <button type="button" className="ghost" disabled={waiting} onClick={disconnect}>
            Bağlantıyı kes
          </button>
        )}
      </div>
      {isSteam && (
        <details>
          <summary>Gelişmiş</summary>
          <label>
            API anahtarı
            <input type="password" value={apiKey} onChange={(e) => setApiKey(e.target.value)} autoComplete="off" />
          </label>
          <label>
            SteamID
            <input value={steamId} onChange={(e) => setSteamId(e.target.value)} />
          </label>
          <small>Anahtar girilirse giriş yerine anahtar kullanılır.</small>
          <button type="button" className="ghost" onClick={() => void saveAdvanced()}>Kaydet</button>
          {advNote && <p role="status">{advNote}</p>}
        </details>
      )}
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
