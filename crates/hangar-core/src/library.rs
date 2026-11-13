use crate::db::Db;
use crate::merge::group;
use crate::model::{Game, Platform, PlatformEntry};
use crate::providers::{LibraryProvider, ProviderError};
use std::path::PathBuf;

pub struct ScanReport {
    pub games: usize,
    pub warnings: Vec<String>,
}

pub type OwnedLists = Vec<(Platform, Result<Vec<PlatformEntry>, ProviderError>)>;

pub fn scan_all(
    db: &Db,
    providers: &[Box<dyn LibraryProvider>],
    owned: OwnedLists,
) -> rusqlite::Result<ScanReport> {
    let mut warnings = Vec::new();
    let mut entries: Vec<PlatformEntry> = Vec::new();
    for p in providers {
        match p.scan() {
            Ok(list) => entries.extend(list),
            Err(e) => warnings.push(format!("{:?} taranamadı: {e}", p.platform())),
        }
    }
    for (platform, result) in owned {
        match result {
            Ok(list) => {
                for o in list {
                    let installed = entries
                        .iter_mut()
                        .find(|e| e.platform == platform && e.platform_id == o.platform_id);
                    match installed {
                        Some(e) => {
                            if o.playtime_min.is_some() {
                                e.playtime_min = o.playtime_min;
                            }
                            e.last_played = e.last_played.or(o.last_played);
                            if o.install_uri.is_some() {
                                e.install_uri = o.install_uri;
                            }
                        }
                        None => entries.push(o),
                    }
                }
            }
            Err(_) if platform == Platform::Steam => warnings.push(
                "Steam sahip olunan oyunlar alınamadı: API anahtarını kontrol et.".to_string(),
            ),
            Err(_) => warnings.push(format!("{:?} sahip olunan oyunlar alınamadı.", platform)),
        }
    }
    let groups = group(entries, &db.overrides()?);
    let games = groups.len();
    db.replace_entries(groups)?;
    Ok(ScanReport { games, warnings })
}

#[derive(Debug, Clone, PartialEq)]
pub enum LaunchTarget {
    Uri(String),
    Exe { path: PathBuf, args: Vec<String> },
}

/// A launch string containing `://` is a URI; anything else is an exe path.
pub fn launch_target(entry: &PlatformEntry, launch: &str) -> LaunchTarget {
    if launch.contains("://") {
        LaunchTarget::Uri(launch.to_string())
    } else {
        LaunchTarget::Exe { path: PathBuf::from(launch), args: entry.launch_args.clone() }
    }
}

pub fn resolve(game: &Game, want: Option<Platform>) -> Option<(&PlatformEntry, String)> {
    let find = |p: Platform| game.entries.iter().find(|e| e.platform == p);
    let e = want
        .and_then(find)
        .or_else(|| game.preferred.and_then(find))
        .or_else(|| Platform::ALL.into_iter().find_map(find))?;
    let uri = if e.installed {
        e.launch_uri.clone()
    } else {
        e.install_uri
            .clone()
            .unwrap_or_else(|| e.launch_uri.clone())
    };
    Some((e, uri))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::merge::Override;

    struct Fake(Platform, Result<Vec<PlatformEntry>, ProviderError>);
    impl LibraryProvider for Fake {
        fn platform(&self) -> Platform {
            self.0
        }
        fn is_available(&self) -> bool {
            true
        }
        fn scan(&self) -> Result<Vec<PlatformEntry>, ProviderError> {
            self.1.clone()
        }
    }

    fn entry(p: Platform, id: &str, title: &str, installed: bool) -> PlatformEntry {
        PlatformEntry {
            platform: p,
            platform_id: id.into(),
            title: title.into(),
            installed,
            install_path: None,
            size_bytes: None,
            last_played: None,
            playtime_min: None,
            launch_uri: format!("launch:{id}"),
            install_uri: Some(format!("install:{id}")),
            launch_args: vec![],
        }
    }

    fn fake(p: Platform, v: Vec<PlatformEntry>) -> Box<dyn LibraryProvider> {
        Box::new(Fake(p, Ok(v)))
    }

    #[test]
    fn uri_vs_exe() {
        let e = entry(Platform::Steam, "1", "Hades", true);
        assert_eq!(
            launch_target(&e, "steam://rungameid/1"),
            LaunchTarget::Uri("steam://rungameid/1".into())
        );
        let e = PlatformEntry { launch_args: vec!["--x".into()], ..e };
        assert_eq!(
            launch_target(&e, r"C:\Oyunlar\Şehir Yapımı\game.exe"),
            LaunchTarget::Exe {
                path: r"C:\Oyunlar\Şehir Yapımı\game.exe".into(),
                args: vec!["--x".into()]
            }
        );
    }

    #[test]
    fn steam_default_on_merged_card() {
        let g = Game {
            id: 1,
            title: "Hades".into(),
            norm_title: "hades".into(),
            cover_path: None,
            preferred: None,
            last_launched: None,
            entries: vec![
                entry(Platform::Riot, "r", "Hades", true),
                entry(Platform::Steam, "1", "Hades", true),
                entry(Platform::Local, "l", "Hades", true),
            ],
        };
        assert_eq!(resolve(&g, None).unwrap().0.platform, Platform::Steam);
    }

    #[test]
    fn replace_preserves_preference() {
        let db = Db::open_in_memory().unwrap();
        let provs = vec![
            fake(
                Platform::Steam,
                vec![entry(Platform::Steam, "1", "Hades", true)],
            ),
            fake(
                Platform::Epic,
                vec![entry(Platform::Epic, "e", "Hades", true)],
            ),
        ];
        scan_all(&db, &provs, vec![]).unwrap();
        let id = db.games().unwrap()[0].id;
        db.set_preferred(id, Platform::Epic).unwrap();
        scan_all(&db, &provs, vec![]).unwrap();
        let g = &db.games().unwrap()[0];
        assert_eq!(g.id, id);
        assert_eq!(g.preferred, Some(Platform::Epic));
    }

    #[test]
    fn vanished_game_removed() {
        let db = Db::open_in_memory().unwrap();
        let a = entry(Platform::Steam, "1", "Alpha", true);
        let b = entry(Platform::Steam, "2", "Beta", true);
        scan_all(&db, &[fake(Platform::Steam, vec![a.clone(), b])], vec![]).unwrap();
        assert_eq!(db.games().unwrap().len(), 2);
        scan_all(&db, &[fake(Platform::Steam, vec![a])], vec![]).unwrap();
        let games = db.games().unwrap();
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].title, "Alpha");
    }

    #[test]
    fn owned_does_not_clobber_installed() {
        let db = Db::open_in_memory().unwrap();
        let installed = entry(Platform::Steam, "1", "Hades", true);
        let mut owned = entry(Platform::Steam, "1", "Hades", false);
        owned.playtime_min = Some(120);
        owned.last_played = Some(1000);
        let r = scan_all(
            &db,
            &[fake(Platform::Steam, vec![installed])],
            vec![(Platform::Steam, Ok(vec![owned]))],
        )
        .unwrap();
        assert_eq!(r.games, 1);
        let e = &db.games().unwrap()[0].entries[0];
        assert!(e.installed);
        assert_eq!(e.playtime_min, Some(120));
        assert_eq!(e.last_played, Some(1000));
    }

    #[test]
    fn owned_error_is_warning() {
        let db = Db::open_in_memory().unwrap();
        let provs = vec![fake(
            Platform::Steam,
            vec![entry(Platform::Steam, "1", "Hades", true)],
        )];
        let r = scan_all(&db, &provs, vec![(Platform::Steam, Err(ProviderError::Network("x".into())))]).unwrap();
        assert_eq!(
            r.warnings,
            vec!["Steam sahip olunan oyunlar alınamadı: API anahtarını kontrol et.".to_string()]
        );
        assert_eq!(db.games().unwrap().len(), 1);
    }

    #[test]
    fn owned_epic_keeps_manifest_title() {
        let db = Db::open_in_memory().unwrap();
        let installed = entry(Platform::Epic, "Fig", "Death's Door", true);
        let mut owned = entry(Platform::Epic, "Fig", "Deaths Door", false);
        owned.install_uri = Some("com.epicgames.launcher://apps/ns%3Aid%3AFig?action=install".into());
        let other = entry(Platform::Epic, "Owl", "Owlboy", false);
        let r = scan_all(&db, &[fake(Platform::Epic, vec![installed])],
            vec![(Platform::Epic, Ok(vec![owned, other]))]).unwrap();
        assert_eq!(r.games, 2);
        let games = db.games().unwrap();
        let g = games.iter().find(|g| g.title == "Death's Door").unwrap();
        assert_eq!(g.entries.len(), 1);
        assert!(g.entries[0].installed);
        assert_eq!(g.entries[0].install_uri.as_deref(), Some("com.epicgames.launcher://apps/ns%3Aid%3AFig?action=install"));
        assert!(games.iter().any(|g| g.title == "Owlboy" && !g.entries[0].installed));
    }

    #[test]
    fn owned_ids_do_not_cross_platforms() {
        let db = Db::open_in_memory().unwrap();
        scan_all(&db, &[fake(Platform::Steam, vec![entry(Platform::Steam, "1", "Hades", true)])],
            vec![(Platform::Epic, Ok(vec![entry(Platform::Epic, "1", "Hades", false)]))]).unwrap();
        let g = &db.games().unwrap()[0];
        assert_eq!(g.entries.len(), 2);
        assert!(g.entries.iter().any(|e| e.platform == Platform::Steam && e.installed));
        assert!(g.entries.iter().any(|e| e.platform == Platform::Epic && !e.installed));
    }

    #[test]
    fn non_steam_owned_error_warning() {
        let db = Db::open_in_memory().unwrap();
        let r = scan_all(&db, &[], vec![(Platform::Epic, Err(ProviderError::Network("x".into())))]).unwrap();
        assert_eq!(r.warnings, vec!["Epic sahip olunan oyunlar alınamadı.".to_string()]);
    }

    #[test]
    fn provider_error_is_warning() {
        let db = Db::open_in_memory().unwrap();
        let provs: Vec<Box<dyn LibraryProvider>> = vec![
            Box::new(Fake(Platform::Epic, Err(ProviderError::Io("boom".into())))),
            fake(
                Platform::Steam,
                vec![entry(Platform::Steam, "1", "Hades", true)],
            ),
        ];
        let r = scan_all(&db, &provs, vec![]).unwrap();
        assert_eq!(r.warnings.len(), 1);
        assert!(r.warnings[0].contains("taranamadı"));
        assert_eq!(db.games().unwrap().len(), 1);
    }

    fn game_with(entries: Vec<PlatformEntry>, preferred: Option<Platform>) -> Game {
        Game {
            id: 1,
            title: "T".into(),
            norm_title: "t".into(),
            cover_path: None,
            preferred,
            last_launched: None,
            entries,
        }
    }

    #[test]
    fn resolve_defaults_to_steam() {
        let g = game_with(
            vec![
                entry(Platform::Steam, "1", "T", true),
                entry(Platform::Epic, "e", "T", true),
            ],
            None,
        );
        let (e, uri) = resolve(&g, None).unwrap();
        assert_eq!(e.platform, Platform::Steam);
        assert_eq!(uri, "launch:1");
        let g2 = game_with(g.entries.clone(), Some(Platform::Epic));
        assert_eq!(resolve(&g2, None).unwrap().0.platform, Platform::Epic);
        assert_eq!(
            resolve(&g2, Some(Platform::Ubisoft)).unwrap().0.platform,
            Platform::Epic
        );
    }

    #[test]
    fn resolve_uninstalled_returns_install_uri() {
        let g = game_with(vec![entry(Platform::Epic, "e", "T", false)], None);
        assert_eq!(resolve(&g, None).unwrap().1, "install:e");
    }

    #[test]
    fn split_override_survives_rescan() {
        let db = Db::open_in_memory().unwrap();
        db.add_override(Override {
            a: (Platform::Steam, "1".into()),
            b: (Platform::Epic, "e".into()),
            merged: false,
        })
        .unwrap();
        let provs = vec![
            fake(
                Platform::Steam,
                vec![entry(Platform::Steam, "1", "Hades", true)],
            ),
            fake(
                Platform::Epic,
                vec![entry(Platform::Epic, "e", "Hades", true)],
            ),
        ];
        assert_eq!(scan_all(&db, &provs, vec![]).unwrap().games, 2);
        assert_eq!(db.games().unwrap().len(), 2);
        scan_all(&db, &provs, vec![]).unwrap();
        assert_eq!(db.games().unwrap().len(), 2);
    }

    #[test]
    fn local_provider_rescan_is_stable() {
        use crate::db::LocalGame;
        use crate::providers::local::LocalProvider;
        let db = Db::open_in_memory().unwrap();
        let provs: Vec<Box<dyn LibraryProvider>> = vec![Box::new(LocalProvider::new(vec![LocalGame {
            id: 1,
            title: "Hades".into(),
            exe_path: "x.exe".into(),
            args: vec![],
            source: "manual".into(),
        }]))];
        assert_eq!(scan_all(&db, &provs, vec![]).unwrap().games, 1);
        assert_eq!(scan_all(&db, &provs, vec![]).unwrap().games, 1);
        let games = db.games().unwrap();
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].entries.len(), 1);
    }

    #[test]
    fn merge_after_split_wins() {
        let db = Db::open_in_memory().unwrap();
        let ov = |merged| Override {
            a: (Platform::Steam, "1".into()),
            b: (Platform::Epic, "e".into()),
            merged,
        };
        let provs = vec![
            fake(Platform::Steam, vec![entry(Platform::Steam, "1", "Hades", true)]),
            fake(Platform::Epic, vec![entry(Platform::Epic, "e", "Hades", true)]),
        ];
        db.add_override(ov(false)).unwrap();
        scan_all(&db, &provs, vec![]).unwrap();
        assert_eq!(db.games().unwrap().len(), 2);
        let mut rev = ov(true);
        std::mem::swap(&mut rev.a, &mut rev.b);
        db.add_override(rev).unwrap();
        scan_all(&db, &provs, vec![]).unwrap();
        assert_eq!(db.games().unwrap().len(), 1);
    }
}
