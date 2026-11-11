import { useEffect, useRef } from "react";
import type { Sort } from "../lib/library";

interface Props {
  query: string;
  onQuery: (q: string) => void;
  sort: Sort;
  onSort: (s: Sort) => void;
}

export default function SearchBar({ query, onQuery, sort, onSort }: Props) {
  const input = useRef<HTMLInputElement>(null);

  useEffect(() => {
    const h = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        input.current?.focus();
        input.current?.select();
      }
    };
    window.addEventListener("keydown", h);
    return () => window.removeEventListener("keydown", h);
  }, []);

  return (
    <div className="bar">
      <label className="s">
        <span aria-hidden>⌕</span>
        <input ref={input} placeholder="Kütüphanede ara" value={query} onChange={(e) => onQuery(e.target.value)} />
        <kbd>Ctrl K</kbd>
      </label>
      <label className="sort">
        Sırala:
        <select value={sort} onChange={(e) => onSort(e.target.value as Sort)}>
          <option value="recent">Son oynanan</option>
          <option value="title">Ada göre</option>
          <option value="playtime">Oynama süresi</option>
        </select>
      </label>
    </div>
  );
}
