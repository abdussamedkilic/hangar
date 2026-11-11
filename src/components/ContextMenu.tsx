import { useEffect } from "react";

interface Props {
  x: number;
  y: number;
  items: Array<{ label: string; onSelect: () => void }>;
  onClose: () => void;
}

export default function ContextMenu({ x, y, items, onClose }: Props) {
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
    <div className="ctx" role="menu" style={{ left: x, top: y }}>
      {items.map((i) => (
        <button key={i.label} role="menuitem" onClick={() => { onClose(); i.onSelect(); }}>{i.label}</button>
      ))}
    </div>
  );
}
