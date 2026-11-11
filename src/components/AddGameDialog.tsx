import { useState, type FormEvent } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api, type ScanReport } from "../lib/api";
import Modal from "./Modal";

interface Props {
  edit?: { id: number; title: string; exe: string };
  onDone: (r: ScanReport) => void;
  onClose: () => void;
}

const stem = (path: string) => (path.split(/[\\/]/).pop() ?? "").replace(/\.exe$/i, "");

export default function AddGameDialog({ edit, onDone, onClose }: Props) {
  const [exe, setExe] = useState(edit?.exe ?? "");
  const [name, setName] = useState(edit?.title ?? "");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const pick = async () => {
    try {
      const f = await open({ filters: [{ name: "Oyun", extensions: ["exe"] }] });
      if (typeof f !== "string") return;
      setExe(f);
      if (!name.trim()) setName(stem(f));
    } catch (err) {
      setError(String(err));
    }
  };

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      const title = name.trim();
      const r = edit ? await api.updateLocalGame(edit.id, title, exe) : await api.addLocalGame(title, exe);
      onDone(r);
      onClose();
    } catch (err) {
      setError(String(err));
      setBusy(false);
    }
  };

  const label = edit ? "Oyunu düzenle" : "Oyun ekle";
  return (
    <Modal label={label} onClose={onClose}>
      <form onSubmit={submit}>
        <h2>{label}</h2>
        <div className="fld">
          <span>Oyun dosyası</span>
          <div className="row">
            <button type="button" className="ghost" onClick={pick}>Exe seç</button>
          </div>
          <small>{exe ? <code>{exe}</code> : "Henüz seçilmedi"}</small>
        </div>
        <label className="fld">
          <span>Ad</span>
          <input value={name} onChange={(e) => setName(e.target.value)} spellCheck={false} />
        </label>
        {error && <p className="err" role="alert">{error}</p>}
        <div className="acts">
          <button type="button" className="ghost" onClick={onClose}>Vazgeç</button>
          <button type="submit" className="primary" disabled={busy || !name.trim() || !exe}>{edit ? "Kaydet" : "Ekle"}</button>
        </div>
      </form>
    </Modal>
  );
}
