use serde::{Deserialize, Serialize};
use std::fmt;
use zeroize::Zeroize;

pub mod epic;
pub mod epic_api;
pub mod gog;
pub mod gog_api;
pub mod login;
pub mod sanitize;
pub mod steam;
pub mod store;
pub mod ubi_probe;
pub mod ubi_demux;
pub mod ubisoft;
pub mod ubisoft_api;

#[derive(Debug, Clone, PartialEq)]
pub enum AuthError {
    NotConnected,
    Corrupt,
    InvalidGrant,
    /// The credential could not be written to disk.
    Save,
    Network(String),
    Parse(String),
}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuthError::NotConnected => write!(f, "Hesap bağlı değil"),
            AuthError::Corrupt => write!(f, "Bağlantı okunamadı"),
            AuthError::InvalidGrant => write!(f, "Bağlantının süresi doldu"),
            AuthError::Save => write!(f, "Bağlantı kaydedilemedi"),
            AuthError::Network(s) => write!(f, "Sunucuya ulaşılamadı ({s})"),
            AuthError::Parse(s) => write!(f, "Yanıt anlaşılamadı ({s})"),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct StoredCred {
    pub v: u32,
    pub refresh_token: String,
    pub refresh_expires_at: String,
    pub account_id: String,
    pub display_name: String,
}

impl fmt::Debug for StoredCred {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StoredCred")
            .field("v", &self.v)
            .field("refresh_token", &"***")
            .field("refresh_expires_at", &self.refresh_expires_at)
            .field("account_id", &"***")
            .field("display_name", &self.display_name)
            .finish()
    }
}

impl Drop for StoredCred {
    fn drop(&mut self) {
        self.refresh_token.zeroize();
    }
}

/// GOG credential: the Galaxy refresh token (access tokens stay in memory).
#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct GogCred {
    pub v: u32,
    pub refresh_token: String,
    pub user_id: String,
    pub display_name: String,
}

impl fmt::Debug for GogCred {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GogCred")
            .field("v", &self.v)
            .field("refresh_token", &"***")
            .field("user_id", &"***")
            .field("display_name", &self.display_name)
            .finish()
    }
}

impl Drop for GogCred {
    fn drop(&mut self) {
        self.refresh_token.zeroize();
    }
}

/// Steam credential: only the account identity (the web session lives in the persistent webview profile).
#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct SteamCred {
    pub v: u32,
    pub steamid: String,
    pub display_name: String,
}

impl fmt::Debug for SteamCred {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SteamCred")
            .field("v", &self.v)
            .field("steamid", &"***")
            .field("display_name", &self.display_name)
            .finish()
    }
}

impl Drop for SteamCred {
    fn drop(&mut self) {
        self.steamid.zeroize();
    }
}

/// Ubisoft credential: only the long-lived remember-me ticket is persisted (tickets/sessions stay in memory).
#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct UbiCred {
    pub v: u32,
    pub remember_me_ticket: String,
    pub user_id: String,
    pub display_name: String,
}

impl fmt::Debug for UbiCred {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UbiCred")
            .field("v", &self.v)
            .field("remember_me_ticket", &"***")
            .field("user_id", &"***")
            .field("display_name", &self.display_name)
            .finish()
    }
}

impl Drop for UbiCred {
    fn drop(&mut self) {
        self.remember_me_ticket.zeroize();
    }
}

pub struct Tokens {
    pub access_token: String,
    pub cred: StoredCred,
}

impl fmt::Debug for Tokens {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Tokens")
            .field("access_token", &"***")
            .field("cred", &self.cred)
            .finish()
    }
}

impl Drop for Tokens {
    fn drop(&mut self) {
        self.access_token.zeroize();
    }
}

pub fn parse_token(json: &str) -> Result<Tokens, AuthError> {
    let bad = || AuthError::Parse("token".into());
    let v: serde_json::Value = serde_json::from_str(json).map_err(|_| bad())?;
    let s = |k: &str| v.get(k).and_then(|x| x.as_str()).map(str::to_string).ok_or_else(bad);
    Ok(Tokens {
        access_token: s("access_token")?,
        cred: StoredCred {
            v: 1,
            refresh_token: s("refresh_token")?,
            refresh_expires_at: s("refresh_expires_at")?,
            account_id: s("account_id")?,
            display_name: s("displayName")?,
        },
    })
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_token_reads_fields() {
        let j = r#"{"access_token":"AT","expires_at":"2026-11-12T20:00:00Z","refresh_token":"RT","refresh_expires_at":"2026-12-12T20:00:00Z","account_id":"acc1","displayName":"oyuncu"}"#;
        let t = parse_token(j).unwrap();
        assert_eq!(t.access_token, "AT");
        assert_eq!(t.cred.refresh_token, "RT");
        assert_eq!(t.cred.display_name, "oyuncu");
        assert_eq!(t.cred.v, 1);
        assert!(matches!(parse_token("{}"), Err(AuthError::Parse(ref s)) if s == "token"));
    }
    #[test]
    fn debug_and_errors_never_show_tokens() {
        let t = parse_token(r#"{"access_token":"AT-SECRET","refresh_token":"RT-SECRET","refresh_expires_at":"x","account_id":"ACC-SECRET","displayName":"d"}"#).unwrap();
        let s = format!("{t:?} {:?}", t.cred);
        for bad in ["AT-SECRET", "RT-SECRET", "ACC-SECRET"] {
            assert!(!s.contains(bad));
        }
        assert!(!AuthError::Network("durum 500".into()).to_string().contains("http"));
    }
    #[test]
    fn basic_auth_matches_client() {
        use base64::Engine;
        assert_eq!(
            base64::engine::general_purpose::STANDARD.encode("34a02cf8f4414e29b15921876da36f9a:daafbccc737745039dffe53d94fc76cf"),
            epic_api::BASIC_AUTH
        );
    }
}
