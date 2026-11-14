import { useEffect, useLayoutEffect, useRef, useState } from "react";

export type MenuItem = { label: string; onSelect?: () => void; checked?: boolean; items?: MenuItem[] };

interface Props {
  x: number;
  y: number;
  items: MenuItem[];
  onClose: () => void;
}

export function clampMenu(x: number, y: number, w: number, h: number, vw: number, vh: number): { x: number; y: number } {
  return { x: Math.max(0, Math.min(x, vw - w - 10)), y: Math.max(0, Math.min(y, vh - h - 10)) };
}

export default function ContextMenu({ x, y, items, onClose }: Props) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ x, y });
  const [open, setOpen] = useState<number | null>(null);

  useLayoutEffect(() => {
    const r = ref.current?.getBoundingClientRect();
    if (r) setPos(clampMenu(x, y, r.width, r.height, window.innerWidth, window.innerHeight));
  }, [x, y, items.length]);

  useEffect(() => {
    const down = (e: MouseEvent) => !(e.target as Element).closest?.(".ctx") && onClose();
    const key = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    document.addEventListener("mousedown", down);
    document.addEventListener("keydown", key);
    return () => {
      document.removeEventListener("mousedown", down);
      document.removeEventListener("keydown", key);
    };
  }, [onClose]);

  return (
    <div ref={ref} className="ctx" role="menu" style={{ left: pos.x, top: pos.y }}>
      {items.map((i, idx) =>
        i.items ? (
          <div key={`${idx}:${i.label}`} className="ctx-sub-wrap" onMouseEnter={() => setOpen(idx)}>
            <button
              role="menuitem"
              aria-haspopup="menu"
              aria-expanded={open === idx}
              onClick={() => setOpen(open === idx ? null : idx)}
              onKeyDown={(e) => {
                if (e.key === "ArrowRight" || e.key === "Enter") { e.preventDefault(); setOpen(idx); }
                if (e.key === "ArrowLeft") setOpen(null);
              }}
            >
              {i.label} ›
            </button>
            {open === idx && <SubMenu items={i.items} onClose={onClose} />}
          </div>
        ) : (
          <button key={`${idx}:${i.label}`} role="menuitem" onMouseEnter={() => setOpen(null)} onClick={() => { onClose(); i.onSelect?.(); }}>
            {i.label}
          </button>
        ),
      )}
    </div>
  );
}

function SubMenu({ items, onClose }: { items: MenuItem[]; onClose: () => void }) {
  const ref = useRef<HTMLDivElement>(null);
  const [style, setStyle] = useState<React.CSSProperties>({});
  useLayoutEffect(() => {
    const r = ref.current?.getBoundingClientRect();
    if (!r) return;
    const s: React.CSSProperties = {};
    if (r.right > window.innerWidth) { s.left = "auto"; s.right = "100%"; }
    if (r.bottom > window.innerHeight) s.top = Math.max(-r.top, -5 - (r.bottom - window.innerHeight) - 8);
    setStyle(s);
  }, []);
  return (
    <div ref={ref} className="ctx ctx-sub" role="menu" style={style}>
      {items.map((i, idx) => (
        <button
          key={`${idx}:${i.label}`}
          role={i.checked === undefined ? "menuitem" : "menuitemcheckbox"}
          aria-checked={i.checked === undefined ? undefined : i.checked}
          onClick={() => { onClose(); i.onSelect?.(); }}
        >
          {i.checked !== undefined && <span className="ctx-check" aria-hidden="true">{i.checked ? "✓" : ""}</span>}
          {i.label}
        </button>
      ))}
    </div>
  );
}
