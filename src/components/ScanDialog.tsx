import { useEffect, useState } from "react";
import { api, type Candidate, type ScanReport } from "../lib/api";
import Modal from "./Modal";

interface Props {
  onDone: (r: ScanReport) => void;
  onClose: () => void;
}

interface Row extends Candidate {
  on: boolean;
}

export default function ScanDialog({ onDone, onClose }: Props) {
  const [rows, setRows] = useState<Row[] | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api
      .scanCandidates()
      .then((c) => setRows(c.map((x) => ({ ...x, on: true }))))
      .catch((err) => {
        setError(String(err));
        setRows([]);
      });
  }, []);

  const patch = (i: number, p: Partial<Row>) => setRows((r) => r && r.map((x, j) => (j === i ? { ...x, ...p } : x)));
  const chosen = (rows ?? []).filter((r) => r.on && r.title.trim());

  const submit = async () => {
    setBusy(true);
    setError(null);
    try {
      const r = await api.addLocalGames(chosen.map((c) => ({ title: c.title.trim(), exe_path: c.exe_path })));
      onDone(r);
      onClose();
    } catch (err) {
      setError(String(err));
      setBusy(false);
    }
  };

  return (
    <Modal label="Klasör taraması" onClose={onClose}>
      <h2>Bulunan oyunlar</h2>
      {rows === null && <p>Taranıyor…</p>}
      {rows !== null && rows.length === 0 && !error && <p>Bulunamadı — Oyun ekle ile elle ekleyebilirsin.</p>}
      {rows !== null && rows.length > 0 && (
        <ul className="scanlist">
          {rows.map((r, i) => (
            <li key={r.exe_path}>
              <input type="checkbox" checked={r.on} onChange={(e) => patch(i, { on: e.target.checked })} aria-label={`Seç: ${r.exe_path}`} />
              <div>
                <input value={r.title} onChange={(e) => patch(i, { title: e.target.value })} spellCheck={false} aria-label={`Ad: ${r.exe_path}`} />
                <small><code>{r.exe_path}</code></small>
              </div>
            </li>
          ))}
        </ul>
      )}
      {error && <p className="err" role="alert">{error}</p>}
      <div className="acts">
        <button type="button" className="ghost" onClick={onClose}>Vazgeç</button>
        <button type="button" className="primary" disabled={busy || chosen.length === 0} onClick={submit}>Seçilenleri ekle</button>
      </div>
    </Modal>
  );
}
