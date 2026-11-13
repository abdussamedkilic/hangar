use crate::model::{Platform, PlatformEntry};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Override {
    pub a: (Platform, String),
    pub b: (Platform, String),
    pub merged: bool,
}

const SUFFIXES: [&str; 13] = [
    "ultimate edition",
    "definitive edition",
    "game of the year edition",
    "goty",
    "deluxe edition",
    "complete edition",
    "remastered",
    "director's cut",
    "enhanced edition",
    "special edition",
    "gold edition",
    "anniversary edition",
    "amazon prime",
];

fn clean(s: &str) -> String {
    let mut out = String::new();
    for c in s.to_lowercase().chars() {
        match c {
            '\u{307}' | '™' | '®' | '©' => {}
            'ı' => out.push('i'),
            'ş' => out.push('s'),
            'ğ' => out.push('g'),
            'ü' => out.push('u'),
            'ö' => out.push('o'),
            'ç' => out.push('c'),
            c if c.is_alphanumeric() => out.push(c),
            _ => out.push(' '),
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Normalizes a game title into a grouping key. Steps, in order (Task 10 mirrors these in TS):
/// 1. Lowercase (Unicode `to_lowercase`; "İ" becomes "i" + U+0307).
/// 2. Transliterate: U+0307 dropped, `ı`->`i`, `ş`->`s`, `ğ`->`g`, `ü`->`u`, `ö`->`o`, `ç`->`c`.
/// 3. Remove `™`, `®`, `©`.
/// 4. Every remaining non-alphanumeric char becomes a space (so "director's" -> "director s").
///    "Alphanumeric" is Unicode-aware (Rust `char::is_alphanumeric`; in TS use `[\p{L}\p{N}]`).
///    Note `™ ® ©` are deleted in step 3, not turned into spaces.
/// 5. Collapse whitespace runs to one space and trim.
/// 6. Repeatedly strip a trailing edition suffix, then trim. Suffixes (each normalized with steps
///    1-5 before comparing): `ultimate edition`, `definitive edition`,
///    `game of the year edition`, `goty`, `deluxe edition`, `complete edition`, `remastered`,
///    `director's cut`, `enhanced edition`, `special edition`, `gold edition`,
///    `anniversary edition`, `amazon prime`. Rule: the suffix must be trailing and a whole word
///    (preceded by a space), and is never stripped if the title would become empty.
pub fn normalize(title: &str) -> String {
    strip_suffixes(title).0
}

/// Runs the `normalize` suffix loop; returns the key and the dropped suffixes (outermost first).
fn strip_suffixes(title: &str) -> (String, Vec<&'static str>) {
    let mut t = clean(title);
    let mut dropped = Vec::new();
    loop {
        let hit = SUFFIXES.iter().map(|s| (*s, clean(s))).find(|(_, s)| {
            t.strip_suffix(s.as_str())
                .is_some_and(|rest| rest.ends_with(' '))
        });
        match hit {
            Some((raw, s)) => {
                t.truncate(t.len() - s.len());
                dropped.push(raw);
            }
            None => break,
        }
        t = t.trim().to_string();
    }
    (t, dropped)
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
pub struct Edition {
    pub label: String,
    pub copy: Option<String>,
}

fn edition_label(suffix: &str) -> Option<&'static str> {
    Some(match suffix {
        "remastered" => "Remastered",
        "complete edition" => "Complete Edition",
        "goty" | "game of the year edition" => "GOTY",
        "definitive edition" => "Definitive Edition",
        "ultimate edition" => "Ultimate Edition",
        "deluxe edition" => "Deluxe Edition",
        "director's cut" => "Director's Cut",
        "enhanced edition" => "Enhanced Edition",
        "special edition" => "Special Edition",
        "gold edition" => "Gold Edition",
        "anniversary edition" => "Anniversary Edition",
        _ => return None,
    })
}

/// Edition label (first dropped edition suffix, else "Orijinal") and copy marker (Prime).
pub fn edition_of(title: &str) -> Edition {
    let dropped = strip_suffixes(title).1;
    let copy = dropped.contains(&"amazon prime").then(|| "Prime".to_string());
    let label = dropped
        .iter()
        .find_map(|s| edition_label(s))
        .unwrap_or("Orijinal")
        .to_string();
    Edition { label, copy }
}

/// Readable base title: marks removed and the suffixes `normalize` drops trimmed off the end.
pub fn display_base(title: &str) -> String {
    let stripped: String = title.chars().filter(|c| !matches!(c, '™' | '®' | '©')).collect();
    let target = normalize(title);
    let words: Vec<&str> = stripped.split_whitespace().collect();
    for k in (1..=words.len()).rev() {
        let prefix = words[..k].join(" ");
        if clean(&prefix) == target {
            let r = prefix.trim_end_matches(|c: char| matches!(c, '-' | ':' | ',') || c.is_whitespace());
            if !r.is_empty() {
                return r.to_string();
            }
            break;
        }
    }
    title.trim().to_string()
}

/// Groups entries by group key (normalized title, plus `|edition` unless Original). Overrides are order-independent: all `merged=true` overrides
/// are unioned first (set key = lowest member norm), then `merged=false` splits are evaluated
/// against that snapshot; a split moves only `b` (to key `<norm_b>#<platform>:<id>`).
pub fn group(
    entries: Vec<PlatformEntry>,
    overrides: &[Override],
) -> Vec<(String, Vec<PlatformEntry>)> {
    let editions: Vec<Edition> = entries.iter().map(|e| edition_of(&e.title)).collect();
    // Group key: one card = one build. Remastered, Complete Edition, ... are separate cards; the
    // Original keeps the plain normalized title (so existing games rows survive).
    let norms: Vec<String> = entries
        .iter()
        .zip(&editions)
        .map(|(e, ed)| {
            let n = normalize(&e.title);
            if ed.label == "Orijinal" {
                n
            } else {
                format!("{n}|{}", ed.label)
            }
        })
        .collect();
    // Local entries are never auto-merged: they keep their own key. Same-store copies share a card.
    let dup: Vec<bool> = entries.iter().map(|e| e.platform == Platform::Local).collect();
    let eff: Vec<String> = (0..entries.len())
        .map(|i| {
            if dup[i] {
                format!("{}#{}:{}", norms[i], entries[i].platform.as_str(), entries[i].platform_id)
            } else {
                norms[i].clone()
            }
        })
        .collect();
    let idx: HashMap<(Platform, &str), usize> = entries
        .iter()
        .enumerate()
        .map(|(i, e)| ((e.platform, e.platform_id.as_str()), i))
        .collect();
    let find = |k: &(Platform, String)| idx.get(&(k.0, k.1.as_str())).copied();

    // Order-independent: union all merges first, then apply splits.
    let mut parent: Vec<usize> = (0..entries.len()).collect();
    fn root(p: &mut [usize], mut x: usize) -> usize {
        while p[x] != x {
            p[x] = p[p[x]];
            x = p[x];
        }
        x
    }
    // Seed: link non-dup entries whose norms are equal, so merging an auto-grouped game keeps its group.
    let mut first: HashMap<&str, usize> = HashMap::new();
    for i in 0..entries.len() {
        if dup[i] {
            continue;
        }
        match first.get(norms[i].as_str()) {
            Some(&f) => {
                let (ra, rb) = (root(&mut parent, i), root(&mut parent, f));
                parent[ra] = rb;
            }
            None => {
                first.insert(norms[i].as_str(), i);
            }
        }
    }
    for o in overrides.iter().filter(|o| o.merged) {
        if let (Some(a), Some(b)) = (find(&o.a), find(&o.b)) {
            let (ra, rb) = (root(&mut parent, a), root(&mut parent, b));
            parent[ra] = rb;
        }
    }
    // Set key = lowest norm among members.
    let mut set_key: HashMap<usize, &str> = HashMap::new();
    for (i, n) in eff.iter().enumerate() {
        let r = root(&mut parent, i);
        let k = set_key.entry(r).or_insert(n.as_str());
        if n.as_str() < *k {
            *k = n.as_str();
        }
    }
    let mut keys: Vec<String> = (0..entries.len())
        .map(|i| set_key[&root(&mut parent, i)].to_string())
        .collect();
    // Splits are decided against a snapshot of the post-merge keys, then applied together.
    let snapshot = keys.clone();
    for o in overrides.iter().filter(|o| !o.merged) {
        if let (Some(a), Some(b)) = (find(&o.a), find(&o.b)) {
            if snapshot[a] == snapshot[b] {
                keys[b] = format!("{}#{}:{}", norms[b], o.b.0.as_str(), o.b.1);
            }
        }
    }
    let mut groups: BTreeMap<String, Vec<PlatformEntry>> = BTreeMap::new();
    for (e, k) in entries.into_iter().zip(keys) {
        groups.entry(k).or_default().push(e);
    }
    groups.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(platform: Platform, id: &str, title: &str) -> PlatformEntry {
        PlatformEntry {
            platform,
            platform_id: id.into(),
            title: title.into(),
            installed: false,
            install_path: None,
            size_bytes: None,
            last_played: None,
            playtime_min: None,
            launch_uri: String::new(),
            install_uri: None,
            launch_args: vec![],
            cover_url: None,
            edition: None,
            copy: None,
        }
    }

    #[test]
    fn normalize_strips_marks() {
        assert_eq!(normalize("DEATH STRANDING™"), normalize("Death Stranding"));
        assert_eq!(normalize("Control: Ultimate Edition"), "control");
        assert_eq!(normalize("Hades®"), "hades");
        assert_eq!(normalize("Cyberpunk 2077"), "cyberpunk 2077");
        assert_eq!(normalize("Baldur's Gate 3"), "baldur s gate 3");
        assert_eq!(normalize("Doom - Director's Cut"), "doom");
        assert_eq!(
            normalize("Game of the Year Edition"),
            "game of the year edition"
        );
    }

    #[test]
    fn normalize_turkish_stable() {
        assert_eq!(normalize("İstanbul Kıyamet"), "istanbul kiyamet");
    }

    #[test]
    fn groups_same_title_across_platforms() {
        let g = group(
            vec![
                e(Platform::Steam, "1", "Hades"),
                e(Platform::Epic, "x", "Hades"),
            ],
            &[],
        );
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].0, "hades");
        assert_eq!(g[0].1.len(), 2);
    }

    #[test]
    fn override_split() {
        let o = Override {
            a: (Platform::Steam, "1".into()),
            b: (Platform::Epic, "x".into()),
            merged: false,
        };
        let g = group(
            vec![
                e(Platform::Steam, "1", "Hades"),
                e(Platform::Epic, "x", "Hades"),
            ],
            &[o],
        );
        assert_eq!(g.len(), 2);
        assert_eq!(g[0].0, "hades");
        assert_eq!(g[1].0, "hades#epic:x");
    }

    #[test]
    fn merge_keeps_auto_group() {
        let o = Override {
            a: (Platform::Steam, "1".into()),
            b: (Platform::Ubisoft, "u".into()),
            merged: true,
        };
        let g = group(
            vec![
                e(Platform::Steam, "1", "Hades"),
                e(Platform::Epic, "x", "Hades"),
                e(Platform::Ubisoft, "u", "Alpha"),
            ],
            &[o],
        );
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].1.len(), 3);
    }

    #[test]
    fn same_platform_equal_norm_grouped() {
        let g = group(
            vec![
                e(Platform::Steam, "2", "Hades"),
                e(Platform::Steam, "1", "Hades"),
                e(Platform::Epic, "x", "Hades"),
            ],
            &[],
        );
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].0, "hades");
        assert_eq!(g[0].1.len(), 3);
        let es = || vec![e(Platform::Steam, "2", "Hades"), e(Platform::Steam, "1", "Hades")];
        let o = Override {
            a: (Platform::Steam, "1".into()),
            b: (Platform::Steam, "2".into()),
            merged: false,
        };
        let g = group(es(), &[o]);
        assert_eq!(g.len(), 2);
        assert!(g.iter().any(|(k, _)| k == "hades#steam:2"));
    }

    #[test]
    fn local_never_auto_merges() {
        let g = group(
            vec![e(Platform::Steam, "1", "Hades"), e(Platform::Local, "l1", "Hades")],
            &[],
        );
        assert_eq!(g.len(), 2);
    }

    #[test]
    fn local_merges_with_explicit_override() {
        let o = Override {
            a: (Platform::Steam, "1".into()),
            b: (Platform::Local, "l1".into()),
            merged: true,
        };
        let g = group(
            vec![e(Platform::Steam, "1", "Hades"), e(Platform::Local, "l1", "Hades")],
            &[o],
        );
        assert_eq!(g.len(), 1);
    }

    #[test]
    fn override_merge() {
        let o = Override {
            a: (Platform::Steam, "1".into()),
            b: (Platform::Epic, "x".into()),
            merged: true,
        };
        let g = group(
            vec![
                e(Platform::Steam, "1", "Hades"),
                e(Platform::Epic, "x", "Hades II"),
            ],
            &[o],
        );
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].0, "hades");
        assert_eq!(g[0].1.len(), 2);
    }

    #[test]
    fn merge_chain_is_order_independent() {
        let es = || {
            vec![
                e(Platform::Steam, "1", "Zed"),
                e(Platform::Epic, "x", "Alpha"),
                e(Platform::Ubisoft, "u", "Mid"),
            ]
        };
        let m = |a: (Platform, &str), b: (Platform, &str)| Override {
            a: (a.0, a.1.into()),
            b: (b.0, b.1.into()),
            merged: true,
        };
        let ab = m((Platform::Steam, "1"), (Platform::Epic, "x"));
        let bc = m((Platform::Epic, "x"), (Platform::Ubisoft, "u"));
        let g1 = group(es(), &[ab.clone(), bc.clone()]);
        let g2 = group(es(), &[bc, ab]);
        assert_eq!(g1, g2);
        assert_eq!(g1.len(), 1);
        assert_eq!(g1[0].0, "alpha");
        assert_eq!(g1[0].1.len(), 3);
    }

    #[test]
    fn normalize_new_suffixes() {
        assert_eq!(normalize("BioShock™ Remastered - Amazon Prime"), "bioshock");
        assert_eq!(normalize("Hades - Amazon Prime"), "hades");
        assert_eq!(normalize("Baldur's Gate Enhanced Edition"), "baldur s gate");
        assert_eq!(normalize("BioShock 2"), "bioshock 2");
    }

    #[test]
    fn edition_labels() {
        assert_eq!(edition_of("BioShock"), Edition { label: "Orijinal".into(), copy: None });
        assert_eq!(edition_of("BioShock™ Remastered"), Edition { label: "Remastered".into(), copy: None });
        assert_eq!(edition_of("BioShock™ Remastered - Amazon Prime"), Edition { label: "Remastered".into(), copy: Some("Prime".into()) });
        assert_eq!(edition_of("BioShock Infinite: Complete Edition"), Edition { label: "Complete Edition".into(), copy: None });
        assert_eq!(edition_of("Hades - Amazon Prime"), Edition { label: "Orijinal".into(), copy: Some("Prime".into()) });
    }

    #[test]
    fn display_base_cases() {
        assert_eq!(display_base("BioShock™ Remastered - Amazon Prime"), "BioShock");
        assert_eq!(display_base("BioShock Infinite: Complete Edition"), "BioShock Infinite");
        assert_eq!(display_base("BioShock® 2"), "BioShock 2");
        assert_eq!(display_base("Death's Door"), "Death's Door");
    }

    #[test]
    fn display_base_never_empty() {
        assert_eq!(display_base("Remastered"), "Remastered");
    }

    fn bioshock_entries() -> Vec<PlatformEntry> {
        use Platform::*;
        vec![
            e(Steam, "7670", "BioShock"),
            e(Steam, "409710", "BioShock Remastered"),
            e(Gog, "2022341186", "BioShock™"),
            e(Gog, "1439656515", "BioShock™ Remastered"),
            e(Gog, "1157510446", "BioShock™ Remastered - Amazon Prime"),
            e(Epic, "e1", "BioShock Remastered"),
            e(Steam, "8850", "BioShock 2"),
            e(Steam, "409720", "BioShock 2 Remastered"),
            e(Gog, "1806891286", "BioShock™ 2"),
            e(Gog, "1482265668", "BioShock™ 2 Remastered"),
            e(Gog, "1271128013", "BioShock™ 2 Remastered - Amazon Prime"),
            e(Epic, "e2", "BioShock 2 Remastered"),
            e(Steam, "8870", "BioShock Infinite"),
            e(Gog, "1752654506", "BioShock Infinite Complete Edition"),
            e(Gog, "1680121572", "BioShock Infinite Complete Edition - Amazon Prime"),
            e(Epic, "e3", "BioShock Infinite: Complete Edition"),
        ]
    }

    #[test]
    fn bioshock_family() {
        let g = group(bioshock_entries(), &[]);
        let got: Vec<(&str, usize)> = g.iter().map(|(k, v)| (k.as_str(), v.len())).collect();
        assert_eq!(
            got,
            vec![
                ("bioshock", 2),
                ("bioshock 2", 2),
                ("bioshock 2|Remastered", 4),
                ("bioshock infinite", 1),
                ("bioshock infinite|Complete Edition", 3),
                ("bioshock|Remastered", 4),
            ]
        );
        // The Original card keeps the plain normalized key.
        assert_eq!(g[0].0, normalize("BioShock™"));
    }

    #[test]
    fn every_edition_suffix_is_its_own_card() {
        for s in SUFFIXES.iter().filter(|s| **s != "amazon prime") {
            let title = format!("Foo {s}");
            assert_ne!(edition_of(&title).label, "Orijinal", "{s}");
            let g = group(vec![e(Platform::Steam, "1", "Foo"), e(Platform::Epic, "x", &title)], &[]);
            assert_eq!(g.len(), 2, "{s}");
        }
    }

    #[test]
    fn same_store_duplicates_share_card() {
        let g = group(
            vec![
                e(Platform::Gog, "1449710114", "Mafia II: Definitive Edition"),
                e(Platform::Gog, "1943447848", "Mafia II: Definitive Edition"),
            ],
            &[],
        );
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].1.len(), 2);
    }

    #[test]
    fn split_override_still_wins() {
        let o = Override {
            a: (Platform::Steam, "409710".into()),
            b: (Platform::Gog, "1439656515".into()),
            merged: false,
        };
        let g = group(bioshock_entries(), &[o]);
        assert_eq!(g.len(), 7);
        let alone = g
            .iter()
            .find(|(_, es)| es.iter().any(|x| x.platform_id == "1439656515"))
            .unwrap();
        assert_eq!(alone.1.len(), 1);
        assert_eq!(alone.0, "bioshock|Remastered#gog:1439656515");
    }

    #[test]
    fn manual_merge_joins_editions() {
        let o = Override {
            a: (Platform::Steam, "7670".into()),
            b: (Platform::Steam, "409710".into()),
            merged: true,
        };
        let g = group(bioshock_entries(), &[o]);
        assert_eq!(g.len(), 5);
    }

    #[test]
    fn split_chain_is_order_independent() {
        let es = || {
            vec![
                e(Platform::Steam, "1", "Hades"),
                e(Platform::Epic, "x", "Hades"),
                e(Platform::Ubisoft, "u", "Hades"),
            ]
        };
        let sp = |a: (Platform, &str), b: (Platform, &str)| Override {
            a: (a.0, a.1.into()),
            b: (b.0, b.1.into()),
            merged: false,
        };
        let ab = sp((Platform::Steam, "1"), (Platform::Epic, "x"));
        let bc = sp((Platform::Epic, "x"), (Platform::Ubisoft, "u"));
        let g1 = group(es(), &[ab.clone(), bc.clone()]);
        let g2 = group(es(), &[bc, ab]);
        assert_eq!(g1, g2);
        assert_eq!(g1.len(), 3);
    }
}
