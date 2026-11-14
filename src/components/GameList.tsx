import { useEffect, useRef } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import type { Game } from "../lib/api";
import { currentEntry, isInstalled } from "../lib/library";
import { ABBR, ORDER } from "./GameCard";
import { missingExe, PLATFORM_NAME, verb as verbOf } from "./PlayButton";
import { formatPlaytime } from "./Stage";
import Cover from "./Cover";

const ROW_H = 64;

interface Props {
  games: Game[];
  selectedId: number | null;
  onSelect: (id: number) => void;
  onContext?: (g: Game, x: number, y: number) => void;
  onLaunch: (g: Game) => void;
}

export default function GameList({ games, selectedId, onSelect, onContext, onLaunch }: Props) {
  const scroller = useRef<HTMLDivElement>(null);
  const virt = useVirtualizer({
    count: games.length,
    getScrollElement: () => scroller.current,
    estimateSize: () => ROW_H,
    overscan: 8,
  });
  useEffect(() => virt.measure(), []); // eslint-disable-line react-hooks/exhaustive-deps

  return (
    <div className="gscroll lscroll" ref={scroller}>
      <div className="ginner" role="list" aria-label="Oyun listesi" style={{ height: virt.getTotalSize() }}>
        {virt.getVirtualItems().map((v) => {
          const g = games[v.index];
          const inst = isInstalled(g);
          const stores = ORDER.filter((p) => g.entries.some((e) => e.platform === p));
          const tip = stores.map((p) => PLATFORM_NAME[p]).join(", ");
          const minutes = g.entries.reduce((s, e) => s + (e.playtime_min ?? 0), 0);
          const cur = currentEntry(g);
          const verb = verbOf(cur);
          return (
            <div
              key={v.key}
              role="listitem"
              className={`lrow${g.id === selectedId ? " sel" : ""}${inst ? "" : " notinst"}`}
              style={{ height: ROW_H, transform: `translateY(${v.index * ROW_H}px)` }}
              onClick={() => onSelect(g.id)}
              draggable
              onDragStart={(e) => {
                e.dataTransfer.setData("application/x-hangar-game", String(g.id));
                e.dataTransfer.effectAllowed = "copy";
              }}
              onContextMenu={(e) => {
                if (!onContext) return;
                e.preventDefault();
                onContext(g, e.clientX, e.clientY);
              }}
            >
              <button className="lmain" aria-pressed={g.id === selectedId}>
                <span className="lcover"><Cover game={g} /></span>
                <span className="ltitle">{g.title}</span>
                <span className="pg" title={tip}>
                  {stores.slice(0, 3).map((p) => <b key={p} lang="en">{ABBR[p]}</b>)}
                  {stores.length > 3 && <b>+{stores.length - 3}</b>}
                </span>
                <span className="lstate">{inst ? "Kurulu" : "Kurulu değil"}</span>
                <span className="ltime">{formatPlaytime(minutes)}</span>
              </button>
              <button
                className="lplay"
                disabled={missingExe(cur)}
                aria-label={`${verb} ${g.title}`}
                onClick={(e) => {
                  e.stopPropagation();
                  onLaunch(g);
                }}
              >
                {verb}
              </button>
            </div>
          );
        })}
      </div>
    </div>
  );
}
