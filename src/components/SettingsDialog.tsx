import { useCallback, useEffect, useRef, useState, type FormEvent } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api, type ScanReport, type Settings } from "../lib/api";
import Modal from "./Modal";
import ScanDialog from "./ScanDialog";
import AccountsSection from "./AccountsSection";

interface Props {
  settings: Settings;
  onSaved: () => Promise<void>;
  onResetTheme?: () => Promise<void>;
  onScanned?: (r: ScanReport) => void;
  onAccountsChanged?: (label?: string) => Promise<void>;
  onClose: () => void;
}

export default function SettingsDialog({ settings, onSaved, onResetTheme, onScanned, onAccountsChanged, onClose }: Props) {
  const [key, setKey] = useState(settings.steam_api_key ?? "");
  const [steamId, setSteamId] = useState(settings.steam_id ?? "");
  const [show, setShow] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [folders, setFolders] = useState<string[]>([]);
  const [scanning, setScanning] = useState(false);

  useEffect(() => {
    api.getScanFolders().then(setFolders).catch((err) => setError(String(err)));
  }, []);

  const saveFolders = async (next: string[]) => {
    setFolders(next);
    try {
      await api.setScanFolders(next);
    } catch (err) {
      setError(String(err));
    }
  };

  const addFolder = async () => {
    try {
      const f = await open({ directory: true });
      if (typeof f === "string" && !folders.includes(f)) await saveFolders([...folders, f]);
    } catch (err) {
      setError(String(err));
    }
  };

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      await api.saveSettings({ steam_api_key: key.trim() || null, steam_id: steamId.trim() || null });
      await onSaved();
      onClose();
    } catch (err) {
      setError(String(err));
      setBusy(false);
    }
  };

  const scanningRef = useRef(false);
  scanningRef.current = scanning;
  const onCloseRef = useRef(onClose);
  onCloseRef.current = onClose;
  const modalClose = useCallback(() => {
    if (!scanningRef.current) onCloseRef.current();
  }, []);

  return (
    <Modal label="Ayarlar" onClose={modalClose}>
      <form onSubmit={submit}>
        <h2>Ayarlar</h2>
        <label className="fld">
          <span>Steam API anahtarı</span>
          <div className="row">
            <input type={show ? "text" : "password"} value={key} onChange={(e) => setKey(e.target.value)} autoComplete="off" spellCheck={false} />
            <button type="button" className="ghost" onClick={() => setShow((s) => !s)}>{show ? "Gizle" : "Göster"}</button>
          </div>
          <small>Anahtarı şuradan alabilirsin: <code>https://steamcommunity.com/dev/apikey</code></small>
        </label>
        <label className="fld">
          <span>SteamID</span>
          <input value={steamId} onChange={(e) => setSteamId(e.target.value)} placeholder="Otomatik bulunur" spellCheck={false} />
        </label>
        <div className="fld">
          <span>Oyun klasörleri</span>
          <ul className="scanlist">
            {folders.map((f) => (
              <li key={f}>
                <code>{f}</code>
                <button type="button" className="ghost" onClick={() => saveFolders(folders.filter((x) => x !== f))}>Kaldır</button>
              </li>
            ))}
          </ul>
          <div className="row">
            <button type="button" className="ghost" onClick={addFolder}>Klasör ekle</button>
            <button type="button" className="ghost" disabled={folders.length === 0} onClick={() => setScanning(true)}>Klasörleri tara</button>
          </div>
        </div>
        <div className="fld">
          <span>Görünüm</span>
          <div className="row">
            <button type="button" className="ghost" onClick={() => api.openThemeFile().catch((err) => setError(String(err)))}>Tema dosyasını aç</button>
            <button type="button" className="ghost" onClick={() => onResetTheme?.().catch((err) => setError(String(err)))}>Varsayılana dön</button>
          </div>
        </div>
        <AccountsSection onChanged={onAccountsChanged ?? (async () => {})} />
        {error && <p className="err" role="alert">{error}</p>}
        <div className="acts">
          <button type="button" className="ghost" onClick={onClose}>Vazgeç</button>
          <button type="submit" className="primary" disabled={busy}>{busy ? "Kaydediliyor…" : "Kaydet ve tara"}</button>
        </div>
      </form>
      {scanning && <ScanDialog onDone={(r) => onScanned?.(r)} onClose={() => setScanning(false)} />}
    </Modal>
  );
}
