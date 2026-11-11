import type { Filter } from "../lib/library";

const LIB: Array<[Filter, string]> = [["all", "Tümü"], ["installed", "Kurulu"]];
const PLAT: Array<[Filter, string]> = [["steam", "Steam"], ["epic", "Epic"], ["ubisoft", "Ubisoft"], ["gog", "GOG"], ["riot", "Riot"], ["local", "Yerel"]];

const OPTIONAL: Filter[] = ["gog", "riot", "local"];

interface Props {
  filter: Filter;
  counts: Record<Filter, number>;
  onFilter: (f: Filter) => void;
  onSettings: () => void;
  onRescan: () => void;
  onAdd: () => void;
  rescanning: boolean;
}

export default function Sidebar({ filter, counts, onFilter, onSettings, onRescan, onAdd, rescanning }: Props) {
  const group = (title: string, items: Array<[Filter, string]>) => (
    <div className="grp">
      <h4>{title}</h4>
      {items.filter(([k]) => !OPTIONAL.includes(k) || counts[k] > 0).map(([k, label]) => (
        <button key={k} className={filter === k ? "on" : ""} aria-pressed={filter === k} onClick={() => onFilter(k)}>
          {label}
          <span>{counts[k]}</span>
        </button>
      ))}
    </div>
  );
  return (
    <aside>
      <div className="logo"><i />Hangar</div>
      {group("Kütüphane", LIB)}
      {group("Platformlar", PLAT)}
      <button className="add-btn" onClick={onAdd}>Oyun ekle</button>
      <button className="rescan-btn" onClick={onRescan} disabled={rescanning}>{rescanning ? "Taranıyor…" : "Yenile"}</button>
      <button className="settings-btn" onClick={onSettings}>Ayarlar</button>
    </aside>
  );
}
