import type { Game, Platform } from "../lib/api";
import { isInstalled } from "../lib/library";
import Cover from "./Cover";

const ABBR: Record<Platform, string> = { steam: "STE", epic: "EPI", ubisoft: "UBI" };

interface Props {
  game: Game;
  selected: boolean;
  onSelect: () => void;
  onContext?: (x: number, y: number) => void;
}

export default function GameCard({ game, selected, onSelect, onContext }: Props) {
  return (
    <button className={`card${selected ? " sel" : ""}${isInstalled(game) ? "" : " notinst"}`} onClick={onSelect} aria-pressed={selected}
      onContextMenu={(e) => { if (!onContext) return; e.preventDefault(); onContext(e.clientX, e.clientY); }}>
      <div className="badges">
        <span className="pg">{game.entries.map((e) => <b key={e.platform + ":" + e.platform_id} lang="en">{ABBR[e.platform]}</b>)}</span>
        <span className="dot" />
      </div>
      <Cover game={game} />
      <span className="cn">{game.title}</span>
    </button>
  );
}
