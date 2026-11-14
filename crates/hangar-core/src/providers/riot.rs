use super::{LibraryProvider, ProviderError};
use crate::model::{Platform, PlatformEntry};
use std::path::PathBuf;

pub fn parse_client_path(json: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let p = v.get("rc_default")?.as_str()?;
    valid_client_path(p).then(|| p.to_string())
}

/// The json is user-writable and the value gets executed: only a local
/// absolute `...RiotClientServices.exe` is accepted.
fn valid_client_path(p: &str) -> bool {
    let b = p.as_bytes();
    let drive = b.len() > 2 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'/' || b[2] == b'\\');
    let name = p.rsplit(['/', '\\']).next().unwrap_or("");
    drive && !p.contains("://") && name.eq_ignore_ascii_case("RiotClientServices.exe")
}

/// Top-level `product_install_full_path` (line-based; no YAML crate).
pub fn install_path_from_yaml(yaml: &str) -> Option<String> {
    let rest = yaml.lines().find_map(|l| l.strip_prefix("product_install_full_path:"))?;
    let v = rest.trim().trim_matches(['"', '\'']);
    (!v.is_empty()).then(|| v.to_string())
}

pub fn product_title(product: &str, patchline: &str) -> Option<String> {
    let name = match product {
        "league_of_legends" => "League of Legends",
        "teamfighttactics" => "Teamfight Tactics",
        "valorant" => "VALORANT",
        _ => return None,
    };
    match patchline {
        "live" => Some(name.to_string()),
        "pbe" => Some(format!("{name} (PBE)")),
        _ => None,
    }
}

pub const DEFAULT_ROOT: &str = r"C:\ProgramData\Riot Games";

pub struct RiotProvider {
    root: PathBuf,
}

impl RiotProvider {
    pub fn with_root(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn detect() -> Option<Self> {
        let root = PathBuf::from(DEFAULT_ROOT);
        root.join("RiotClientInstalls.json").is_file().then(|| Self::with_root(root))
    }
}

impl LibraryProvider for RiotProvider {
    fn platform(&self) -> Platform {
        Platform::Riot
    }

    fn is_available(&self) -> bool {
        true
    }

    fn scan(&self) -> Result<Vec<PlatformEntry>, ProviderError> {
        let Some(client) = std::fs::read_to_string(self.root.join("RiotClientInstalls.json"))
            .ok()
            .and_then(|j| parse_client_path(&j))
            .filter(|c| std::path::Path::new(c).is_file())
        else {
            return Ok(vec![]);
        };
        Ok(self.scan_products(&client))
    }
}

impl RiotProvider {
    /// `client` must already be validated (see `parse_client_path`).
    fn scan_products(&self, client: &str) -> Vec<PlatformEntry> {
        let Ok(dirs) = std::fs::read_dir(self.root.join("Metadata")) else {
            return vec![];
        };
        let mut out: Vec<PlatformEntry> = dirs
            .flatten()
            .filter_map(|d| {
                let name = d.file_name().to_string_lossy().into_owned();
                let (product, patchline) = name.split_once('.')?;
                let title = product_title(product, patchline)?;
                let yaml = std::fs::read_to_string(d.path().join(format!("{name}.product_settings.yaml"))).ok()?;
                let install = install_path_from_yaml(&yaml)?;
                if !std::path::Path::new(&install).exists() {
                    return None;
                }
                Some(PlatformEntry {
                    platform: Platform::Riot,
                    platform_id: name.clone(),
                    title,
                    launch_uri: client.to_string(),
                    installed: true,
                    install_path: Some(install),
                    size_bytes: None,
                    last_played: None,
                    playtime_min: None,
                    install_uri: None,
                    launch_args: vec![
                        format!("--launch-product={product}"),
                        format!("--launch-patchline={patchline}"),
                    ],
                    cover_url: None,
                    edition: None,
                    copy: None,
                })
            })
            .collect();
        out.sort_by(|a, b| a.platform_id.cmp(&b.platform_id));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    const JSON: &str = include_str!("../../tests/fixtures/riot/RiotClientInstalls.json");
    const YAML: &str = include_str!("../../tests/fixtures/riot/league_of_legends.live.product_settings.yaml");

    #[test]
    fn titles() {
        assert_eq!(product_title("league_of_legends", "live").as_deref(), Some("League of Legends"));
        assert_eq!(product_title("teamfighttactics", "live").as_deref(), Some("Teamfight Tactics"));
        assert_eq!(product_title("valorant", "live").as_deref(), Some("VALORANT"));
        assert_eq!(product_title("league_of_legends", "pbe").as_deref(), Some("League of Legends (PBE)"));
        assert_eq!(product_title("bacon", "live"), None);
    }

    #[test]
    fn client_path() {
        assert_eq!(parse_client_path(JSON).as_deref(), Some("C:/Riot Games/Riot Client/RiotClientServices.exe"));
        assert_eq!(parse_client_path("not json"), None);
    }

    #[test]
    fn client_path_rejects_unsafe() {
        let j = |p: &str| format!("{{\"rc_default\": {}}}", serde_json::to_string(p).unwrap());
        for bad in [
            "https://evil/x.exe",
            r"\\server\share\RiotClientServices.exe",
            "//server/share/RiotClientServices.exe",
            "C:/Windows/System32/cmd.exe",
            "RiotClientServices.exe",
            "/RiotClientServices.exe",
        ] {
            assert_eq!(parse_client_path(&j(bad)), None, "{bad}");
        }
        assert!(parse_client_path(&j("c:/Riot Games/Riot Client/riotclientservices.EXE")).is_some());
    }

    #[test]
    fn scan_requires_client_file() {
        let root = setup("noclient");
        let lol = root.join("lol");
        fs::create_dir_all(&lol).unwrap();
        product(&root, "league_of_legends.live", &lol);
        // valid on every OS (drive-letter form) but never exists
        let json = r#"{"rc_default": "Z:/hangar-nope/RiotClientServices.exe"}"#;
        fs::write(root.join("RiotClientInstalls.json"), json).unwrap();
        assert_eq!(RiotProvider::with_root(root.clone()).scan().unwrap(), vec![]);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn yaml_path() {
        assert_eq!(install_path_from_yaml(YAML).as_deref(), Some("C:/Riot Games/League of Legends"));
        assert_eq!(install_path_from_yaml("nothing: 1\n"), None);
    }

    fn setup(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("hangar-riot-{}-{}", tag, std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn product(root: &std::path::Path, name: &str, install: &std::path::Path) {
        let dir = root.join("Metadata").join(name);
        fs::create_dir_all(&dir).unwrap();
        let yaml = format!("product_install_full_path: \"{}\"\n", install.to_string_lossy().replace('\\', "/"));
        fs::write(dir.join(format!("{name}.product_settings.yaml")), yaml).unwrap();
    }

    #[test]
    fn scan_emits_only_installed_known_products() {
        let root = setup("scan");
        let client_s = "C:/Riot Games/Riot Client/RiotClientServices.exe";
        let lol = root.join("games").join("League of Legends");
        fs::create_dir_all(&lol).unwrap();
        product(&root, "league_of_legends.live", &lol);
        product(&root, "league_of_legends.pbe", &root.join("games").join("missing"));
        product(&root, "league_of_legends.live.game_patch", &lol);
        product(&root, "bacon.live", &lol);
        fs::create_dir_all(root.join("Metadata").join("lion.live")).unwrap();

        let got = RiotProvider::with_root(root.clone()).scan_products(client_s);
        assert_eq!(got.len(), 1);
        let e = &got[0];
        assert_eq!(e.platform, Platform::Riot);
        assert_eq!(e.platform_id, "league_of_legends.live");
        assert_eq!(e.title, "League of Legends");
        assert!(e.installed);
        assert_eq!(e.launch_uri, client_s);
        assert_eq!(e.launch_args, vec!["--launch-product=league_of_legends", "--launch-patchline=live"]);
        assert_eq!(e.install_path.as_deref(), Some(lol.to_string_lossy().replace('\\', "/").as_str()));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn pbe_install_is_a_separate_entry() {
        const PBE_YAML: &str = include_str!("../../tests/fixtures/riot/league_of_legends.pbe.product_settings.yaml");
        assert_eq!(install_path_from_yaml(PBE_YAML).as_deref(), Some("C:/Riot Games/League of Legends (PBE)"));
        let root = setup("pbe");
        let live = root.join("games").join("League of Legends");
        let pbe = root.join("games").join("League of Legends (PBE)");
        fs::create_dir_all(&live).unwrap();
        fs::create_dir_all(&pbe).unwrap();
        product(&root, "league_of_legends.live", &live);
        product(&root, "league_of_legends.pbe", &pbe);
        let got = RiotProvider::with_root(root.clone()).scan_products("C:/Riot Games/Riot Client/RiotClientServices.exe");
        let ids: Vec<_> = got.iter().map(|e| e.platform_id.as_str()).collect();
        assert_eq!(ids, ["league_of_legends.live", "league_of_legends.pbe"]);
        assert_eq!(got[1].title, "League of Legends (PBE)");
        assert_eq!(got[1].launch_args, vec!["--launch-product=league_of_legends", "--launch-patchline=pbe"]);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn missing_client_json_is_empty() {
        let root = setup("nojson");
        assert_eq!(RiotProvider::with_root(root.clone()).scan().unwrap(), vec![]);
        let _ = fs::remove_dir_all(&root);
    }
}
