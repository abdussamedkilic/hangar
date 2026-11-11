import { convertFileSrc } from "@tauri-apps/api/core";
import type { Game } from "../lib/api";

export function hue(s: string): number {
  let h = 0;
  for (const c of s) h = (h * 31 + c.charCodeAt(0)) % 360;
  return h;
}

export function placeholderBg(title: string): string {
  const a = hue(title), b = (a + 48) % 360;
  return `radial-gradient(120% 80% at 80% 10%,hsl(${b} 70% 60% / .9),transparent 60%),linear-gradient(160deg,hsl(${a} 55% 38%),hsl(${(a + 200) % 360} 45% 14%))`;
}

export default function Cover({ game }: { game: Pick<Game, "title" | "cover_path"> }) {
  return (
    <div className="cover" style={game.cover_path ? undefined : { background: placeholderBg(game.title) }}>
      {game.cover_path ? (
        <img src={convertFileSrc(game.cover_path)} alt={game.title} loading="lazy" draggable={false} />
      ) : (
        <span className="ct" lang="en">{game.title}</span>
      )}
    </div>
  );
}
