use crate::model::{Platform, PlatformEntry};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Override {
    pub a: (Platform, String),
    pub b: (Platform, String),
    pub merged: bool,
}

const SUFFIXES: [&str; 8] = [
    "ultimate edition",
    "definitive edition",
    "game of the year edition",
    "goty",
    "deluxe edition",
    "complete edition",
    "remastered",
    "director's cut",
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
///    `director's cut`. Rule: the suffix must be trailing and a whole word (preceded by a space),
///    and is never stripped if the title would become empty.
pub fn normalize(title: &str) -> String {
    let mut t = clean(title);
    loop {
        let hit = SUFFIXES.iter().map(|s| clean(s)).find(|s| {
            t.strip_suffix(s.as_str())
                .is_some_and(|rest| rest.ends_with(' '))
        });
        match hit {
            Some(s) => t.truncate(t.len() - s.len()),
            None => break,
        }
        t = t.trim().to_string();
    }
    t
}

/// Groups entries by normalized title. Overrides are order-independent: all `merged=true` overrides
/// are unioned first (set key = lowest member norm), then `merged=false` splits are evaluated
/// against that snapshot; a split moves only `b` (to key `<norm_b>#<platform>:<id>`).
pub fn group(
    entries: Vec<PlatformEntry>,
    overrides: &[Override],
) -> Vec<(String, Vec<PlatformEntry>)> {
    let norms: Vec<String> = entries.iter().map(|e| normalize(&e.title)).collect();
    // Same-platform entries with an equal norm are never auto-grouped: the later one (by
    // platform_id) is a "dup" and keeps its own key. Explicit merge overrides may still join it.
    let mut order: Vec<usize> = (0..entries.len()).collect();
    order.sort_by(|&a, &b| {
        (entries[a].platform_id.as_str(), entries[a].platform.as_str())
            .cmp(&(entries[b].platform_id.as_str(), entries[b].platform.as_str()))
    });
    let mut seen: std::collections::HashSet<(&str, Platform)> = Default::default();
    let mut dup = vec![false; entries.len()];
    for &i in &order {
        dup[i] = !seen.insert((norms[i].as_str(), entries[i].platform));
    }
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
    fn same_platform_equal_norm_not_grouped() {
        let es = || {
            vec![
                e(Platform::Steam, "2", "Hades"),
                e(Platform::Steam, "1", "Hades"),
                e(Platform::Epic, "x", "Hades"),
            ]
        };
        let g = group(es(), &[]);
        assert_eq!(g.len(), 2);
        assert_eq!(g[0].0, "hades");
        assert_eq!(g[0].1.len(), 2);
        assert_eq!(g[1].0, "hades#steam:2");
        let o = Override {
            a: (Platform::Steam, "1".into()),
            b: (Platform::Steam, "2".into()),
            merged: true,
        };
        assert_eq!(group(es(), &[o]).len(), 1);
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
