interface Props {
  installedOnly: boolean;
  installedCount: number;
  onToggleInstalled: () => void;
  onSettings: () => void;
  onRescan: () => void;
  onAdd: () => void;
  rescanning: boolean;
}

export default function Sidebar({ installedOnly, installedCount, onToggleInstalled, onSettings, onRescan, onAdd, rescanning }: Props) {
  return (
    <aside>
      <div className="logo"><i />Hangar</div>
      <div className="grp">
        <h4>Filtre</h4>
        <button className={installedOnly ? "on" : ""} aria-pressed={installedOnly} onClick={onToggleInstalled}>
          Kurulu
          <span>{installedCount}</span>
        </button>
      </div>
      <button className="add-btn" onClick={onAdd}>Oyun ekle</button>
      <button className="rescan-btn" onClick={onRescan} disabled={rescanning}>{rescanning ? "Taranıyor…" : "Yenile"}</button>
      <button className="settings-btn" onClick={onSettings}>Ayarlar</button>
    </aside>
  );
}
