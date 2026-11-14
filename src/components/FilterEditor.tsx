import { useState, type FormEvent } from "react";
import type { Collection, Platform } from "../lib/api";
import type { FilterRule, SavedFilter } from "../lib/library";
import Modal from "./Modal";

interface Props {
  initial?: SavedFilter;
  collections: Collection[];
  onSave: (name: string, rule: FilterRule) => Promise<void>;
  onClose: () => void;
}

const PLAT: Array<[Platform, string]> = [["steam", "Steam"], ["epic", "Epic"], ["ubisoft", "Ubisoft"], ["gog", "GOG"], ["riot", "Riot"], ["local", "Yerel"]];
const YN = [["", "Fark etmez"], ["yes", "Evet"], ["no", "Hayır"]] as const;

export default function FilterEditor({ initial, collections, onSave, onClose }: Props) {
  const r = initial?.rule ?? {};
  const [name, setName] = useState(initial?.name ?? "");
  const [stores, setStores] = useState<Platform[]>(r.stores ?? []);
  const [installed, setInstalled] = useState<string>(r.installed ?? "");
  const [played, setPlayed] = useState<string>(r.played ?? "");
  const [edition, setEdition] = useState<string>(r.edition ?? "");
  const [colId, setColId] = useState<string>(r.collection && collections.some((c) => c.id === r.collection!.id) ? String(r.collection.id) : "");
  const [colIn, setColIn] = useState<boolean>(r.collection?.in ?? true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const toggle = (p: Platform) => setStores((s) => (s.includes(p) ? s.filter((x) => x !== p) : [...s, p]));

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    const rule: FilterRule = {};
    if (stores.length) rule.stores = stores;
    if (installed === "yes" || installed === "no") rule.installed = installed;
    if (played === "yes" || played === "no") rule.played = played;
    if (edition === "original" || edition === "special") rule.edition = edition;
    if (colId) rule.collection = { id: Number(colId), in: colIn };
    setBusy(true);
    setError(null);
    try {
      await onSave(name.trim(), rule);
      onClose();
    } catch (err) {
      setError(String(err));
      setBusy(false);
    }
  };

  const label = initial ? "Filtreyi düzenle" : "Yeni filtre";
  return (
    <Modal label={label} onClose={onClose}>
      <form onSubmit={submit}>
        <h2>{label}</h2>
        <label className="fld">
          <span>Ad</span>
          <input value={name} onChange={(e) => setName(e.target.value)} spellCheck={false} maxLength={40} />
        </label>
        <fieldset className="fld chk-set">
          <legend>Mağaza</legend>
          <div className="chk-row">
            {PLAT.map(([p, l]) => (
              <label key={p} className="chk">
                <input type="checkbox" checked={stores.includes(p)} onChange={() => toggle(p)} />
                <span>{l}</span>
              </label>
            ))}
          </div>
        </fieldset>
        <div className="fld-grid">
          <label className="fld">
            <span>Kurulu</span>
            <select value={installed} onChange={(e) => setInstalled(e.target.value)}>
              {YN.map(([v, l]) => <option key={v} value={v}>{l}</option>)}
            </select>
          </label>
          <label className="fld">
            <span>Oynanmış</span>
            <select value={played} onChange={(e) => setPlayed(e.target.value)}>
              {YN.map(([v, l]) => <option key={v} value={v}>{l}</option>)}
            </select>
          </label>
          <label className="fld">
            <span>Sürüm</span>
            <select value={edition} onChange={(e) => setEdition(e.target.value)}>
              <option value="">Fark etmez</option>
              <option value="original">Orijinal</option>
              <option value="special">Özel sürüm</option>
            </select>
          </label>
        </div>
        <small className="fld-note">Oynama süresi yalnız Steam'de var; diğerlerinde Hangar'dan başlatılanlar sayılır.</small>
        {collections.length > 0 && (
          <div className="fld-grid two">
            <label className="fld">
              <span>Koleksiyon</span>
              <select value={colId} onChange={(e) => setColId(e.target.value)}>
                <option value="">Fark etmez</option>
                {collections.map((c) => <option key={c.id} value={c.id}>{c.name}</option>)}
              </select>
            </label>
            <label className="fld">
              <span>Koleksiyon durumu</span>
              <select value={colIn ? "in" : "out"} onChange={(e) => setColIn(e.target.value === "in")} disabled={!colId}>
                <option value="in">İçinde</option>
                <option value="out">Dışında</option>
              </select>
            </label>
          </div>
        )}
        {error && <p className="err" role="alert">{error}</p>}
        <div className="acts">
          <button type="button" className="ghost" onClick={onClose}>Vazgeç</button>
          <button type="submit" className="primary" disabled={busy || !name.trim()}>Kaydet</button>
        </div>
      </form>
    </Modal>
  );
}
