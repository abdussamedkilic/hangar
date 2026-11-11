use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Steam,
    Epic,
    Ubisoft,
}

impl Platform {
    /// Priority order.
    pub const ALL: [Platform; 3] = [Platform::Steam, Platform::Epic, Platform::Ubisoft];

    pub fn as_str(&self) -> &'static str {
        match self {
            Platform::Steam => "steam",
            Platform::Epic => "epic",
            Platform::Ubisoft => "ubisoft",
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
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Game {
    pub id: i64,
    pub title: String,
    pub norm_title: String,
    pub cover_path: Option<String>,
    pub preferred: Option<Platform>,
    pub last_launched: Option<i64>,
    pub entries: Vec<PlatformEntry>,
}
