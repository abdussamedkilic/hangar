import { useMemo, useState } from "react";
import type { Game } from "../lib/api";
import { view } from "../lib/library";
import { ABBR, ORDER } from "./GameCard";
import Modal from "./Modal";

interface Props {
  source: Game;
  games: Game[];
  onPick: (g: Game) => void;
  onClose: () => void;
}

export default function MergePicker({ source, games, onPick, onClose }: Props) {
  const [q, setQ] = useState("");
  const list = useMemo(() => view(games, "all", q, "title").filter((g) => g.id !== source.id), [games, q, source.id]);
  return (
    <Modal label="Şununla birleştir" onClose={onClose}>
      <h2>Şununla birleştir</h2>
      <p className="hint" lang="en">{source.title}</p>
      <input className="pick-q" value={q} onChange={(e) => setQ(e.target.value)} placeholder="Oyun ara" aria-label="Oyun ara" />
      <ul className="pick-list">
        {list.map((g) => (
          <li key={g.id}><button onClick={() => onPick(g)}>
            <span lang="en">{g.title}</span>{" "}
            <small className="pick-stores">{ORDER.filter((p) => g.entries.some((e) => e.platform === p)).map((p) => ABBR[p]).join(" ")}</small>
          </button></li>
        ))}
        {list.length === 0 && <li className="none">Eşleşen oyun yok.</li>}
      </ul>
      <div className="acts"><button className="ghost" onClick={onClose}>Vazgeç</button></div>
    </Modal>
  );
}
