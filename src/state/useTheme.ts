import { useCallback, useEffect, useRef } from "react";
import { api } from "../lib/api";

const VARS = ["--bg", "--panel", "--raise", "--line", "--fg", "--dim", "--accent", "--accent-alt", "--on-accent", "--installed"];

const JSON_NAME: Record<string, string> = {
  "--bg": "background", "--panel": "panel", "--raise": "raised", "--line": "border", "--fg": "text",
  "--dim": "textDim", "--accent": "accent", "--accent-alt": "accentAlt", "--on-accent": "onAccent", "--installed": "installed",
};

/** Clears the 10 theme variables, then writes the valid ones. Returns a warning per invalid value. */
export function applyTheme(colors: Record<string, string>, root: HTMLElement, supports: (v: string) => boolean): string[] {
  for (const v of VARS) root.style.removeProperty(v);
  const warnings: string[] = [];
  for (const [name, value] of Object.entries(colors)) {
    if (!VARS.includes(name)) continue;
    if (supports(value)) root.style.setProperty(name, value);
    else warnings.push(`Tema: "${JSON_NAME[name]}" geçersiz renk`);
  }
  return warnings;
}

const cssSupports = (v: string) => typeof CSS !== "undefined" && CSS.supports("color", v);

export function useTheme(onWarnings: (w: string[]) => void): { reload(): Promise<void>; reset(): Promise<void> } {
  const cb = useRef(onWarnings);
  cb.current = onWarnings;
  const last = useRef<string>("");

  const apply = useCallback((r: { colors: Record<string, string>; warnings: string[] }) => {
    const w = [...r.warnings, ...applyTheme(r.colors, document.documentElement, cssSupports)];
    const sig = JSON.stringify(w);
    if (sig !== last.current) {
      last.current = sig;
      cb.current(w);
    }
  }, []);

  const reload = useCallback(async () => {
    try {
      apply(await api.getTheme());
    } catch {
      /* theme is optional */
    }
  }, [apply]);

  const reset = useCallback(async () => {
    apply(await api.resetTheme());
  }, [apply]);

  useEffect(() => {
    void reload();
    const onFocus = () => void reload();
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, [reload]);

  return { reload, reset };
}
