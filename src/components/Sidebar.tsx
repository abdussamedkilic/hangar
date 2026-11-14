import type { ReactNode } from "react";
import type { ActiveFilter, SavedFilter } from "../lib/library";
import { AddIcon, FilterPlusIcon, CollapseIcon, ExpandIcon, FilterIcon, InstalledIcon, RefreshIcon, SettingsIcon } from "./icons";

interface Props {
  collapsed: boolean;
  onToggleCollapsed: () => void;
  filter: ActiveFilter;
  savedFilters: SavedFilter[];
  counts: Record<string, number>;
  onFilter: (f: ActiveFilter) => void;
  onNewFilter: () => void;
  onFilterContext: (f: SavedFilter, x: number, y: number) => void;
  onAdd: () => void;
  onRescan: () => void;
  onSettings: () => void;
  rescanning: boolean;
}

const initial = (name: string) => ([...name.trim()][0] ?? "?").toLocaleUpperCase("tr");

export default function Sidebar({ collapsed, onToggleCollapsed, filter, savedFilters, counts, onFilter, onNewFilter, onFilterContext, onAdd, onRescan, onSettings, rescanning }: Props) {
  const installedOn = filter.kind === "installed";
  // Collapsed: the visible label is gone, so every button keeps its name in aria-label + title.
  const tip = (label: string) => ({ "aria-label": label, title: label });
  const btn = (label: string, icon: ReactNode, onClick: () => void, extra?: { disabled?: boolean; cls?: string }) => (
    <button className={`sb-btn ${extra?.cls ?? ""}`} {...tip(label)} onClick={onClick} disabled={extra?.disabled}>
      {icon}
      {!collapsed && <span className="lbl">{label}</span>}
    </button>
  );

  return (
    <aside className={collapsed ? "collapsed" : ""}>
      <div className="sb-head">
        <div className="logo"><i />{!collapsed && "Hangar"}</div>
        <button className="sb-toggle" {...tip(collapsed ? "Menüyü genişlet" : "Menüyü daralt")} aria-expanded={!collapsed} onClick={onToggleCollapsed}>
          {collapsed ? <ExpandIcon /> : <CollapseIcon />}
        </button>
      </div>
      <nav className="grp" aria-label="Filtreler">
        {!collapsed && <h4>Filtre</h4>}
        <button
          className={`sb-btn ${installedOn ? "on" : ""}`}
          {...tip("Kurulu")}
          aria-pressed={installedOn}
          onClick={() => onFilter(installedOn ? { kind: "none" } : { kind: "installed" })}
        >
          <InstalledIcon />
          {!collapsed && <span className="lbl">Kurulu</span>}
          {!collapsed && <span className="cnt">{counts.installed ?? 0}</span>}
        </button>
        {savedFilters.map((f) => {
          const on = filter.kind === "saved" && filter.id === f.id;
          return (
            <button
              key={f.id}
              className={`sb-btn ${on ? "on" : ""}`}
              {...tip(f.name)}
              aria-pressed={on}
              onClick={() => onFilter(on ? { kind: "none" } : { kind: "saved", id: f.id })}
              onContextMenu={(e) => {
                e.preventDefault();
                onFilterContext(f, e.clientX, e.clientY);
              }}
            >
              {collapsed ? <span className="ini" aria-hidden="true">{initial(f.name)}</span> : <FilterIcon />}
              {!collapsed && <span className="lbl">{f.name}</span>}
              {!collapsed && <span className="cnt">{counts[String(f.id)] ?? 0}</span>}
            </button>
          );
        })}
        {btn("Yeni filtre", <FilterPlusIcon />, onNewFilter, { cls: "sb-new" })}
      </nav>
      <div className="sb-foot">
        {btn("Oyun ekle", <AddIcon />, onAdd)}
        {btn(rescanning ? "Taranıyor…" : "Yenile", <span className={rescanning ? "spin" : ""}><RefreshIcon /></span>, onRescan, { disabled: rescanning })}
        {btn("Ayarlar", <SettingsIcon />, onSettings)}
      </div>
    </aside>
  );
}
