import { useEffect, useRef, useState } from "react";
import type { Collection } from "../lib/api";
import type { Platform, Tab, tabCounts } from "../lib/library";

const PLAT: Array<[Platform, string]> = [["steam", "Steam"], ["epic", "Epic"], ["ubisoft", "Ubisoft"], ["gog", "GOG"], ["riot", "Riot"], ["local", "Yerel"]];

interface Props {
  tab: Tab;
  counts: ReturnType<typeof tabCounts>;
  /** Counts ignoring the installed filter; decides which store tabs exist. Defaults to `counts`. */
  presence?: ReturnType<typeof tabCounts>;
  collections: Collection[];
  installedOnly: boolean;
  renamingId?: number | null;
  onTab: (t: Tab) => void;
  onClearInstalled: () => void;
  onCreate: (name: string) => Promise<void>;
  onRename?: (id: number, name: string) => Promise<void>;
  onRenameCancel?: () => void;
  onTabContext: (c: Collection, x: number, y: number) => void;
  onDropGame?: (collectionId: number, gameId: number) => void;
  viewMode?: "grid" | "list";
  onViewMode?: (m: "grid" | "list") => void;
}

/** Text input that commits on Enter (non-empty), cancels on Escape or blur. */
function NameInput({ label, initial, onCommit, onCancel }: { label: string; initial: string; onCommit: (name: string) => void; onCancel: () => void }) {
  const ref = useRef<HTMLInputElement>(null);
  useEffect(() => ref.current?.focus(), []);
  return (
    <input
      ref={ref}
      className="tab-input"
      aria-label={label}
      defaultValue={initial}
      maxLength={60}
      onKeyDown={(e) => {
        if (e.key === "Enter") {
          const v = e.currentTarget.value.trim();
          if (v) onCommit(v);
        } else if (e.key === "Escape") onCancel();
      }}
      onBlur={onCancel}
    />
  );
}

const same = (a: Tab, b: Tab) =>
  a.kind === b.kind &&
  (a.kind === "all" || (a.kind === "platform" && b.kind === "platform" && a.platform === b.platform) || (a.kind === "collection" && b.kind === "collection" && a.id === b.id));

const GAME_MIME = "application/x-hangar-game";
/** Accepts only Hangar game drags; marks the drop as allowed. */
const accept = (e: React.DragEvent) => {
  if (!Array.from(e.dataTransfer.types).includes(GAME_MIME)) return false;
  e.preventDefault();
  e.dataTransfer.dropEffect = "copy";
  return true;
};

export default function TabStrip({ tab, counts, presence = counts, collections, installedOnly, renamingId = null, onTab, onClearInstalled, onCreate, onRename, onRenameCancel, onTabContext, onDropGame, viewMode, onViewMode }: Props) {
  const [adding, setAdding] = useState(false);
  const [overId, setOverId] = useState<number | null>(null);
  const btn = (key: string, t: Tab, label: string, n: number, extra?: React.ButtonHTMLAttributes<HTMLButtonElement> & { drop?: boolean }) => (
    <button key={key} role="tab" aria-selected={same(t, tab)} className={(same(t, tab) ? "tab on" : "tab") + (extra?.drop ? " drop-ok" : "")} onClick={() => onTab(t)} {...(({ drop: _d, ...rest }) => rest)(extra ?? {})}>
      {label}
      <span>{n}</span>
    </button>
  );
  const plats = PLAT.filter(([p]) => presence.platform[p] > 0);
  return (
    <div className="tabstrip">
      <div role="tablist" aria-label="Kütüphane sekmeleri">
        {btn("all", { kind: "all" }, "Tümü", counts.all)}
        {plats.map(([p, label]) => btn(p, { kind: "platform", platform: p }, label, counts.platform[p]))}
        {collections.length > 0 && <span role="separator" aria-orientation="vertical" className="tab-sep" />}
        {collections.map((c) =>
          renamingId === c.id ? (
            <NameInput key={c.id} label="Yeni ad" initial={c.name} onCommit={(v) => void onRename?.(c.id, v)} onCancel={() => onRenameCancel?.()} />
          ) : (
            btn(`c${c.id}`, { kind: "collection", id: c.id }, c.name, counts.collection[c.id] ?? 0, {
              drop: overId === c.id,
              onDragEnter: (e) => accept(e) && setOverId(c.id),
              onDragOver: (e) => accept(e) && setOverId(c.id),
              onDragLeave: () => setOverId((o) => (o === c.id ? null : o)),
              onDrop: (e) => {
                if (!accept(e)) return;
                const id = Number(e.dataTransfer.getData(GAME_MIME));
                setOverId(null);
                if (Number.isFinite(id)) onDropGame?.(c.id, id);
              },
              onContextMenu: (e) => {
                e.preventDefault();
                onTabContext(c, e.clientX, e.clientY);
              },
            })
          ),
        )}
      </div>
      {adding ? (
        <NameInput label="Koleksiyon adı" initial="" onCommit={(v) => { setAdding(false); void onCreate(v); }} onCancel={() => setAdding(false)} />
      ) : (
        <button className="tab-add" aria-label="Koleksiyon ekle" onClick={() => setAdding(true)}>+</button>
      )}
      {installedOnly && <button className="tab-chip" onClick={onClearInstalled}>Kurulu ✕</button>}
      {viewMode && onViewMode && (
        <div className="vtoggle" role="group" aria-label="Görünüm">
          <button aria-label="Kart görünümü" aria-pressed={viewMode === "grid"} onClick={() => onViewMode("grid")}>
            <svg viewBox="0 0 18 18" aria-hidden="true"><rect x="2" y="2" width="5.5" height="5.5" rx="1" /><rect x="10.5" y="2" width="5.5" height="5.5" rx="1" /><rect x="2" y="10.5" width="5.5" height="5.5" rx="1" /><rect x="10.5" y="10.5" width="5.5" height="5.5" rx="1" /></svg>
          </button>
          <button aria-label="Liste görünümü" aria-pressed={viewMode === "list"} onClick={() => onViewMode("list")}>
            <svg viewBox="0 0 18 18" aria-hidden="true"><path d="M2 4h14M2 9h14M2 14h14" strokeLinecap="round" /></svg>
          </button>
        </div>
      )}
    </div>
  );
}
