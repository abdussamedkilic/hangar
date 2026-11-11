import { useState, type FormEvent } from "react";
import { api, type Settings } from "../lib/api";
import Modal from "./Modal";

interface Props {
  settings: Settings;
  onSaved: () => Promise<void>;
  onClose: () => void;
}

export default function SettingsDialog({ settings, onSaved, onClose }: Props) {
  const [key, setKey] = useState(settings.steam_api_key ?? "");
  const [steamId, setSteamId] = useState(settings.steam_id ?? "");
  const [show, setShow] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

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

  return (
    <Modal label="Ayarlar" onClose={onClose}>
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
        {error && <p className="err" role="alert">{error}</p>}
        <div className="acts">
          <button type="button" className="ghost" onClick={onClose}>Vazgeç</button>
          <button type="submit" className="primary" disabled={busy}>{busy ? "Kaydediliyor…" : "Kaydet ve tara"}</button>
        </div>
      </form>
    </Modal>
  );
}
