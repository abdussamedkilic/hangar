use super::{LibraryProvider, ProviderError};
use crate::model::{Platform, PlatformEntry};

pub struct UbiInstall {
    pub id: String,
    pub install_dir: String,
}

pub trait InstallSource: Send + Sync {
    fn installs(&self) -> Vec<UbiInstall>;
}

#[cfg(windows)]
pub struct RegistrySource;

#[cfg(windows)]
const INSTALLS_KEY: &str = "SOFTWARE\\WOW6432Node\\Ubisoft\\Launcher\\Installs";

#[cfg(windows)]
impl InstallSource for RegistrySource {
    fn installs(&self) -> Vec<UbiInstall> {
        use winreg::{enums::HKEY_LOCAL_MACHINE, RegKey};
        let Ok(root) = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(INSTALLS_KEY) else {
            return vec![];
        };
        root.enum_keys()
            .flatten()
            .filter_map(|id| {
                let dir: String = root.open_subkey(&id).ok()?.get_value("InstallDir").ok()?;
                if !std::path::Path::new(&dir).exists() {
                    return None;
                }
                Some(UbiInstall { id, install_dir: dir })
            })
            .collect()
    }
}

pub struct UbisoftProvider {
    source: Box<dyn InstallSource>,
}

impl UbisoftProvider {
    pub fn new(source: Box<dyn InstallSource>) -> Self {
        Self { source }
    }

    #[cfg(windows)]
    pub fn detect() -> Option<Self> {
        use winreg::{enums::HKEY_LOCAL_MACHINE, RegKey};
        RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(INSTALLS_KEY).ok()?;
        Some(Self::new(Box::new(RegistrySource)))
    }

    #[cfg(not(windows))]
    pub fn detect() -> Option<Self> {
        None
    }
}

impl LibraryProvider for UbisoftProvider {
    fn platform(&self) -> Platform {
        Platform::Ubisoft
    }

    fn is_available(&self) -> bool {
        true
    }

    fn scan(&self) -> Result<Vec<PlatformEntry>, ProviderError> {
        Ok(self
            .source
            .installs()
            .into_iter()
            .filter_map(|i| {
                let title = i.install_dir.trim_end_matches(['/', '\\']).rsplit(['/', '\\']).next()?.to_string();
                if title.is_empty() {
                    return None;
                }
                Some(PlatformEntry {
                    platform: Platform::Ubisoft,
                    launch_uri: format!("uplay://launch/{}/0", i.id),
                    platform_id: i.id,
                    title,
                    installed: true,
                    install_path: Some(i.install_dir),
                    size_bytes: None,
                    last_played: None,
                    playtime_min: None,
                    install_uri: Some("uplay://".to_string()),
                })
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake(Vec<(&'static str, &'static str)>);
    impl InstallSource for Fake {
        fn installs(&self) -> Vec<UbiInstall> {
            self.0
                .iter()
                .map(|(i, d)| UbiInstall { id: i.to_string(), install_dir: d.to_string() })
                .collect()
        }
    }

    fn scan(v: Vec<(&'static str, &'static str)>) -> Vec<PlatformEntry> {
        UbisoftProvider::new(Box::new(Fake(v))).scan().unwrap()
    }

    #[test]
    fn entry_from_install() {
        let got = scan(vec![("4923", "D:/Games/Assassin's Creed Valhalla/")]);
        assert_eq!(got.len(), 1);
        let e = &got[0];
        assert_eq!(e.platform, Platform::Ubisoft);
        assert_eq!(e.platform_id, "4923");
        assert_eq!(e.title, "Assassin's Creed Valhalla");
        assert!(e.installed);
        assert_eq!(e.install_path.as_deref(), Some("D:/Games/Assassin's Creed Valhalla/"));
        assert_eq!(e.launch_uri, "uplay://launch/4923/0");
        assert_eq!(e.install_uri.as_deref(), Some("uplay://"));
    }

    #[test]
    fn backslash_paths() {
        let got = scan(vec![("1", r"C:\Games\Far Cry 6\\")]);
        assert_eq!(got[0].title, "Far Cry 6");
    }

    #[test]
    fn empty_source_is_empty() {
        assert!(scan(vec![]).is_empty());
    }

    #[test]
    fn skips_missing_install_dir() {
        let got = scan(vec![("541", ""), ("4923", "D:/Games/Valhalla")]);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].platform_id, "4923");
    }
}
