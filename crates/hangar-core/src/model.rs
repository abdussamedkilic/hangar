use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Steam,
    Epic,
    Ubisoft,
    Gog,
    Riot,
    Local,
}

impl Platform {
    /// Priority order.
    pub const ALL: [Platform; 6] = [
        Platform::Steam,
        Platform::Epic,
        Platform::Ubisoft,
        Platform::Gog,
        Platform::Riot,
        Platform::Local,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            Platform::Steam => "steam",
            Platform::Epic => "epic",
            Platform::Ubisoft => "ubisoft",
            Platform::Gog => "gog",
            Platform::Riot => "riot",
            Platform::Local => "local",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PlatformEntry {
    pub platform: Platform,
    pub platform_id: String,
    pub title: String,
    pub installed: bool,
    pub install_path: Option<String>,
    pub size_bytes: Option<u64>,
    /// Unix seconds.
    pub last_played: Option<i64>,
    pub playtime_min: Option<u32>,
    pub launch_uri: String,
    pub install_uri: Option<String>,
    /// Extra arguments when `launch_uri` is an exe path.
    #[serde(default)]
    pub launch_args: Vec<String>,
    /// Public cover image URL (Epic/GOG); old stored JSON lacks it.
    #[serde(default)]
    pub cover_url: Option<String>,
    /// Edition label ("Remastered"...), `None` for Original. Computed by `Db::games()`.
    #[serde(default)]
    pub edition: Option<String>,
    /// Copy marker ("Prime"). Computed by `Db::games()`.
    #[serde(default)]
    pub copy: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Game {
    pub id: i64,
    pub title: String,
    pub norm_title: String,
    pub cover_path: Option<String>,
    /// Preferred entry as (platform, platform_id).
    pub preferred: Option<(Platform, String)>,
    /// The entry Play uses: preferred, else latest-played installed, else menu order.
    #[serde(default)]
    pub default_entry: Option<(Platform, String)>,
    pub last_launched: Option<i64>,
    pub entries: Vec<PlatformEntry>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_priority() {
        use Platform::*;
        assert_eq!(Platform::ALL, [Steam, Epic, Ubisoft, Gog, Riot, Local]);
    }

    #[test]
    fn old_json_without_launch_args_still_loads() {
        let j = r#"{"platform":"steam","platform_id":"1","title":"Hades","installed":true,"install_path":null,"size_bytes":null,"last_played":null,"playtime_min":null,"launch_uri":"steam://rungameid/1","install_uri":null}"#;
        let e: PlatformEntry = serde_json::from_str(j).unwrap();
        assert!(e.launch_args.is_empty());
    }

    #[test]
    fn old_json_without_cover_url_still_loads() {
        let j = r#"{"platform":"gog","platform_id":"1","title":"Prey","installed":false,"install_path":null,"size_bytes":null,"last_played":null,"playtime_min":null,"launch_uri":"x","install_uri":null,"launch_args":[]}"#;
        let e: PlatformEntry = serde_json::from_str(j).unwrap();
        assert!(e.cover_url.is_none());
    }
}
