use crate::db::Db;
use crate::merge::normalize;
use crate::model::{Game, Platform};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

const MAX_BYTES: u64 = 10 * 1024 * 1024;

/// Looks for Steam's cached portrait art: per-appid folder layout first, then the flat legacy one.
pub fn local_steam_cover(steam_root: &Path, appid: &str) -> Option<PathBuf> {
    let lc = steam_root.join("appcache").join("librarycache");
    [
        lc.join(appid).join("library_600x900.jpg"),
        lc.join(format!("{appid}_library_600x900.jpg")),
    ]
    .into_iter()
    .find(|p| p.is_file())
}

pub fn steam_cdn_url(appid: &str) -> String {
    format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{appid}/library_600x900_2x.jpg")
}

/// First store-search result whose normalized name equals `norm_title`; returns its appid.
pub fn parse_store_search(json: &str, norm_title: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    v.get("items")?.as_array()?.iter().find_map(|it| {
        let name = it.get("name")?.as_str()?;
        (normalize(name) == norm_title).then(|| it.get("id")?.as_u64().map(|i| i.to_string()))?
    })
}

fn get_bytes(url: &str, query: &[(&str, &str)]) -> Option<Vec<u8>> {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(10))
        .build();
    let mut req = agent.get(url);
    for (k, v) in query {
        req = req.query(k, v);
    }
    let resp = req.call().ok()?;
    let expected = resp
        .header("Content-Length")
        .and_then(|v| v.parse::<usize>().ok());
    let mut buf = Vec::new();
    resp.into_reader()
        .take(MAX_BYTES + 1)
        .read_to_end(&mut buf)
        .ok()?;
    if buf.is_empty() || buf.len() as u64 > MAX_BYTES || expected.is_some_and(|n| n != buf.len()) {
        return None;
    }
    Some(buf)
}

pub fn is_image(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0xFF, 0xD8, 0xFF]) || bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47])
}

fn find_appid(game: &Game) -> Option<String> {
    if let Some(e) = game.entries.iter().find(|e| e.platform == Platform::Steam) {
        return Some(e.platform_id.clone());
    }
    let body = get_bytes(
        "https://store.steampowered.com/api/storesearch/",
        &[("term", &game.title), ("l", "english"), ("cc", "US")],
    )?;
    parse_store_search(std::str::from_utf8(&body).ok()?, &normalize(&game.title))
}

/// Returns the cover path for `game`, fetching and caching it if needed. Any failure yields None.
pub fn ensure_cover(
    db: &Db,
    cache_dir: &Path,
    game: &Game,
    steam_root: Option<&Path>,
) -> Option<String> {
    if let Some(p) = &game.cover_path {
        if Path::new(p).is_file() {
            return Some(p.clone());
        }
    }
    let appid = find_appid(game)?;
    let local = steam_root
        .and_then(|r| local_steam_cover(r, &appid))
        .and_then(|p| std::fs::read(p).ok())
        .filter(|b| is_image(b));
    let bytes = local
        .or_else(|| get_bytes(&steam_cdn_url(&appid), &[]).filter(|b| is_image(b)))
        .or_else(|| {
            get_bytes(
                &format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{appid}/header.jpg"),
                &[],
            )
            .filter(|b| is_image(b))
        })?;
    std::fs::create_dir_all(cache_dir).ok()?;
    let dest = cache_dir.join(format!("{}.jpg", game.id));
    let tmp = cache_dir.join(format!("{}.jpg.tmp", game.id));
    if std::fs::write(&tmp, bytes).is_err() || std::fs::rename(&tmp, &dest).is_err() {
        let _ = std::fs::remove_file(&tmp);
        return None;
    }
    let path = dest.to_string_lossy().into_owned();
    db.set_cover(game.id, &path).ok()?;
    Some(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_search_exact_match() {
        let json = include_str!("../tests/fixtures/artwork/synthetic-storesearch-alan-wake.json");
        assert_eq!(
            parse_store_search(json, "alan wake 2").as_deref(),
            Some("1000108")
        );
        assert_eq!(
            parse_store_search(json, "alan wake").as_deref(),
            Some("108710")
        );
        assert_eq!(parse_store_search(json, "control"), None);
        assert_eq!(parse_store_search("not json", "x"), None);
    }

    #[test]
    fn local_cover_both_layouts() {
        let root = std::env::temp_dir().join(format!("hangar-art-{}", std::process::id()));
        let lc = root.join("appcache").join("librarycache");
        std::fs::create_dir_all(lc.join("10")).unwrap();
        std::fs::write(lc.join("10").join("library_600x900.jpg"), b"x").unwrap();
        std::fs::write(lc.join("20_library_600x900.jpg"), b"x").unwrap();
        assert_eq!(
            local_steam_cover(&root, "10"),
            Some(lc.join("10").join("library_600x900.jpg"))
        );
        assert_eq!(
            local_steam_cover(&root, "20"),
            Some(lc.join("20_library_600x900.jpg"))
        );
        assert_eq!(local_steam_cover(&root, "30"), None);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn is_image_magic() {
        assert!(is_image(&[0xFF, 0xD8, 0xFF, 0xE0, 1]));
        assert!(is_image(&[0x89, 0x50, 0x4E, 0x47, 0x0D]));
        assert!(!is_image(b"<html>404</html>"));
        assert!(!is_image(b""));
    }

    #[test]
    fn cdn_url_format() {
        assert_eq!(
            steam_cdn_url("620"),
            "https://cdn.cloudflare.steamstatic.com/steam/apps/620/library_600x900_2x.jpg"
        );
    }
}
