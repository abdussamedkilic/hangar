import { useEffect, useRef, useState } from "react";
import type { EntryKey, Game, Platform, PlatformEntry } from "../lib/api";
import { currentEntry } from "../lib/library";

export const PLATFORM_NAME: Record<Platform, string> = { steam: "Steam", epic: "Epic", ubisoft: "Ubisoft", gog: "GOG", riot: "Riot", local: "Yerel" };

/** `Oyna|Kur · Mağaza (kopya) [· sürüm]`; the edition only when the card mixes editions. */
function labelText(e: PlatformEntry, showEdition: boolean): string {
  let t = `${e.installed ? "Oyna" : "Kur"} · ${PLATFORM_NAME[e.platform]}`;
  if (e.copy) t += ` (${e.copy})`;
  if (showEdition && e.edition) t += ` · ${e.edition}`;
  return t;
}

/** Turkish verb stays unmarked; store and edition names are `lang="en"`. */
function Label({ e, showEdition, nth }: { e: PlatformEntry; showEdition: boolean; nth?: number }) {
  let rest = PLATFORM_NAME[e.platform];
  if (e.copy) rest += ` (${e.copy})`;
  if (showEdition && e.edition) rest += ` · ${e.edition}`;
  return (
    <>
      {e.installed ? "Oyna" : "Kur"} · <span lang="en">{rest}</span>
      {nth ? ` · ${nth}. kopya` : ""}
    </>
  );
}

const keyOf = (e: PlatformEntry): EntryKey => [e.platform, e.platform_id];

interface Props {
  game: Game;
  onLaunch: (key: EntryKey) => void;
  onChoose: (key: EntryKey) => void;
}

export default function PlayButton({ game, onLaunch, onChoose }: Props) {
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const current = currentEntry(game);
  const showEdition = new Set(game.entries.map((e) => e.edition ?? "")).size > 1;
  const labels = game.entries.map((e) => labelText(e, showEdition));
  // Rows identical in store, copy, edition and store title are numbered in entries order.
  const nths = game.entries.map((e, i) => {
    const same = game.entries.filter((o, j) => j < i && labels[j] === labels[i] && o.title === e.title).length;
    return same > 0 ? same + 1 : 0;
  });
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
      <button className={`play-main${multi ? "" : " solo"}`} onClick={() => onLaunch(keyOf(current))}>
        <span><Label e={current} showEdition={showEdition} nth={nths[game.entries.indexOf(current)]} /></span>
      </button>
      {multi && (
        <>
          <button className="play-alt" aria-label="Sürüm ve platform seç" aria-haspopup="menu" aria-expanded={open} onClick={() => setOpen((o) => !o)}>
            ▾
          </button>
          {open && (
            <div className="play-menu" role="menu">
              {game.entries.map((e, i) => {
                const checked = e.platform === current.platform && e.platform_id === current.platform_id;
                const dup = labels.filter((l) => l === labels[i]).length > 1;
                return (
                  <button
                    key={e.platform + ":" + e.platform_id}
                    role="menuitemradio"
                    aria-checked={checked}
                    onClick={() => {
                      setOpen(false);
                      onChoose(keyOf(e));
                    }}
                  >
                    <span><Label e={e} showEdition={showEdition} nth={nths[i]} /></span>
                    {checked && <span aria-hidden="true"> ✓</span>}
                    {dup && <small className="alt-title" lang="en">{e.title}</small>}
                  </button>
                );
              })}
            </div>
          )}
        </>
      )}
    </div>
  );
}
