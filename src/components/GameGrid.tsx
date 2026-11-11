import { useEffect, useMemo, useRef, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import Lenis from "lenis";
import gsap from "gsap";
import { useGSAP } from "@gsap/react";
import type { Game } from "../lib/api";
import { reducedMotion } from "../lib/motion";
import GameCard from "./GameCard";

gsap.registerPlugin(useGSAP);

const MIN_W = 112, GAP = 12, NAME_H = 24;

interface Props {
  games: Game[];
  selectedId: number | null;
  onSelect: (id: number) => void;
  nudge?: boolean;
  onNudge?: () => void;
  onContext?: (g: Game, x: number, y: number) => void;
}

export default function GameGrid({ games, selectedId, onSelect, nudge, onNudge, onContext }: Props) {
  const items = useMemo<Array<Game | null>>(() => (nudge ? [...games, null] : games), [games, nudge]);
  const scroller = useRef<HTMLDivElement>(null);
  const inner = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(0);
  const animated = useRef(false);

  useEffect(() => {
    const el = scroller.current!;
    const cs = getComputedStyle(el);
    setWidth(el.clientWidth - (parseFloat(cs.paddingLeft) || 0) - (parseFloat(cs.paddingRight) || 0));
    const ro = new ResizeObserver(([e]) => setWidth(e.contentRect.width));
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  useEffect(() => {
    if (reducedMotion() || !scroller.current || !inner.current) return;
    const lenis = new Lenis({ wrapper: scroller.current, content: inner.current, autoRaf: true });
    return () => lenis.destroy();
  }, []);

  const avail = Math.max(0, width); // contentRect already excludes padding
  const cols = Math.max(1, Math.floor((avail + GAP) / (MIN_W + GAP)));
  const cardW = (avail - GAP * (cols - 1)) / cols;
  const rowH = Math.max(1, Math.round((cardW * 4) / 3 + NAME_H)) + GAP;
  const rows = Math.ceil(items.length / cols);

  const virt = useVirtualizer({
    count: rows,
    getScrollElement: () => scroller.current,
    estimateSize: () => rowH,
    overscan: 3,
  });
  useEffect(() => virt.measure(), [rowH]); // eslint-disable-line react-hooks/exhaustive-deps

  const ready = items.length > 0 && width > 0;
  useGSAP(
    () => {
      if (!ready || animated.current) return;
      animated.current = true;
      if (reducedMotion()) return;
      gsap.delayedCall(0.05, () =>
        gsap.from(".card", { y: 18, opacity: 0, duration: 0.5, ease: "power3.out", stagger: 0.025, clearProps: "all" }),
      );
    },
    { scope: scroller, dependencies: [ready] },
  );

  return (
    <div className="gscroll" ref={scroller}>
      <div className="ginner" ref={inner} style={{ height: virt.getTotalSize() }}>
        {virt.getVirtualItems().map((v) => (
          <div
            key={v.key}
            className="grow"
            style={{ top: 0, height: rowH - GAP, transform: `translateY(${v.index * rowH}px)`, gridTemplateColumns: `repeat(${cols}, minmax(0,1fr))` }}
          >
            {items.slice(v.index * cols, v.index * cols + cols).map((g) =>
              g ? (
                <GameCard key={g.id} game={g} selected={g.id === selectedId} onSelect={() => onSelect(g.id)} onContext={(x, y) => onContext?.(g, x, y)} />
              ) : (
                <button key="nudge" className="card nudge" onClick={onNudge}>
                  <span className="nudge-box">Kurulu olmayan Steam oyunlarını görmek için API anahtarı ekle</span>
                </button>
              ),
            )}
          </div>
        ))}
      </div>
    </div>
  );
}
