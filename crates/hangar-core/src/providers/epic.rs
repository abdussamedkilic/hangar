use super::{LibraryProvider, ProviderError};
use crate::model::{Platform, PlatformEntry};
use std::path::PathBuf;

pub struct EpicProvider {
    manifests_dir: PathBuf,
}

impl EpicProvider {
    pub fn with_dir(manifests_dir: PathBuf) -> Self {
        Self { manifests_dir }
    }

    pub fn detect() -> Option<Self> {
        let p = PathBuf::from(r"C:\ProgramData\Epic\EpicGamesLauncher\Data\Manifests");
        p.exists().then(|| Self::with_dir(p))
    }
}

pub fn parse_item(json: &str) -> Option<PlatformEntry> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let s = |k: &str| v.get(k).and_then(|x| x.as_str()).filter(|x| !x.is_empty());
    let b = |k: &str| v.get(k).and_then(|x| x.as_bool());
    if b("bIsIncompleteInstall") != Some(false) || b("bIsApplication") != Some(true) {
        return None;
    }
    let is_addon = v
        .get("AppCategories")
        .and_then(|c| c.as_array())
        .is_some_and(|c| c.iter().any(|x| x.as_str() == Some("addons")));
    if is_addon {
        return None;
    }
    let app = s("AppName")?;
    Some(PlatformEntry {
        platform: Platform::Epic,
        platform_id: app.to_string(),
        title: s("DisplayName")?.to_string(),
        installed: true,
        install_path: s("InstallLocation").map(str::to_string),
        size_bytes: v.get("InstallSize").and_then(|x| x.as_u64()),
        last_played: None,
        playtime_min: None,
        launch_uri: format!(
            "com.epicgames.launcher://apps/{}%3A{}%3A{}?action=launch&silent=true",
            s("CatalogNamespace")?,
            s("CatalogItemId")?,
            app
        ),
        install_uri: Some("com.epicgames.launcher://store".to_string()),
    })
}

impl LibraryProvider for EpicProvider {
    fn platform(&self) -> Platform {
        Platform::Epic
    }

    fn is_available(&self) -> bool {
        self.manifests_dir.exists()
    }

    fn scan(&self) -> Result<Vec<PlatformEntry>, ProviderError> {
        let Ok(dir) = std::fs::read_dir(&self.manifests_dir) else { return Ok(vec![]) };
        let mut out = Vec::new();
        for f in dir.flatten() {
            if f.path().extension().and_then(|e| e.to_str()) != Some("item") {
                continue;
            }
            let Ok(src) = std::fs::read_to_string(f.path()) else { continue };
            if let Some(e) = parse_item(&src) {
                out.push(e);
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fx(name: &str) -> String {
        let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/epic").join(name);
        std::fs::read_to_string(p).unwrap()
    }

    #[test]
    fn item_to_entry() {
        let e = parse_item(&fx("real-deathsdoor.item")).unwrap();
        assert_eq!(e.platform, Platform::Epic);
        assert_eq!(e.platform_id, "65d73e3be8824829b5b788bd849b6559");
        assert_eq!(e.title, "Death's Door");
        assert!(e.installed);
        assert_eq!(e.install_path.as_deref(), Some(r"C:\Program Files\Epic Games\DeathsDoor"));
        assert_eq!(e.size_bytes, Some(3927634783));
        assert_eq!(e.last_played, None);
        assert_eq!(e.playtime_min, None);
        assert_eq!(
            e.launch_uri,
            "com.epicgames.launcher://apps/f588333a02fe4011980472c7be02db12%3A267fb10f28a141ec94a715f1cb24954d%3A65d73e3be8824829b5b788bd849b6559?action=launch&silent=true"
        );
        assert_eq!(e.install_uri.as_deref(), Some("com.epicgames.launcher://store"));
    }

    #[test]
    fn skips_dlc_and_incomplete() {
        assert!(parse_item(&fx("synthetic-dlc.item")).is_none());
        assert!(parse_item(&fx("synthetic-incomplete.item")).is_none());
        assert!(parse_item("not json").is_none());
    }

    #[test]
    fn missing_dir_is_empty() {
        let p = EpicProvider::with_dir(PathBuf::from("/definitely/not/here"));
        assert_eq!(p.scan(), Ok(vec![]));
    }

    #[test]
    fn scan_reads_items() {
        let d = std::env::temp_dir().join(format!("hangar-epic-scan-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("A.item"), fx("real-deathsdoor.item")).unwrap();
        std::fs::write(d.join("B.item"), fx("synthetic-dlc.item")).unwrap();
        std::fs::write(d.join("C.item"), "garbage").unwrap();
        std::fs::write(d.join("ignored.txt"), fx("real-deathsdoor.item")).unwrap();
        let got = EpicProvider::with_dir(d.clone()).scan().unwrap();
        let _ = std::fs::remove_dir_all(&d);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].title, "Death's Door");
    }
}
