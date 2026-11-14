//! Ubisoft U0 keşfi (yalnızca debug probe kullanır): ağ çağrıları ve yanıt özetleri.
//! Gizli değerleri (ticket, oturum, kullanıcı) asla Display/log etmez.

use crate::accounts::login::UBI_APP_ID;
use serde_json::Value;
use std::time::Duration;

const GRAPHQL_URL: &str = "https://public-ubiservices.ubi.com/v1/profiles/me/uplay/graphql";
const SESSIONS_URL: &str = "https://public-ubiservices.ubi.com/v3/profiles/sessions";
const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
const TIMEOUT: Duration = Duration::from_secs(20);

pub use crate::accounts::ubisoft_api::{owned_games_body, parse_login, UbiLogin};

pub fn game_fields_body() -> String {
    serde_json::json!({"query": "{ __type(name:\"Game\"){ fields { name type { name kind ofType { name } } } } }"}).to_string()
}

// Hata metnine URL/başlık sızmasın diye yalnızca durum kodu/genel metin.
fn net_err(e: ureq::Error) -> String {
    match e {
        ureq::Error::Status(c, _) => format!("durum {c}"),
        ureq::Error::Transport(_) => "ağ hatası".into(),
    }
}

pub fn graphql(l: &UbiLogin, body: &str) -> Result<String, String> {
    let r = ureq::post(GRAPHQL_URL)
        .timeout(TIMEOUT)
        .set("Authorization", &format!("Ubi_v1 t={}", l.ticket))
        .set("Ubi-AppId", UBI_APP_ID)
        .set("Ubi-SessionId", &l.session_id)
        .set("Content-Type", "application/json")
        .set("User-Agent", UA)
        .send_string(body)
        .map_err(net_err)?;
    r.into_string().map_err(|_| "yanıt okunamadı".into())
}

/// Yenileme denemesi; yalnızca HTTP durum metni döner.
pub fn refresh_status(l: &UbiLogin) -> String {
    if l.remember_ticket.is_empty() {
        return "rememberMeTicket yok (atlandı)".into();
    }
    let r = ureq::post(SESSIONS_URL)
        .timeout(TIMEOUT)
        .set("Authorization", &format!("rm_v1 t={}", l.remember_ticket))
        .set("Ubi-AppId", UBI_APP_ID)
        .set("Content-Type", "application/json")
        .set("Origin", "https://connect.ubisoft.com")
        .set("Referer", "https://connect.ubisoft.com")
        .set("User-Agent", UA)
        .send_string(r#"{"rememberMe":true}"#);
    match r {
        Ok(r) => format!("HTTP {}", r.status()),
        Err(ureq::Error::Status(c, _)) => format!("HTTP {c}"),
        Err(_) => "ağ hatası".into(),
    }
}

fn has_pc(v: &Value) -> bool {
    match v {
        Value::Object(m) => m.get("type").and_then(Value::as_str) == Some("PC") || m.values().any(has_pc),
        Value::Array(a) => a.iter().any(has_pc),
        _ => false,
    }
}

/// (sahip olunan sayısı, PC sahibi sayısı).
pub fn count_owned(body: &str) -> (usize, usize) {
    let v: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let nodes = v["data"]["viewer"]["ownedGames"]["nodes"].as_array().cloned().unwrap_or_default();
    let pc = nodes.iter().filter(|n| has_pc(&n["viewer"]["meta"]["ownedPlatformGroups"])).count();
    (nodes.len(), pc)
}

pub fn field_names(body: &str) -> Vec<String> {
    let v: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    v["data"]["__type"]["fields"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|f| f["name"].as_str().map(str::to_string))
        .collect()
}

/// Dizindeki dosyaların adı + boyutu (yalnızca okuma).
pub fn list_dir(path: &std::path::Path) -> String {
    match std::fs::read_dir(path) {
        Err(_) => format!("{}: yok/okunamadı", path.display()),
        Ok(rd) => {
            let items: Vec<String> = rd
                .flatten()
                .map(|e| format!("{} ({} B)", e.file_name().to_string_lossy(), e.metadata().map(|m| m.len()).unwrap_or(0)))
                .collect();
            format!("{}: {}", path.display(), if items.is_empty() { "boş".into() } else { items.join(", ") })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn counts() {
        let b = r#"{"data":{"viewer":{"ownedGames":{"nodes":[{"viewer":{"meta":{"ownedPlatformGroups":[[{"type":"PC"}]]}}},{"viewer":{"meta":{"ownedPlatformGroups":[[{"type":"XBOX"}]]}}}]}}}}"#;
        assert_eq!(count_owned(b), (2, 1));
        assert_eq!(field_names(r#"{"data":{"__type":{"fields":[{"name":"id"},{"name":"name"}]}}}"#), vec!["id", "name"]);
    }
}
