use super::{LibraryProvider, ProviderError};
use crate::db::LocalGame;
use crate::model::{Platform, PlatformEntry};
use std::path::Path;

/// Games the user added by hand (exe path + name).
pub struct LocalProvider {
    games: Vec<LocalGame>,
}

impl LocalProvider {
    pub fn new(games: Vec<LocalGame>) -> Self {
        Self { games }
    }
}

impl LibraryProvider for LocalProvider {
    fn platform(&self) -> Platform {
        Platform::Local
    }

    fn is_available(&self) -> bool {
        true
    }

    fn scan(&self) -> Result<Vec<PlatformEntry>, ProviderError> {
        Ok(self
            .games
            .iter()
            .map(|g| {
                let exe = Path::new(&g.exe_path);
                PlatformEntry {
                    platform: Platform::Local,
                    platform_id: g.id.to_string(),
                    title: g.title.clone(),
                    installed: exe.is_file(),
                    install_path: exe.parent().map(|d| d.to_string_lossy().into_owned()),
                    size_bytes: None,
                    last_played: None,
                    playtime_min: None,
                    launch_uri: g.exe_path.clone(),
                    install_uri: None,
                    launch_args: g.args.clone(),
                    cover_url: None,
                    edition: None,
                    copy: None,
                }
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::LocalGame;
    use crate::providers::LibraryProvider;

    fn game(id: i64, exe: &str) -> LocalGame {
        LocalGame { id, title: "Hades".into(), exe_path: exe.into(), args: vec![], source: "manual".into() }
    }

    #[test]
    fn missing_exe_is_not_installed() {
        let p = LocalProvider::new(vec![game(1, r"C:\nope\missing.exe")]);
        let e = &p.scan().unwrap()[0];
        assert!(!e.installed);
        assert_eq!(e.platform_id, "1");
    }

    #[test]
    fn existing_exe_is_installed() {
        let path = std::env::temp_dir().join("hangar_local_test.exe");
        std::fs::write(&path, b"x").unwrap();
        let s = path.to_string_lossy().to_string();
        let p = LocalProvider::new(vec![game(7, &s)]);
        let e = &p.scan().unwrap()[0];
        assert!(e.installed);
        assert_eq!(e.launch_uri, s);
        assert_eq!(e.install_path.as_deref(), path.parent().unwrap().to_str());
        let _ = std::fs::remove_file(&path);
    }
}
