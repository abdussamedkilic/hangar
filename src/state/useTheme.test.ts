import { applyTheme } from "./useTheme";

test("invalid color is skipped", () => {
  const el = document.createElement("div");
  const w = applyTheme({ "--accent": "kırmızımsı", "--bg": "#000" }, el, (v) => v.startsWith("#"));
  expect(el.style.getPropertyValue("--bg")).toBe("#000");
  expect(el.style.getPropertyValue("--accent")).toBe("");
  expect(w).toEqual(['Tema: "accent" geçersiz renk']);
});

test("reapply clears previous overrides", () => {
  const el = document.createElement("div");
  const ok = () => true;
  applyTheme({ "--bg": "#000", "--fg": "#fff" }, el, ok);
  applyTheme({ "--fg": "#eee" }, el, ok);
  expect(el.style.getPropertyValue("--bg")).toBe("");
  expect(el.style.getPropertyValue("--fg")).toBe("#eee");
});
