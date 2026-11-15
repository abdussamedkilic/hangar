//! Theme file: JSON of colour overrides. Colour syntax is validated by the front end (`CSS.supports`).

use serde::Serialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::Path;

/// (JSON name, CSS variable)
pub const KEYS: [(&str, &str); 10] = [
    ("background", "--bg"),
    ("panel", "--panel"),
    ("raised", "--raise"),
    ("border", "--line"),
    ("text", "--fg"),
    ("textDim", "--dim"),
    ("accent", "--accent"),
    ("accentAlt", "--accent-alt"),
    ("onAccent", "--on-accent"),
    ("installed", "--installed"),
];

const HELP_KEY: &str = "_yardım";

const HELP: [&str; 10] = [
    "background: uygulamanın arka planı",
    "panel: kenar çubuğu ve panel zemini",
    "raised: yükseltilmiş yüzeyler (kartlar, menüler)",
    "border: ince çizgiler ve kenarlıklar",
    "text: ana yazı rengi",
    "textDim: soluk, ikincil yazı rengi",
    "accent: vurgu rengi (düğmeler, seçili öğeler)",
    "accentAlt: vurgu renginin koyu tonu (üzerine gelince)",
    "onAccent: vurgu renginin üzerindeki yazı rengi",
    "installed: kurulu oyun göstergesi",
];

/// Defaults mirror `src/styles/tokens.css`.
const DEFAULTS: [&str; 10] = [
    "#0f1318", "#151a21", "#1d242e", "#252d38", "#e8ebf0", "#8892a0", "#f2a93b", "#d6922c", "#1a1205", "#4fd18b",
];

#[derive(Serialize, Debug, PartialEq, Default)]
pub struct ThemeRead {
    /// CSS variable -> raw value.
    pub colors: BTreeMap<String, String>,
    pub warnings: Vec<String>,
}

pub fn parse(text: &str) -> ThemeRead {
    let mut out = ThemeRead::default();
    let value: Value = match serde_json::from_str(text.trim_start_matches('\u{feff}')) {
        Ok(v) => v,
        Err(e) => {
            out.warnings.push(format!("Tema dosyası okunamadı: {e}"));
            return out;
        }
    };
    let Some(map) = value.as_object() else {
        out.warnings.push("Tema dosyası okunamadı: kök bir nesne olmalı".to_string());
        return out;
    };
    for (name, v) in map {
        if name == HELP_KEY {
            continue;
        }
        let Some((_, var)) = KEYS.iter().find(|(k, _)| k == name) else {
            out.warnings.push(format!("Tema: bilinmeyen anahtar \"{name}\""));
            continue;
        };
        match v.as_str() {
            Some(s) => {
                out.colors.insert((*var).to_string(), s.to_string());
            }
            None => out.warnings.push(format!("Tema: \"{name}\" metin olmalı")),
        }
    }
    out
}

fn file_for(values: &[&str; 10]) -> String {
    let mut m = serde_json::Map::new();
    for ((name, _), val) in KEYS.iter().zip(values) {
        m.insert((*name).to_string(), json!(val));
    }
    m.insert(HELP_KEY.to_string(), json!(HELP));
    serde_json::to_string_pretty(&Value::Object(m)).expect("theme json")
}

pub fn default_file() -> String {
    file_for(&DEFAULTS)
}

/// Ready-made themes, values in `KEYS` order.
pub const PRESETS: [(&str, [&str; 10]); 4] = [
    ("Gece Mavisi", ["#0b1220", "#111a2e", "#18233b", "#24304a", "#e6ecf7", "#8794ad", "#4f8cff", "#3a73e0", "#06122b", "#3ddc97"]),
    ("Orman", ["#0e1510", "#141e16", "#1b281e", "#26362a", "#e5efe6", "#86a08b", "#7ccf6a", "#62b452", "#0c1a08", "#e9c46a"]),
    ("Gül", ["#170e13", "#1f141a", "#2a1b23", "#3a2631", "#f5e9ef", "#a8909c", "#ff5c8a", "#e2416f", "#2a0612", "#5fd3b3"]),
    ("Gündüz", ["#f4f5f7", "#ffffff", "#eceef2", "#d5d9e0", "#1a1f27", "#5d6673", "#d9731a", "#bb6110", "#ffffff", "#1f9d5a"]),
];

/// Writes each preset as `<name>.json` unless the file already exists (user edits are never overwritten).
pub fn seed_presets(themes_dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(themes_dir)?;
    for (name, values) in &PRESETS {
        let path = themes_dir.join(format!("{name}.json"));
        if !path.exists() {
            std::fs::write(path, file_for(values))?;
        }
    }
    Ok(())
}

/// Missing file: empty result, no warning.
pub fn read_file(path: &Path) -> ThemeRead {
    match std::fs::read_to_string(path) {
        Ok(t) => parse(&t),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => ThemeRead::default(),
        Err(e) => ThemeRead { colors: BTreeMap::new(), warnings: vec![format!("Tema dosyası okunamadı: {e}")] },
    }
}

pub const DEFAULT_NAME: &str = "Varsayılan";

const LEGACY_NAME: &str = "Özel";

/// Returns the trimmed name, or a Turkish error. Guards against names that are invalid file names on Windows.
pub fn validate_name(name: &str, existing: &[String]) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Tema adı boş olamaz".to_string());
    }
    if name.chars().count() > 40 {
        return Err("Tema adı en fazla 40 karakter olabilir".to_string());
    }
    if name.ends_with('.') || name.chars().any(|c| c.is_control() || r#"\/:*?"<>|"#.contains(c)) {
        return Err("Tema adında geçersiz karakter var".to_string());
    }
    let stem = name.split('.').next().unwrap_or("").trim_end().to_uppercase();
    let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.len() == 4
            && matches!(stem.as_bytes()[3], b'1'..=b'9'));
    if reserved {
        return Err("Bu ad kullanılamaz".to_string());
    }
    let lower = name.to_lowercase();
    if lower == DEFAULT_NAME.to_lowercase() || existing.iter().any(|e| e.to_lowercase() == lower) {
        return Err("Bu adda bir tema zaten var".to_string());
    }
    Ok(name.to_string())
}

/// Stems of `*.json` files, sorted case-insensitively. Missing dir: empty.
pub fn list_names(dir: &Path) -> Vec<String> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut names: Vec<String> = rd
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .filter_map(|e| {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()).map(|x| x.eq_ignore_ascii_case("json")) != Some(true) {
                return None;
            }
            p.file_stem().and_then(|s| s.to_str()).map(str::to_string)
        })
        .collect();
    names.sort_by_key(|n| n.to_lowercase());
    names
}

/// Moves a pre-themes `theme.json` into `themes/Özel.json` (or `Özel 2`, ...). Returns the new name, once.
pub fn migrate_legacy(app_dir: &Path) -> Option<String> {
    let old = app_dir.join("theme.json");
    if !old.is_file() {
        return None;
    }
    let dir = app_dir.join("themes");
    std::fs::create_dir_all(&dir).ok()?;
    let mut n = 1;
    loop {
        let name = if n == 1 { LEGACY_NAME.to_string() } else { format!("{LEGACY_NAME} {n}") };
        let target = dir.join(format!("{name}.json"));
        if !target.exists() {
            if std::fs::rename(&old, &target).is_err() {
                std::fs::copy(&old, &target).ok()?;
                std::fs::remove_file(&old).ok()?;
            }
            return Some(name);
        }
        n += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_matches_tokens_css() {
        let css = include_str!("../../../src/styles/tokens.css");
        let mut expected = BTreeMap::new();
        for (_, var) in KEYS {
            let prefix = format!("{var}:");
            let line = css
                .lines()
                .map(str::trim)
                .find(|l| l.starts_with(&prefix))
                .unwrap_or_else(|| panic!("{var} missing in tokens.css"));
            let val = line[prefix.len()..].trim().trim_end_matches(';').trim();
            expected.insert(var.to_string(), val.to_string());
        }
        let r = parse(&default_file());
        assert_eq!(r.colors, expected);
        assert!(r.warnings.is_empty());
    }

    #[test]
    fn unknown_key_and_non_string_warn() {
        let r = parse(r##"{"accent":"#fff","foo":"#000","text":5}"##);
        assert_eq!(r.colors, BTreeMap::from([("--accent".to_string(), "#fff".to_string())]));
        assert_eq!(r.warnings.len(), 2);
        assert!(r.warnings.iter().any(|w| w.contains("foo")));
        assert!(r.warnings.iter().any(|w| w.contains("text")));
    }

    #[test]
    fn broken_json() {
        let r = parse("{");
        assert!(r.colors.is_empty());
        assert_eq!(r.warnings.len(), 1);
        assert!(r.warnings[0].starts_with("Tema dosyası okunamadı"));
    }

    #[test]
    fn bom_is_ignored() {
        let r = parse("\u{feff}{\"accent\":\"#fff\"}");
        assert!(r.warnings.is_empty());
        assert_eq!(r.colors.get("--accent").map(String::as_str), Some("#fff"));
    }

    #[test]
    fn help_key_ignored() {
        let r = parse(r#"{"_yardım":["x"]}"#);
        assert!(r.warnings.is_empty());
    }

    fn tmp(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("hangar-theme-{tag}-{}-{:?}", std::process::id(), std::thread::current().id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn presets_parse_cleanly() {
        for (name, values) in &PRESETS {
            let r = parse(&file_for(values));
            assert!(r.warnings.is_empty(), "{name}");
            assert_eq!(r.colors.len(), 10, "{name}");
        }
    }

    #[test]
    fn seed_writes_presets_without_overwriting() {
        let d = tmp("seed").join("themes");
        seed_presets(&d).unwrap();
        assert_eq!(list_names(&d).len(), 4);
        std::fs::write(d.join("Orman.json"), "custom").unwrap();
        seed_presets(&d).unwrap();
        assert_eq!(std::fs::read_to_string(d.join("Orman.json")).unwrap(), "custom");
    }

    #[test]
    fn validate_trims_and_accepts() {
        assert_eq!(validate_name("  Gece ", &[]), Ok("Gece".to_string()));
    }

    #[test]
    fn validate_rejects() {
        let long = "a".repeat(41);
        for bad in ["", "   ", long.as_str(), "a/b", "x.", "CON", "nul", "varsayılan"] {
            assert!(validate_name(bad, &[]).is_err(), "{bad:?}");
        }
        assert!(validate_name("gece", &["Gece".to_string()]).is_err());
        assert!(validate_name("com1.txt", &[]).is_err());
        assert!(validate_name("a	b", &[]).is_err());
    }

    #[test]
    fn list_names_sorted_and_json_only() {
        let d = tmp("list");
        for f in ["b.json", "A.json", "note.txt"] {
            std::fs::write(d.join(f), "{}").unwrap();
        }
        assert_eq!(list_names(&d), vec!["A".to_string(), "b".to_string()]);
        assert!(list_names(&d.join("yok")).is_empty());
    }

    #[test]
    fn migrate_moves_once() {
        let d = tmp("mig");
        std::fs::write(d.join("theme.json"), "OLD").unwrap();
        assert_eq!(migrate_legacy(&d), Some("Özel".to_string()));
        assert_eq!(std::fs::read_to_string(d.join("themes/Özel.json")).unwrap(), "OLD");
        assert!(!d.join("theme.json").exists());
        assert_eq!(migrate_legacy(&d), None);
    }

    #[test]
    fn migrate_avoids_collision() {
        let d = tmp("migc");
        std::fs::create_dir_all(d.join("themes")).unwrap();
        std::fs::write(d.join("themes/Özel.json"), "X").unwrap();
        std::fs::write(d.join("theme.json"), "OLD").unwrap();
        assert_eq!(migrate_legacy(&d), Some("Özel 2".to_string()));
        assert_eq!(std::fs::read_to_string(d.join("themes/Özel.json")).unwrap(), "X");
    }
}
