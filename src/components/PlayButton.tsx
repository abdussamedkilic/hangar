import { useEffect, useRef, useState } from "react";
import type { Game, Platform } from "../lib/api";
import { defaultPlatform } from "../lib/library";

export const PLATFORM_NAME: Record<Platform, string> = { steam: "Steam", epic: "Epic", ubisoft: "Ubisoft", gog: "GOG", riot: "Riot", local: "Yerel" };

interface Props {
  game: Game;
  onLaunch: (p: Platform) => void;
  onChoose: (p: Platform) => void;
}

export default function PlayButton({ game, onLaunch, onChoose }: Props) {
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const current = defaultPlatform(game);
  const installed = game.entries.find((e) => e.platform === current)?.installed ?? false;
  const multi = game.entries.length > 1;

  useEffect(() => {
    if (!open) return;
    const down = (e: MouseEvent) => {
      if (!root.current?.contains(e.target as Node)) setOpen(false);
    };
    const key = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    document.addEventListener("mousedown", down);
    document.addEventListener("keydown", key);
    return () => {
      document.removeEventListener("mousedown", down);
      document.removeEventListener("keydown", key);
    };
  }, [open]);

  return (
    <div className="play" ref={root}>
      <button className={`play-main${multi ? "" : " solo"}`} onClick={() => onLaunch(current)}>
        {installed ? "Oyna" : "Kur"} · <span lang="en">{PLATFORM_NAME[current]}</span>
      </button>
      {multi && (
        <>
          <button className="play-alt" aria-label="Platform seç" aria-haspopup="menu" aria-expanded={open} onClick={() => setOpen((o) => !o)}>
            ▾
          </button>
          {open && (
            <div className="play-menu" role="menu">
              {game.entries.map((e) => (
                <button
                  key={e.platform + ":" + e.platform_id}
                  role="menuitem"
                  onClick={() => {
                    setOpen(false);
                    onChoose(e.platform);
                  }}
                >
                  {e.installed ? "Oyna" : "Kur"} · {PLATFORM_NAME[e.platform]}
                  {e.platform === current ? " ✓" : ""}
                </button>
              ))}
            </div>
          )}
        </>
      )}
    </div>
  );
}
