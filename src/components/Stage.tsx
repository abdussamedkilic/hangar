import { useRef, useState } from "react";
import gsap from "gsap";
import { useGSAP } from "@gsap/react";
import type { Game, Platform } from "../lib/api";
import { reducedMotion } from "../lib/motion";
import Cover, { placeholderBg } from "./Cover";
import PlayButton, { PLATFORM_NAME } from "./PlayButton";
import { convertFileSrc } from "@tauri-apps/api/core";
import { isInstalled } from "../lib/library";

gsap.registerPlugin(useGSAP);

const DAY = 86400;

export function formatPlaytime(min: number): string {
  if (min <= 0) return "—";
  return min < 60 ? `${min} dk` : `${Math.round(min / 60)} sa`;
}

export function formatLast(ts: number | null, now = Date.now() / 1000): string {
  if (ts === null) return "—";
  const d = Math.max(0, now - ts);
  if (d < DAY) return "Bugün";
  const days = Math.floor(d / DAY);
  if (days < 7) return `${days} gün önce`;
  if (days < 30) return `${Math.floor(days / 7)} hafta önce`;
  if (days < 365) return `${Math.floor(days / 30)} ay önce`;
  return `${Math.floor(days / 365)} yıl önce`;
}

export function formatSize(bytes: number): string {
  if (bytes <= 0) return "—";
  const gb = bytes / 1024 ** 3;
  return gb >= 1 ? `${gb.toFixed(gb >= 10 ? 0 : 1)} GB` : `${Math.round(bytes / 1024 ** 2)} MB`;
}

const backdropStyle = (g: Game) =>
  g.cover_path ? { backgroundImage: `url("${convertFileSrc(g.cover_path)}")` } : { background: placeholderBg(g.title) };

interface Props {
  game: Game | null;
  onLaunch: (p: Platform) => void;
  onChoose: (p: Platform) => void;
  onSplit?: (g: Game) => void;
}

export default function Stage({ game, onLaunch, onChoose, onSplit }: Props) {
  const [more, setMore] = useState(false);
  const root = useRef<HTMLElement>(null);
  const [bd, setBd] = useState<{ cur: Game | null; prev: Game | null }>({ cur: game, prev: null });
  if (game && bd.cur?.id !== game.id) setBd({ cur: game, prev: bd.cur });

  const id = game?.id;
  useGSAP(
    () => {
      if (id === undefined) return;
      if (reducedMotion()) return setBd((s) => ({ ...s, prev: null }));
      gsap.from(".st-l > *", { y: 14, opacity: 0, duration: 0.45, ease: "power3.out", stagger: 0.05, clearProps: "all" });
      gsap.from(".bd-cur", { opacity: 0, duration: 0.4, clearProps: "opacity", onComplete: () => setBd((s) => ({ ...s, prev: null })) });
    },
    { scope: root, dependencies: [id] },
  );

  if (!game) {
    return (
      <section className="stage empty" ref={root}>
        <p>Bu filtreyle eşleşen oyun yok.</p>
      </section>
    );
  }

  const installedEntries = game.entries.filter((e) => e.installed);
  const playtime = game.entries.reduce((s, e) => s + (e.playtime_min ?? 0), 0);
  const last = Math.max(0, game.last_launched ?? 0, ...game.entries.map((e) => e.last_played ?? 0)) || null;
  const size = installedEntries.reduce((s, e) => s + (e.size_bytes ?? 0), 0);

  return (
    <section className="stage" ref={root}>
      <div className="bd-wrap">
        {bd.prev && <div key={`p${bd.prev.id}`} className="bd" style={backdropStyle(bd.prev)} />}
        <div key={`c${game.id}`} className="bd bd-cur" style={backdropStyle(game)} />
      </div>
      <div className="st-l">
        <span className="eb">
          {isInstalled(game) ? "Hazır" : "Kurulu değil"} ·{" "}
          {game.entries.map((e, i) => (
            <span key={e.platform} lang="en">{i ? " + " : ""}{PLATFORM_NAME[e.platform]}</span>
          ))}
        </span>
        <h1 lang="en">{game.title}</h1>
        <div className="kv">
          <div><small>Oynama</small><b>{formatPlaytime(playtime)}</b></div>
          <div><small>Son</small><b>{formatLast(last)}</b></div>
          <div><small>Boyut</small><b>{formatSize(size)}</b></div>
        </div>
        <div className="acts-row">
          <PlayButton game={game} onLaunch={onLaunch} onChoose={onChoose} />
          {game.entries.length >= 2 && onSplit && (
            <div className="more">
              <button className="more-btn" aria-label="Diğer işlemler" aria-haspopup="menu" aria-expanded={more} onClick={() => setMore((o) => !o)}>⋯</button>
              {more && (
                <div className="play-menu" role="menu">
                  <button role="menuitem" onClick={() => { setMore(false); onSplit(game); }}>Bu kayıtları ayır</button>
                </div>
              )}
            </div>
          )}
        </div>
      </div>
      <Cover game={game} />
    </section>
  );
}
