use super::{LibraryProvider, ProviderError};
use crate::model::{Platform, PlatformEntry};
use std::path::{Path, PathBuf};

pub struct GogRecord {
    pub game_id: String,
    pub name: String,
    pub path: String,
    pub exe: String,
}

/// Registry values are executed: only an absolute drive-letter local path
/// (no UNC, no `://`) is accepted.
fn is_local_abs(p: &str) -> bool {
    let b = p.as_bytes();
    b.len() > 2 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'/' || b[2] == b'\\') && !p.contains("://")
}

fn is_local_exe(p: &str) -> bool {
    is_local_abs(p) && p.to_ascii_lowercase().ends_with(".exe")
}

/// `galaxy` is the Galaxy install folder (registry `client` value).
pub fn entries_from(records: Vec<GogRecord>, galaxy: Option<&Path>) -> Vec<PlatformEntry> {
    let galaxy_exe = galaxy
        .and_then(|g| g.to_str())
        .filter(|g| is_local_abs(g))
        .map(|g| format!("{}\\GalaxyClient.exe", g.trim_end_matches(['/', '\\'])));
    records
        .into_iter()
        .filter(|r| !r.name.trim().is_empty() && !r.game_id.is_empty() && is_local_exe(&r.exe))
        .map(|r| {
            let (launch_uri, launch_args) = match &galaxy_exe {
                Some(g) => (
                    g.clone(),
                    vec!["/command=runGame".to_string(), format!("/gameId={}", r.game_id), format!("/path={}", r.path)],
                ),
                None => (r.exe.clone(), vec![]),
            };
            PlatformEntry {
                platform: Platform::Gog,
                installed: Path::new(&r.exe).exists(),
                platform_id: r.game_id,
                title: r.name,
                launch_uri,
                install_path: Some(r.path).filter(|p| !p.is_empty()),
                size_bytes: None,
                last_played: None,
                playtime_min: None,
                install_uri: None,
                launch_args,
                cover_url: None,
                edition: None,
                copy: None,
            }
        })
        .collect()
}

/// GOG Galaxy's local database (read-only; Galaxy may be running).
pub const GALAXY_DB: &str = r"C:\ProgramData\GOG.com\Galaxy\storage\galaxy-2.0.db";

/// releaseKey -> `verticalCover` https URL from the `originalImages` piece. Any failure yields an empty map.
fn vertical_covers(conn: &rusqlite::Connection) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    let Ok(mut stmt) = conn.prepare(
        "SELECT p.releaseKey, p.value FROM GamePieces p JOIN GamePieceTypes t ON t.id = p.gamePieceTypeId WHERE t.type = 'originalImages'",
    ) else {
        return map;
    };
    let Ok(rows) = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))) else {
        return map;
    };
    for (key, value) in rows.flatten() {
        let url = serde_json::from_str::<serde_json::Value>(&value)
            .ok()
            .and_then(|v| v.get("verticalCover").and_then(|u| u.as_str()).map(str::to_string))
            .filter(|u| u.starts_with("https://"));
        if let Some(u) = url {
            map.insert(key, u);
        }
    }
    map
}

/// Owned games (`gog_<id>` releases with a title) from an open Galaxy DB.
pub fn owned_from_galaxy_db(conn: &rusqlite::Connection) -> rusqlite::Result<Vec<PlatformEntry>> {
    let mut stmt = conn.prepare(
        "SELECT r.releaseKey, p.value FROM LibraryReleases r \
         JOIN GamePieces p ON p.releaseKey = r.releaseKey \
         JOIN GamePieceTypes t ON t.id = p.gamePieceTypeId \
         WHERE t.type = 'title' AND r.releaseKey LIKE 'gog\\_%' ESCAPE '\\'",
    )?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
    let covers = vertical_covers(conn);
    let mut out: Vec<PlatformEntry> = Vec::new();
    for row in rows {
        let (key, value) = row?;
        // The id ends up in a URI: digits only.
        let id = match key.strip_prefix("gog_") {
            Some(s) if !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) => s.to_string(),
            _ => continue,
        };
        let title = serde_json::from_str::<serde_json::Value>(&value)
            .ok()
            .and_then(|v| v.get("title").and_then(|t| t.as_str()).map(|t| t.trim().to_string()))
            .unwrap_or_default();
        if id.is_empty() || title.is_empty() || out.iter().any(|e| e.platform_id == id) {
            continue;
        }
        let uri = format!("goggalaxy://openGameView/{id}");
        let cover_url = covers.get(&key).cloned();
        out.push(PlatformEntry {
            platform: Platform::Gog,
            platform_id: id,
            title,
            installed: false,
            launch_uri: uri.clone(),
            install_path: None,
            size_bytes: None,
            last_played: None,
            playtime_min: None,
            install_uri: Some(uri),
            launch_args: vec![],
            cover_url,
            edition: None,
            copy: None,
        });
    }
    out.sort_by(|a, b| a.platform_id.cmp(&b.platform_id));
    Ok(out)
}

/// Missing file, open error or schema mismatch all yield no entries.
pub fn owned_from_path(path: &Path) -> Vec<PlatformEntry> {
    use rusqlite::{Connection, OpenFlags};
    if !path.is_file() {
        return vec![];
    }
    let uri = format!("file:{}?mode=ro", path.to_string_lossy().replace('\\', "/"));
    Connection::open_with_flags(uri, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI)
        .and_then(|c| owned_from_galaxy_db(&c))
        .unwrap_or_default()
}

/// Installed entries win: an owned entry with an installed id is dropped.
pub fn combine(mut installed: Vec<PlatformEntry>, owned: Vec<PlatformEntry>) -> Vec<PlatformEntry> {
    let covers: std::collections::HashMap<String, String> =
        owned.iter().filter_map(|e| Some((e.platform_id.clone(), e.cover_url.clone()?))).collect();
    for e in installed.iter_mut().filter(|e| e.cover_url.is_none()) {
        e.cover_url = covers.get(&e.platform_id).cloned();
    }
    let ids: std::collections::HashSet<String> = installed.iter().map(|e| e.platform_id.clone()).collect();
    installed.extend(owned.into_iter().filter(|e| !ids.contains(&e.platform_id)));
    installed
}

pub struct GogProvider {
    records: Vec<GogRecord>,
    galaxy: Option<PathBuf>,
    owned: Vec<PlatformEntry>,
}

impl GogProvider {
    #[cfg(windows)]
    pub fn detect() -> Option<Self> {
        use winreg::{enums::HKEY_LOCAL_MACHINE, RegKey};
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        let games = hklm.open_subkey("SOFTWARE\\WOW6432Node\\GOG.com\\Games").ok();
        let owned = owned_from_path(Path::new(GALAXY_DB));
        if games.is_none() && owned.is_empty() {
            return None;
        }
        let records = games
            .iter()
            .flat_map(|games| {
                games.enum_keys().flatten().filter_map(move |k| {
                    let g = games.open_subkey(&k).ok()?;
                    Some(GogRecord {
                        game_id: g.get_value("gameID").unwrap_or(k),
                        name: g.get_value("gameName").ok()?,
                        path: g.get_value("path").unwrap_or_default(),
                        exe: g.get_value("exe").ok()?,
                    })
                })
            })
            .collect();
        let galaxy = hklm
            .open_subkey("SOFTWARE\\WOW6432Node\\GOG.com\\GalaxyClient\\paths")
            .ok()
            .and_then(|p| p.get_value::<String, _>("client").ok())
            .map(PathBuf::from)
            .filter(|p| p.to_str().is_some_and(is_local_abs) && p.join("GalaxyClient.exe").is_file());
        Some(Self { records, galaxy, owned })
    }

    #[cfg(not(windows))]
    pub fn detect() -> Option<Self> {
        None
    }
}

impl LibraryProvider for GogProvider {
    fn platform(&self) -> Platform {
        Platform::Gog
    }

    fn is_available(&self) -> bool {
        true
    }

    fn scan(&self) -> Result<Vec<PlatformEntry>, ProviderError> {
        let records = self
            .records
            .iter()
            .map(|r| GogRecord { game_id: r.game_id.clone(), name: r.name.clone(), path: r.path.clone(), exe: r.exe.clone() })
            .collect();
        Ok(combine(entries_from(records, self.galaxy.as_deref()), self.owned.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn galaxy_db() -> rusqlite::Connection {
        let c = rusqlite::Connection::open_in_memory().unwrap();
        c.execute_batch(
            "CREATE TABLE LibraryReleases (releaseKey TEXT);
             CREATE TABLE GamePieceTypes (id INTEGER, type TEXT);
             CREATE TABLE GamePieces (releaseKey TEXT, gamePieceTypeId INTEGER, value TEXT);
             INSERT INTO GamePieceTypes VALUES (1, 'title'), (2, 'originalTitle'), (3, 'originalImages');
             INSERT INTO LibraryReleases VALUES ('gog_2'), ('gog_1'), ('steam_3'), ('gog_4');
             INSERT INTO GamePieces VALUES
               ('gog_1', 1, '{\"title\":\"Prey\"}'),
               ('gog_2', 1, '{\"title\":\"Hades - Amazon Prime\"}'),
               ('steam_3', 1, '{\"title\":\"Steam Game\"}'),
               ('gog_4', 2, '{\"title\":\"Wrong Type\"}'),
               ('gog_1', 3, '{\"background\":\"https://images.gog.com/b.webp\",\"verticalCover\":\"https://images.gog.com/abc_glx_vertical_cover.webp?namespace=gamesdb\"}'),
               ('gog_2', 3, '{\"verticalCover\":\"http://insecure/x.webp\"}');",
        )
        .unwrap();
        c
    }

    #[test]
    fn owned_reads_only_gog_with_title() {
        let got = owned_from_galaxy_db(&galaxy_db()).unwrap();
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].platform_id, "1");
        assert_eq!(got[1].platform_id, "2");
        let e = &got[0];
        assert_eq!(e.platform, Platform::Gog);
        assert_eq!(e.title, "Prey");
        assert!(!e.installed);
        assert_eq!(e.launch_uri, "goggalaxy://openGameView/1");
        assert_eq!(e.install_uri.as_deref(), Some("goggalaxy://openGameView/1"));
        assert!(e.launch_args.is_empty());
        assert!(e.install_path.is_none() && e.size_bytes.is_none() && e.last_played.is_none() && e.playtime_min.is_none());
        assert_eq!(got[1].title, "Hades - Amazon Prime");
        assert_eq!(
            got[0].cover_url.as_deref(),
            Some("https://images.gog.com/abc_glx_vertical_cover.webp?namespace=gamesdb")
        );
        assert_eq!(got[1].cover_url, None); // non-https is rejected
    }

    #[test]
    fn owned_drops_installed_ids() {
        let installed = entries_from(vec![rec("2", "Hades", r"D:\G\a.exe")], None);
        let owned = owned_from_galaxy_db(&galaxy_db()).unwrap();
        let all = combine(installed, owned);
        assert_eq!(all.len(), 2);
        let two: Vec<_> = all.iter().filter(|e| e.platform_id == "2").collect();
        assert_eq!(two.len(), 1);
        assert_eq!(two[0].title, "Hades");
        assert_eq!(all.iter().filter(|e| e.platform_id == "1").count(), 1);
        assert!(all.iter().find(|e| e.platform_id == "1").unwrap().cover_url.is_some());
    }

    #[test]
    fn installed_entry_gets_cover_from_owned() {
        let mut o = owned_from_galaxy_db(&galaxy_db()).unwrap();
        o.retain(|e| e.platform_id == "1");
        let installed = entries_from(vec![rec("1", "Prey", r"D:\G\a.exe")], None);
        let all = combine(installed, o);
        assert_eq!(all.len(), 1);
        assert!(!all[0].installed && all[0].launch_uri == r"D:\G\a.exe");
        assert!(all[0].cover_url.as_deref().is_some_and(|u| u.contains("vertical_cover")));
    }

    #[test]
    fn owned_rejects_non_numeric_ids() {
        let c = rusqlite::Connection::open_in_memory().unwrap();
        c.execute_batch(
            "CREATE TABLE LibraryReleases (releaseKey TEXT);
             CREATE TABLE GamePieceTypes (id INTEGER, type TEXT);
             CREATE TABLE GamePieces (releaseKey TEXT, gamePieceTypeId INTEGER, value TEXT);
             INSERT INTO GamePieceTypes VALUES (1, 'title');
             INSERT INTO LibraryReleases VALUES ('gog_1/x'), ('gog_a b'), ('gog_gog_5'), ('gog_7'), ('gog_');
             INSERT INTO GamePieces VALUES
               ('gog_1/x', 1, '{\"title\":\"A\"}'),
               ('gog_a b', 1, '{\"title\":\"B\"}'),
               ('gog_gog_5', 1, '{\"title\":\"C\"}'),
               ('gog_7', 1, '{\"title\":\"Ok\"}'),
               ('gog_', 1, '{\"title\":\"E\"}');",
        )
        .unwrap();
        let got = owned_from_galaxy_db(&c).unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].platform_id, "7");
    }

    #[test]
    fn non_galaxy_sqlite_file_yields_no_owned() {
        let p = std::env::temp_dir().join(format!("hangar-notgalaxy-{}.db", std::process::id()));
        {
            let c = rusqlite::Connection::open(&p).unwrap();
            c.execute_batch("CREATE TABLE Other (x INTEGER);").unwrap();
        }
        let got = owned_from_path(&p);
        let _ = std::fs::remove_file(&p);
        assert!(got.is_empty());
    }

    #[test]
    fn missing_db_yields_no_owned() {
        assert!(owned_from_path(Path::new("/definitely/not/here/galaxy-2.0.db")).is_empty());
    }

    fn rec(id: &str, name: &str, exe: &str) -> GogRecord {
        GogRecord { game_id: id.into(), name: name.into(), path: r"D:\GOG Games\Game".into(), exe: exe.into() }
    }

    #[test]
    fn with_galaxy_uses_run_game_args() {
        let got = entries_from(
            vec![rec("123", "Witcher", r"D:\GOG Games\Game\w.exe")],
            Some(Path::new(r"C:\Program Files\GOG Galaxy")),
        );
        assert_eq!(got.len(), 1);
        let e = &got[0];
        assert_eq!(e.platform, Platform::Gog);
        assert_eq!(e.platform_id, "123");
        assert_eq!(e.title, "Witcher");
        assert_eq!(e.launch_uri, r"C:\Program Files\GOG Galaxy\GalaxyClient.exe");
        assert_eq!(e.launch_args, vec!["/command=runGame", "/gameId=123", r"/path=D:\GOG Games\Game"]);
    }

    #[test]
    fn without_galaxy_launches_exe() {
        let got = entries_from(vec![rec("1", "A", r"D:\G\a.exe")], None);
        assert_eq!(got[0].launch_uri, r"D:\G\a.exe");
        assert!(got[0].launch_args.is_empty());
        assert!(!got[0].installed);
    }

    #[test]
    fn skips_empty_name() {
        assert!(entries_from(vec![rec("1", "", r"D:\G\a.exe")], None).is_empty());
    }

    #[test]
    fn skips_unsafe_exe() {
        for bad in [
            r"\\server\share\a.exe",
            "//server/share/a.exe",
            "https://evil/a.exe",
            "a.exe",
            r"D:\G\readme.txt",
            r"D:\G\a.exe.lnk",
            "",
        ] {
            assert!(entries_from(vec![rec("1", "A", bad)], None).is_empty(), "{bad}");
        }
        assert_eq!(entries_from(vec![rec("1", "A", r"d:\g\A.EXE")], None).len(), 1);
    }

    #[test]
    fn unsafe_galaxy_is_treated_as_absent() {
        for bad in [r"\\server\share", "//server/share", "https://x", "GOG Galaxy", ""] {
            let got = entries_from(vec![rec("1", "A", r"D:\G\a.exe")], Some(Path::new(bad)));
            assert_eq!(got[0].launch_uri, r"D:\G\a.exe", "{bad}");
            assert!(got[0].launch_args.is_empty());
        }
    }
}
