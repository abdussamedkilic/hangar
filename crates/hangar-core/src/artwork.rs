use crate::db::Db;
use crate::merge::{edition_of, normalize};
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
pub fn parse_store_search(json: &str, norm_title: &str, edition_label: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    v.get("items")?.as_array()?.iter().find_map(|it| {
        let name = it.get("name")?.as_str()?;
        (normalize(name) == norm_title && edition_of(name).label == edition_label).then(|| it.get("id")?.as_u64().map(|i| i.to_string()))?
    })
}

/// Cover/API downloads are limited to known image hosts: https, no userinfo, no explicit port.
pub fn is_allowed_cover_url(url: &str) -> bool {
    let Ok(u) = url::Url::parse(url) else { return false };
    if u.scheme() != "https" || !u.username().is_empty() || u.password().is_some() || u.port().is_some() {
        return false;
    }
    let host = u.host_str().unwrap_or("");
    let sub = |d: &str| host.ends_with(&format!(".{d}"));
    sub("epicgames.com")
        || sub("unrealengine.com")
        || sub("steamstatic.com")
        || ["images.gog.com", "images.gog-statics.com", "api.gog.com", "steamcdn-a.akamaihd.net"].contains(&host)
}

/// Like `get_bytes` but only for allowed hosts, and redirects are not followed.
fn get_bytes_allowed(url: &str) -> Option<Vec<u8>> {
    if !is_allowed_cover_url(url) {
        return None;
    }
    fetch(url, &[], 0)
}

fn get_bytes(url: &str, query: &[(&str, &str)]) -> Option<Vec<u8>> {
    fetch(url, query, 5)
}

fn fetch(url: &str, query: &[(&str, &str)], redirects: u32) -> Option<Vec<u8>> {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(10))
        .redirects(redirects)
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

fn is_webp(bytes: &[u8]) -> bool {
    bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP"
}

pub fn is_image(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0xFF, 0xD8, 0xFF]) || bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]) || is_webp(bytes)
}

/// File extension from magic bytes (call only on `is_image` data).
fn image_ext(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        "png"
    } else if is_webp(bytes) {
        "webp"
    } else {
        "jpg"
    }
}

/// Where a cover may come from, in the order `ensure_cover` tries them.
#[derive(Debug, PartialEq)]
pub enum Candidate {
    Steam(String),
    Url(String),
    /// GOG public API box art for a numeric GOG id.
    GogApi(String),
    SteamSearch,
}

pub fn cover_candidates(game: &Game) -> Vec<Candidate> {
    let mut out: Vec<Candidate> = game
        .entries
        .iter()
        .find(|e| e.platform == Platform::Steam)
        .map(|e| Candidate::Steam(e.platform_id.clone()))
        .into_iter()
        .collect();
    out.extend(Platform::ALL
        .iter()
        .filter_map(|p| game.entries.iter().find(|e| e.platform == *p && e.cover_url.is_some()))
        .filter_map(|e| e.cover_url.clone())
        .map(Candidate::Url));
    if let Some(e) = game
        .entries
        .iter()
        .find(|e| e.platform == Platform::Gog && !e.platform_id.is_empty() && e.platform_id.bytes().all(|b| b.is_ascii_digit()))
    {
        out.push(Candidate::GogApi(e.platform_id.clone()));
    }
    out.push(Candidate::SteamSearch);
    out
}

/// `_links.boxArtImage.href` of GOG's `/v2/games/<id>` response; https only.
pub fn gog_box_art(json: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let href = v.get("_links")?.get("boxArtImage")?.get("href")?.as_str()?;
    href.starts_with("https://").then(|| href.to_string())
}

fn search_appid(game: &Game) -> Option<String> {
    let body = get_bytes(
        "https://store.steampowered.com/api/storesearch/",
        &[("term", &game.title), ("l", "english"), ("cc", "US")],
    )?;
    parse_store_search(
        std::str::from_utf8(&body).ok()?,
        &normalize(&game.title),
        &edition_of(&game.title).label,
    )
}

/// Largest frame of an ICO that is an embedded PNG. Malformed input yields None.
pub fn largest_png_in_ico(bytes: &[u8]) -> Option<Vec<u8>> {
    const SIG: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    if bytes.get(0..4)? != [0, 0, 1, 0] {
        return None;
    }
    let count = u16::from_le_bytes(bytes.get(4..6)?.try_into().ok()?) as usize;
    let mut best: Option<(u32, &[u8])> = None;
    for i in 0..count {
        let e = bytes.get(6 + i * 16..6 + i * 16 + 16)?;
        let dim = |b: u8| if b == 0 { 256u32 } else { b as u32 };
        let area = dim(e[0]) * dim(e[1]);
        let size = u32::from_le_bytes(e[8..12].try_into().ok()?) as usize;
        let off = u32::from_le_bytes(e[12..16].try_into().ok()?) as usize;
        let Some(data) = bytes.get(off..off.checked_add(size)?) else {
            continue;
        };
        if data.starts_with(&SIG) && best.is_none_or(|(a, _)| area > a) {
            best = Some((area, data));
        }
    }
    best.map(|(_, d)| d.to_vec())
}

fn riot_icon_png(riot_root: &Path, game: &Game) -> Option<Vec<u8>> {
    let e = game.entries.iter().find(|e| e.platform == Platform::Riot)?;
    let id = &e.platform_id;
    let ico = std::fs::read(riot_root.join("Metadata").join(id).join(format!("{id}.ico"))).ok()?;
    largest_png_in_ico(&ico)
}

fn write_cover(db: &Db, cache_dir: &Path, game: &Game, ext: &str, bytes: Vec<u8>) -> Option<String> {
    // The caller's snapshot may be stale: a cover picked meanwhile wins.
    if let Some(cur) = db.games().ok()?.into_iter().find(|g| g.id == game.id).and_then(|g| g.cover_path) {
        if Path::new(&cur).is_file() {
            return Some(cur);
        }
    }
    std::fs::create_dir_all(cache_dir).ok()?;
    let dest = cache_dir.join(format!("{}.{ext}", game.id));
    let tmp = cache_dir.join(format!("{}.{ext}.tmp", game.id));
    if std::fs::write(&tmp, bytes).is_err() || std::fs::rename(&tmp, &dest).is_err() {
        let _ = std::fs::remove_file(&tmp);
        return None;
    }
    for other in ["jpg", "png", "webp"].into_iter().filter(|e| *e != ext) {
        let _ = std::fs::remove_file(cache_dir.join(format!("{}.{other}", game.id)));
    }
    let path = dest.to_string_lossy().into_owned();
    db.set_cover(game.id, &path).ok()?;
    Some(path)
}

/// Copies a user-picked PNG/JPG into the covers cache as `<id>-custom.<ext>` and makes it the game's cover.
pub fn set_custom_cover(db: &Db, cache_dir: &Path, game_id: i64, src: &Path) -> Result<String, String> {
    const MAX_CUSTOM: u64 = 20 * 1024 * 1024;
    let too_big = || "Görsel çok büyük".to_string();
    let meta = std::fs::metadata(src).map_err(|e| format!("Görsel okunamadı: {e}"))?;
    if meta.len() > MAX_CUSTOM {
        return Err(too_big());
    }
    let mut bytes = Vec::new();
    std::fs::File::open(src)
        .and_then(|f| f.take(MAX_CUSTOM + 1).read_to_end(&mut bytes))
        .map_err(|e| format!("Görsel okunamadı: {e}"))?;
    if bytes.len() as u64 > MAX_CUSTOM {
        return Err(too_big());
    }
    if !is_image(&bytes) {
        return Err("Desteklenmeyen görsel (PNG, JPG veya WebP seç)".into());
    }
    let ext = image_ext(&bytes);
    std::fs::create_dir_all(cache_dir).map_err(|e| format!("Kapak klasörü oluşturulamadı: {e}"))?;
    // Unique name per pick so the webview URL changes and never serves a cached older image.
    let mut ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let mut dest = cache_dir.join(format!("{game_id}-custom-{ms}.{ext}"));
    while dest.exists() {
        ms += 1;
        dest = cache_dir.join(format!("{game_id}-custom-{ms}.{ext}"));
    }
    let tmp = cache_dir.join(format!("{game_id}-custom-{ms}.{ext}.tmp"));
    if let Err(e) = std::fs::write(&tmp, &bytes).and_then(|_| std::fs::rename(&tmp, &dest)) {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("Kapak kaydedilemedi: {e}"));
    }
    // Only now drop stale files (never the one just written).
    let (plain_jpg, plain_png, plain_webp) =
        (format!("{game_id}.jpg"), format!("{game_id}.png"), format!("{game_id}.webp"));
    let custom = format!("{game_id}-custom-");
    if let Ok(rd) = std::fs::read_dir(cache_dir) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if e.path() != dest && (name == plain_jpg || name == plain_png || name == plain_webp || name.starts_with(&custom)) {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    let path = dest.to_string_lossy().into_owned();
    db.set_cover(game_id, &path).map_err(|e| format!("Veritabanı hatası: {e}"))?;
    Ok(path)
}

/// Returns the cover path for `game`, fetching and caching it if needed. Any failure yields None.
pub fn ensure_cover(
    db: &Db,
    cache_dir: &Path,
    game: &Game,
    steam_root: Option<&Path>,
    riot_root: Option<&Path>,
) -> Option<String> {
    if let Some(p) = &game.cover_path {
        if Path::new(p).is_file() {
            return Some(p.clone());
        }
    }
    if let Some(png) = riot_root.and_then(|r| riot_icon_png(r, game)) {
        if let Some(path) = write_cover(db, cache_dir, game, "png", png) {
            return Some(path);
        }
    }
    for c in cover_candidates(game) {
        let fetched: Option<(&'static str, Vec<u8>)> = match c {
            Candidate::Url(u) => get_bytes_allowed(&u)
                .filter(|b| is_image(b))
                .map(|b| (image_ext(&b), b)),
            Candidate::GogApi(id) => get_bytes_allowed(&format!("https://api.gog.com/v2/games/{id}"))
                .and_then(|b| gog_box_art(std::str::from_utf8(&b).ok()?))
                .and_then(|u| get_bytes_allowed(&u))
                .filter(|b| is_image(b))
                .map(|b| (image_ext(&b), b)),
            Candidate::Steam(id) => steam_bytes(steam_root, &id).map(|b| (image_ext(&b), b)),
            Candidate::SteamSearch => search_appid(game)
                .and_then(|id| steam_bytes(steam_root, &id))
                .map(|b| (image_ext(&b), b)),
        };
        if let Some((ext, b)) = fetched {
            if let Some(path) = write_cover(db, cache_dir, game, ext, b) {
                return Some(path);
            }
        }
    }
    None
}

fn steam_bytes(steam_root: Option<&Path>, appid: &str) -> Option<Vec<u8>> {
    let local = steam_root
        .and_then(|r| local_steam_cover(r, &appid))
        .and_then(|p| std::fs::read(p).ok())
        .filter(|b| is_image(b));
    let bytes = local
        .or_else(|| get_bytes(&steam_cdn_url(appid), &[]).filter(|b| is_image(b)))
        .or_else(|| {
            get_bytes(
                &format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{appid}/header.jpg"),
                &[],
            )
            .filter(|b| is_image(b))
        });
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_search_exact_match() {
        let json = include_str!("../tests/fixtures/artwork/synthetic-storesearch-alan-wake.json");
        assert_eq!(
            parse_store_search(json, "alan wake 2", "Orijinal").as_deref(),
            Some("1000108")
        );
        assert_eq!(
            parse_store_search(json, "alan wake", "Orijinal").as_deref(),
            Some("108710")
        );
        assert_eq!(parse_store_search(json, "control", "Orijinal"), None);
        assert_eq!(parse_store_search("not json", "x", "Orijinal"), None);
        // A Remastered card must not take the Original's cover (and vice versa).
        assert_eq!(parse_store_search(json, "alan wake", "Remastered"), None);
        let rm = r#"{"items":[{"id":1,"name":"Foo"},{"id":2,"name":"Foo Remastered"}]}"#;
        assert_eq!(parse_store_search(rm, "foo", "Remastered").as_deref(), Some("2"));
        assert_eq!(parse_store_search(rm, "foo", "Orijinal").as_deref(), Some("1"));
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
        assert!(is_image(b"RIFF\x10\0\0\0WEBPVP8 "));
        assert!(!is_image(b"RIFF\x10\0\0\0WAVEfmt "));
        assert!(!is_image(b"RIFF"));
        assert!(!is_image(b"<html>404</html>"));
        assert!(!is_image(b""));
    }

    const PNG_SIG: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

    /// entries: (width byte, height byte, data)
    fn build_ico(entries: &[(u8, u8, Vec<u8>)]) -> Vec<u8> {
        let mut out = vec![0, 0, 1, 0];
        out.extend((entries.len() as u16).to_le_bytes());
        let mut offset = 6 + 16 * entries.len() as u32;
        for (w, h, data) in entries {
            out.extend([*w, *h, 0, 0, 1, 0, 32, 0]);
            out.extend((data.len() as u32).to_le_bytes());
            out.extend(offset.to_le_bytes());
            offset += data.len() as u32;
        }
        for (_, _, data) in entries {
            out.extend(data);
        }
        out
    }

    fn fake_png(tag: u8) -> Vec<u8> {
        let mut v = PNG_SIG.to_vec();
        v.extend([tag; 4]);
        v
    }

    #[test]
    fn ico_picks_largest_png() {
        let ico = build_ico(&[(16, 16, vec![0x28, 0, 0, 0, 1, 2]), (0, 0, fake_png(7))]);
        assert_eq!(largest_png_in_ico(&ico), Some(fake_png(7)));
    }

    #[test]
    fn ico_bad_input_is_none() {
        assert_eq!(largest_png_in_ico(&[]), None);
        assert_eq!(largest_png_in_ico(b"garbage garbage garbage"), None);
        let ico = build_ico(&[(0, 0, fake_png(1))]);
        for n in 0..ico.len() {
            assert_eq!(largest_png_in_ico(&ico[..n]), None, "len {n}");
        }
        let only_bmp = build_ico(&[(16, 16, vec![0x28, 0, 0, 0]), (32, 32, vec![0x28, 0, 0, 0])]);
        assert_eq!(largest_png_in_ico(&only_bmp), None);
    }

    #[test]
    fn riot_cover_from_local_icon() {
        use crate::model::PlatformEntry;
        let root = std::env::temp_dir().join(format!("hangar-riot-{}", std::process::id()));
        let meta = root.join("Metadata").join("league_of_legends.live");
        std::fs::create_dir_all(&meta).unwrap();
        std::fs::write(
            meta.join("league_of_legends.live.ico"),
            build_ico(&[(16, 16, vec![0x28, 0]), (0, 0, fake_png(9))]),
        )
        .unwrap();
        let entry = PlatformEntry {
            platform: Platform::Riot,
            platform_id: "league_of_legends.live".into(),
            title: "League of Legends".into(),
            installed: true,
            install_path: None,
            size_bytes: None,
            last_played: None,
            playtime_min: None,
            launch_uri: "x".into(),
            install_uri: None,
            launch_args: vec![],
            cover_url: None,
            edition: None,
            copy: None,
        };
        let db = Db::open_in_memory().unwrap();
        db.replace_entries(vec![("league of legends".into(), vec![entry])]).unwrap();
        let game = db.games().unwrap().remove(0);
        let cache = root.join("covers");
        let path = ensure_cover(&db, &cache, &game, None, Some(&root)).unwrap();
        assert_eq!(Path::new(&path), cache.join(format!("{}.png", game.id)));
        assert_eq!(std::fs::read(&path).unwrap(), fake_png(9));
        assert_eq!(db.games().unwrap()[0].cover_path.as_deref(), Some(path.as_str()));
        std::fs::remove_dir_all(&root).unwrap();
    }

    fn one_game_db() -> (Db, Game) {
        use crate::model::PlatformEntry;
        let entry = PlatformEntry {
            platform: Platform::Local,
            platform_id: "1".into(),
            title: "Hades".into(),
            installed: true,
            install_path: None,
            size_bytes: None,
            last_played: None,
            playtime_min: None,
            launch_uri: "C:\\H.exe".into(),
            install_uri: None,
            launch_args: vec![],
            cover_url: None,
            edition: None,
            copy: None,
        };
        let db = Db::open_in_memory().unwrap();
        db.replace_entries(vec![("hades".into(), vec![entry])]).unwrap();
        let game = db.games().unwrap().remove(0);
        (db, game)
    }

    #[test]
    fn custom_cover_png_is_copied_and_set() {
        let root = std::env::temp_dir().join(format!("hangar-custom-ok-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let src = root.join("pick.png");
        std::fs::write(&src, fake_png(3)).unwrap();
        let (db, game) = one_game_db();
        let cache = root.join("covers");
        let path = set_custom_cover(&db, &cache, game.id, &src).unwrap();
        assert_eq!(Path::new(&path).parent(), Some(cache.as_path()));
        let name = Path::new(&path).file_name().unwrap().to_string_lossy().into_owned();
        assert!(name.starts_with(&format!("{}-custom-", game.id)) && name.ends_with(".png"), "{name}");
        assert_eq!(std::fs::read(&path).unwrap(), fake_png(3));
        assert_eq!(db.games().unwrap()[0].cover_path.as_deref(), Some(path.as_str()));
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn custom_cover_rejects_non_image() {
        let root = std::env::temp_dir().join(format!("hangar-custom-bad-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let src = root.join("note.txt");
        std::fs::write(&src, b"hello").unwrap();
        let (db, game) = one_game_db();
        let err = set_custom_cover(&db, &root.join("covers"), game.id, &src).unwrap_err();
        assert_eq!(err, "Desteklenmeyen görsel (PNG, JPG veya WebP seç)");
        assert_eq!(db.games().unwrap()[0].cover_path, None);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn custom_cover_removes_stale_files() {
        let root = std::env::temp_dir().join(format!("hangar-custom-stale-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let cache = root.join("covers");
        std::fs::create_dir_all(&cache).unwrap();
        let (db, game) = one_game_db();
        let old = cache.join(format!("{}.jpg", game.id));
        std::fs::write(&old, [0xFF, 0xD8, 0xFF, 0]).unwrap();
        let src = root.join("pick.png");
        std::fs::write(&src, fake_png(4)).unwrap();
        let p = set_custom_cover(&db, &cache, game.id, &src).unwrap();
        assert!(!old.exists());
        assert!(Path::new(&p).is_file());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn custom_cover_repeat_pick_changes_path_and_keeps_only_newest() {
        let root = std::env::temp_dir().join(format!("hangar-custom-rep-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let cache = root.join("covers");
        std::fs::create_dir_all(&root).unwrap();
        let (db, game) = one_game_db();
        let src = root.join("pick.png");
        std::fs::write(&src, fake_png(1)).unwrap();
        let p1 = set_custom_cover(&db, &cache, game.id, &src).unwrap();
        std::fs::write(&src, fake_png(2)).unwrap();
        let p2 = set_custom_cover(&db, &cache, game.id, &src).unwrap();
        assert_ne!(p1, p2);
        assert!(!Path::new(&p1).exists());
        assert_eq!(std::fs::read(&p2).unwrap(), fake_png(2));
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn custom_cover_cleanup_spares_other_games_with_shared_prefix() {
        let root = std::env::temp_dir().join(format!("hangar-custom-pre-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let cache = root.join("covers");
        std::fs::create_dir_all(&cache).unwrap();
        let (db, game) = one_game_db();
        let other = cache.join(format!("{}1-custom-5.png", game.id));
        std::fs::write(&other, fake_png(5)).unwrap();
        let src = root.join("pick.png");
        std::fs::write(&src, fake_png(6)).unwrap();
        set_custom_cover(&db, &cache, game.id, &src).unwrap();
        assert!(other.is_file());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn auto_fetch_write_skipped_when_cover_already_set() {
        let root = std::env::temp_dir().join(format!("hangar-custom-race-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let cache = root.join("covers");
        std::fs::create_dir_all(&root).unwrap();
        let (db, stale) = one_game_db(); // snapshot with cover_path None
        let src = root.join("pick.png");
        std::fs::write(&src, fake_png(8)).unwrap();
        let custom = set_custom_cover(&db, &cache, stale.id, &src).unwrap();
        let got = write_cover(&db, &cache, &stale, "jpg", vec![0xFF, 0xD8, 0xFF, 0]);
        assert_eq!(got.as_deref(), Some(custom.as_str()));
        assert!(!cache.join(format!("{}.jpg", stale.id)).exists());
        assert_eq!(db.games().unwrap()[0].cover_path.as_deref(), Some(custom.as_str()));
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn candidates_keep_urls_after_steam() {
        let (_, mut g) = one_game_db();
        let mut steam = g.entries[0].clone();
        steam.platform = Platform::Steam;
        steam.platform_id = "620".into();
        let mut epic = g.entries[0].clone();
        epic.platform = Platform::Epic;
        epic.cover_url = Some("https://e".into());
        g.entries = vec![epic, steam];
        assert_eq!(
            cover_candidates(&g),
            vec![Candidate::Steam("620".into()), Candidate::Url("https://e".into()), Candidate::SteamSearch]
        );
    }

    #[test]
    fn cover_url_allowlist() {
        for ok in [
            "https://images.gog.com/a_glx_vertical_cover.webp?namespace=gamesdb",
            "https://images.gog-statics.com/a.jpg",
            "https://api.gog.com/v2/games/1",
            "https://cdn1.epicgames.com/a.jpg",
            "https://cdn2.unrealengine.com/x.jpg",
            "https://cdn.cloudflare.steamstatic.com/x.jpg",
            "https://steamcdn-a.akamaihd.net/x.jpg",
        ] {
            assert!(is_allowed_cover_url(ok), "{ok}");
        }
        for bad in [
            "http://images.gog.com/a.jpg",
            "https://evil.com/a.jpg",
            "https://images.gog.com.evil.io/a.jpg",
            "https://user@images.gog.com/a.jpg",
            "https://images.gog.com:8443/a.jpg",
            "https://epicgames.com/a.jpg",
            "https://evil-epicgames.com/a.jpg",
            "https://evil-unrealengine.com/x.jpg",
            "https://unrealengine.com/x.jpg",
            "https://127.0.0.1/a.jpg",
            "not a url",
        ] {
            assert!(!is_allowed_cover_url(bad), "{bad}");
        }
    }

    #[test]
    fn write_cover_removes_other_extension_siblings() {
        let root = std::env::temp_dir().join(format!("hangar-sib-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let (db, game) = one_game_db();
        let old = root.join(format!("{}.jpg", game.id));
        std::fs::write(&old, [0xFF, 0xD8, 0xFF]).unwrap();
        let p = write_cover(&db, &root, &game, "webp", b"RIFF....WEBP".to_vec()).unwrap();
        assert!(Path::new(&p).is_file() && !old.exists());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn gog_box_art_parse() {
        let j = r#"{"_links":{"boxArtImage":{"href":"https://images.gog-statics.com/abc.jpg"}}}"#;
        assert_eq!(gog_box_art(j).as_deref(), Some("https://images.gog-statics.com/abc.jpg"));
        assert_eq!(gog_box_art(r#"{"_links":{"boxArtImage":{"href":"http://x/a.jpg"}}}"#), None);
        assert_eq!(gog_box_art(r#"{"_links":{}}"#), None);
        assert_eq!(gog_box_art("nope"), None);
    }

    #[test]
    fn image_ext_from_magic() {
        assert_eq!(image_ext(&fake_png(1)), "png");
        assert_eq!(image_ext(b"RIFF\x10\0\0\0WEBPVP8 "), "webp");
        assert_eq!(image_ext(&[0xFF, 0xD8, 0xFF, 0xE0]), "jpg");
    }

    #[test]
    fn candidates_order() {
        let (_, mut g) = one_game_db();
        let tpl = g.entries[0].clone();
        let mk = |p: Platform, id: &str, url: Option<&str>| {
            let mut e = tpl.clone();
            e.platform = p;
            e.platform_id = id.into();
            e.cover_url = url.map(str::to_string);
            e
        };
        // No Steam entry: urls in Platform::ALL order, then the store search.
        g.entries = vec![mk(Platform::Gog, "1", Some("https://g")), mk(Platform::Epic, "2", Some("https://e")), mk(Platform::Ubisoft, "3", None)];
        assert_eq!(
            cover_candidates(&g),
            vec![Candidate::Url("https://e".into()), Candidate::Url("https://g".into()), Candidate::GogApi("1".into()), Candidate::SteamSearch]
        );
        // A Steam entry goes first; the rest remain as fallbacks.
        g.entries.push(mk(Platform::Steam, "620", None));
        assert_eq!(
            cover_candidates(&g),
            vec![
                Candidate::Steam("620".into()),
                Candidate::Url("https://e".into()),
                Candidate::Url("https://g".into()),
                Candidate::GogApi("1".into()),
                Candidate::SteamSearch
            ]
        );
        // GOG entry without cover_url and a non-numeric id: no API candidate.
        g.entries = vec![mk(Platform::Gog, "abc", None)];
        assert_eq!(cover_candidates(&g), vec![Candidate::SteamSearch]);
        g.entries = vec![mk(Platform::Gog, "77", None)];
        assert_eq!(cover_candidates(&g), vec![Candidate::GogApi("77".into()), Candidate::SteamSearch]);
        // Nothing with a url: search only.
        g.entries = vec![mk(Platform::Local, "9", None)];
        assert_eq!(cover_candidates(&g), vec![Candidate::SteamSearch]);
    }

    #[test]
    fn cdn_url_format() {
        assert_eq!(
            steam_cdn_url("620"),
            "https://cdn.cloudflare.steamstatic.com/steam/apps/620/library_600x900_2x.jpg"
        );
    }
}
