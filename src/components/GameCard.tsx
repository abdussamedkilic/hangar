import type { Game, Platform } from "../lib/api";
import { isInstalled } from "../lib/library";
import { PLATFORM_NAME } from "./PlayButton";
import Cover from "./Cover";

export const ABBR: Record<Platform, string> = { steam: "STE", epic: "EPI", ubisoft: "UBI", gog: "GOG", riot: "RIO", local: "LOC" };

export const ORDER: Platform[] = ["steam", "epic", "gog", "ubisoft", "riot", "local"];
const MAX_BADGES = 3;

interface Props {
  game: Game;
  selected: boolean;
  onSelect: () => void;
  onContext?: (x: number, y: number) => void;
}

export default function GameCard({ game, selected, onSelect, onContext }: Props) {
  const stores = ORDER.filter((p) => game.entries.some((e) => e.platform === p));
  const shown = stores.slice(0, MAX_BADGES);
  const extra = stores.length - shown.length;
  const tip = [...game.entries]
    .sort((a, b) => ORDER.indexOf(a.platform) - ORDER.indexOf(b.platform))
    .map((e) => PLATFORM_NAME[e.platform] + (e.copy ? ` (${e.copy})` : ""))
    .join(", ");
  return (
    <button className={`card${selected ? " sel" : ""}${isInstalled(game) ? "" : " notinst"}`} onClick={onSelect} aria-pressed={selected}
      onContextMenu={(e) => { if (!onContext) return; e.preventDefault(); onContext(e.clientX, e.clientY); }}>
      <div className="badges">
        <span className="pg" title={tip}>
          {shown.map((p) => <b key={p} lang="en">{ABBR[p]}</b>)}
          {extra > 0 && <b>+{extra}</b>}
        </span>
        <span className="dot" />
      </div>
      <Cover game={game} />
      <span className="cn">{game.title}</span>
    </button>
  );
}
