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

pub fn default_file() -> String {
    let mut m = serde_json::Map::new();
    for ((name, _), val) in KEYS.iter().zip(DEFAULTS) {
        m.insert((*name).to_string(), json!(val));
    }
    m.insert(HELP_KEY.to_string(), json!(HELP));
    serde_json::to_string_pretty(&Value::Object(m)).expect("theme json")
}

/// Missing file: empty result, no warning.
pub fn read_file(path: &Path) -> ThemeRead {
    match std::fs::read_to_string(path) {
        Ok(t) => parse(&t),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => ThemeRead::default(),
        Err(e) => ThemeRead { colors: BTreeMap::new(), warnings: vec![format!("Tema dosyası okunamadı: {e}")] },
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
}
