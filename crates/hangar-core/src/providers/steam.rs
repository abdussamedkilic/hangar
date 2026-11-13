use super::{LibraryProvider, ProviderError};
use crate::model::{Platform, PlatformEntry};
use crate::vdf::{parse, Vdf};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub struct SteamProvider {
    root: PathBuf,
}

impl SteamProvider {
    pub fn with_root(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    #[cfg(windows)]
    pub fn detect() -> Option<Self> {
        use winreg::{enums::HKEY_CURRENT_USER, RegKey};
        let key = RegKey::predef(HKEY_CURRENT_USER).open_subkey("Software\\Valve\\Steam").ok()?;
        let path: String = key.get_value("SteamPath").ok()?;
        Some(Self::with_root(PathBuf::from(path)))
    }

    #[cfg(not(windows))]
    pub fn detect() -> Option<Self> {
        None
    }
}

impl LibraryProvider for SteamProvider {
    fn platform(&self) -> Platform {
        Platform::Steam
    }

    fn is_available(&self) -> bool {
        self.root.exists()
    }

    fn scan(&self) -> Result<Vec<PlatformEntry>, ProviderError> {
        let mut out = Vec::new();
        for lib in library_paths(&self.root) {
            let steamapps = lib.join("steamapps");
            let Ok(dir) = std::fs::read_dir(&steamapps) else { continue };
            for f in dir.flatten() {
                let name = f.file_name().to_string_lossy().into_owned();
                if !(name.starts_with("appmanifest_") && name.ends_with(".acf")) {
                    continue;
                }
                let Ok(src) = std::fs::read_to_string(f.path()) else { continue };
                let Some(mut e) = parse_manifest(&src) else { continue };
                if let Some(dir) = installdir(&src) {
                    e.install_path = Some(steamapps.join("common").join(dir).to_string_lossy().into_owned());
                }
                out.push(e);
            }
        }
        Ok(out)
    }
}

fn installdir(src: &str) -> Option<String> {
    let root = parse(src).ok()?;
    Some(root.get("AppState")?.get("installdir")?.as_str()?.to_string())
}

/// Library folders listed in `steamapps/libraryfolders.vdf` that exist on disk.
pub fn library_paths(root: &Path) -> Vec<PathBuf> {
    let Ok(src) = std::fs::read_to_string(root.join("steamapps/libraryfolders.vdf")) else {
        return Vec::new();
    };
    let Ok(doc) = parse(&src) else { return Vec::new() };
    let Some(Vdf::Obj(items)) = doc.get("libraryfolders") else { return Vec::new() };
    items
        .iter()
        .filter_map(|(_, v)| v.get("path")?.as_str().map(PathBuf::from))
        .filter(|p| p.exists())
        .collect()
}

/// Steamworks redistributables, Proton and runtimes are not games.
pub fn is_tool(appid: &str, name: &str) -> bool {
    appid == "228980" || name.starts_with("Proton") || name.starts_with("Steam Linux Runtime")
}

/// Installed game from an appmanifest; `None` for tools/runtimes or malformed input.
/// `install_path` is left `None` (the library is unknown here); `scan()` fills it.
pub fn parse_manifest(src: &str) -> Option<PlatformEntry> {
    let doc = parse(src).ok()?;
    let app = doc.get("AppState")?;
    let field = |k: &str| app.get(k).and_then(Vdf::as_str);
    let appid = field("appid")?;
    let name = field("name")?;
    if is_tool(appid, name) {
        return None;
    }
    Some(PlatformEntry {
        platform: Platform::Steam,
        platform_id: appid.to_string(),
        title: name.to_string(),
        installed: true,
        install_path: None,
        size_bytes: field("SizeOnDisk").and_then(|s| s.parse().ok()),
        last_played: field("LastPlayed").and_then(|s| s.parse::<i64>().ok()).filter(|&t| t != 0),
        playtime_min: None,
        launch_uri: format!("steam://rungameid/{appid}"),
        install_uri: Some(format!("steam://install/{appid}")),
        launch_args: vec![],
        cover_url: None,
        edition: None,
        copy: None,
    })
}

/// Most recent account: `MostRecent "1"`, else the highest `timestamp`, else the first user.
/// SteamID64: 17 digits. Account names / vanity names are not accepted by the Web API.
pub fn is_steamid64(s: &str) -> bool {
    s.len() == 17 && s.bytes().all(|b| b.is_ascii_digit())
}

pub fn most_recent_steamid(loginusers: &str) -> Option<String> {
    let doc = parse(loginusers).ok()?;
    let Some(Vdf::Obj(users)) = doc.get("users") else { return None };
    let flag = |v: &Vdf| v.get("MostRecent").and_then(Vdf::as_str) == Some("1");
    let ts = |v: &Vdf| v.get("timestamp").and_then(Vdf::as_str).and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
    users
        .iter()
        .find(|(_, v)| flag(v))
        .or_else(|| users.iter().max_by_key(|(_, v)| ts(v)))
        .map(|(id, _)| id.clone())
}

pub fn parse_owned_games(json: &str) -> Result<Vec<PlatformEntry>, ProviderError> {
    let v: serde_json::Value = serde_json::from_str(json).map_err(|e| ProviderError::Parse(e.to_string()))?;
    let games = v["response"]["games"].as_array().cloned().unwrap_or_default();
    Ok(games
        .iter()
        .filter_map(|g| {
            let appid = g["appid"].as_u64()?;
            if is_tool(&appid.to_string(), g["name"].as_str()?) {
                return None;
            }
            Some(PlatformEntry {
                platform: Platform::Steam,
                platform_id: appid.to_string(),
                title: g["name"].as_str()?.to_string(),
                installed: false,
                install_path: None,
                size_bytes: None,
                last_played: g["rtime_last_played"].as_i64().filter(|&t| t != 0),
                playtime_min: g["playtime_forever"].as_u64().map(|m| m as u32),
                launch_uri: format!("steam://rungameid/{appid}"),
                install_uri: Some(format!("steam://install/{appid}")),
                launch_args: vec![],
                cover_url: None,
                edition: None,
                copy: None,
            })
        })
        .collect())
}

fn net_err(e: &ureq::Error) -> ProviderError {
    // Never Display the error: it embeds the request URL, which contains the API key.
    match e {
        ureq::Error::Status(code, _) => ProviderError::Network(format!("steam api returned status {code}")),
        ureq::Error::Transport(t) => match t.message() {
            Some(m) => ProviderError::Network(format!("{:?}: {m}", t.kind())),
            None => ProviderError::Network(format!("{:?}", t.kind())),
        },
    }
}

pub fn fetch_owned(api_key: &str, steamid: &str) -> Result<Vec<PlatformEntry>, ProviderError> {
    let body = ureq::get("https://api.steampowered.com/IPlayerService/GetOwnedGames/v1/")
        .query("key", api_key)
        .query("steamid", steamid)
        .query("include_appinfo", "1")
        .query("include_played_free_games", "1")
        .query("format", "json")
        .timeout(Duration::from_secs(10))
        .call()
        .map_err(|e| net_err(&e))?
        .into_string()
        .map_err(|e| ProviderError::Network(e.to_string()))?;
    parse_owned_games(&body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fx(name: &str) -> String {
        let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/steam").join(name);
        std::fs::read_to_string(p).unwrap()
    }

    fn temp_dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("hangar-steam-{}-{}", tag, std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("steamapps")).unwrap();
        d
    }

    fn libfolders(paths: &[&Path]) -> String {
        let mut s = String::from("\"libraryfolders\"\n{\n");
        for (i, p) in paths.iter().enumerate() {
            let esc = p.to_string_lossy().replace('\\', "\\\\");
            s.push_str(&format!("\"{}\"\n{{\n\"path\" \"{}\"\n}}\n", i, esc));
        }
        s.push('}');
        s
    }

    #[test]
    fn manifest_to_entry() {
        let e = parse_manifest(&fx("real-appmanifest_1868140.acf")).unwrap();
        assert_eq!(e.platform, Platform::Steam);
        assert_eq!(e.platform_id, "1868140");
        assert_eq!(e.title, "DAVE THE DIVER");
        assert!(e.installed);
        assert_eq!(e.size_bytes, Some(5919276623));
        assert_eq!(e.last_played, Some(1792087076));
        assert_eq!(e.install_path, None);
        assert_eq!(e.launch_uri, "steam://rungameid/1868140");
        assert_eq!(e.install_uri.as_deref(), Some("steam://install/1868140"));
    }

    #[test]
    fn skips_tools() {
        assert!(parse_manifest(&fx("real-appmanifest_228980.acf")).is_none());
        assert!(parse_manifest(&fx("synthetic-appmanifest_1493710.acf")).is_none());
        let slr = "\"AppState\"\n{\n\"appid\" \"1628350\"\n\"name\" \"Steam Linux Runtime 3.0 (sniper)\"\n}";
        assert!(parse_manifest(slr).is_none());
    }

    #[test]
    fn utf8_title_preserved() {
        let e = parse_manifest(&fx("synthetic-appmanifest_utf8.acf")).unwrap();
        assert_eq!(e.title, "Şeytan İşi");
    }

    #[test]
    fn missing_library_is_skipped() {
        let d = temp_dir("missing");
        let src = libfolders(&[&d, Path::new("Z:\\NoSuchLibrary")]);
        std::fs::write(d.join("steamapps/libraryfolders.vdf"), src).unwrap();
        assert_eq!(library_paths(&d), vec![d.clone()]);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn scan_reads_manifests() {
        let d = temp_dir("scan");
        std::fs::write(d.join("steamapps/libraryfolders.vdf"), libfolders(&[&d])).unwrap();
        std::fs::write(d.join("steamapps/appmanifest_1868140.acf"), fx("real-appmanifest_1868140.acf")).unwrap();
        let v = SteamProvider::with_root(d.clone()).scan().unwrap();
        assert_eq!(v.len(), 1);
        assert!(v[0].install_path.as_deref().unwrap().ends_with("Dave the Diver"));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn steamid_from_loginusers() {
        assert_eq!(most_recent_steamid(&fx("synthetic-loginusers.vdf")).as_deref(), Some("76561190000000001"));
    }

    #[test]
    fn steamid_without_mostrecent() {
        assert_eq!(most_recent_steamid(&fx("real-loginusers.vdf")).as_deref(), Some("76561190000000000"));
    }

    #[test]
    fn owned_games_json() {
        let v = parse_owned_games(&fx("synthetic-owned-games.json")).unwrap();
        assert_eq!(v.len(), 2);
        assert!(!v[0].installed);
        assert_eq!(v[0].playtime_min, Some(1234));
        assert_eq!(v[0].last_played, Some(1730000000));
        assert_eq!(v[1].last_played, None);
    }

    #[test]
    fn owned_games_skip_tools() {
        let j = r#"{"response":{"games":[{"appid":1,"name":"Hades"},{"appid":2,"name":"Proton 9.0"},{"appid":228980,"name":"Steamworks Common Redistributables"}]}}"#;
        let v = parse_owned_games(j).unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].platform_id, "1");
    }

    #[test]
    fn steamid64_rejects_account_names() {
        assert!(is_steamid64("76561198000000001"));
        assert!(!is_steamid64("player"));
        assert!(!is_steamid64("7656"));
    }

    #[test]
    fn network_error_hides_api_key() {
        let e = ureq::Error::Status(403, ureq::Response::new(403, "Forbidden", "").unwrap());
        let msg = net_err(&e).to_string();
        assert!(msg.contains("403"));
        assert!(!msg.contains("key="));
    }
}
