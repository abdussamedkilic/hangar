//! Ubisoft Connect network layer (unofficial endpoints). Never prints URLs, tickets or sessions.

use super::login::UBI_LAUNCHER_APP_ID;
use super::ubi_demux::DemuxGame;
use super::AuthError;
use serde_json::Value;
use std::time::Duration;

pub const GRAPHQL_URL: &str = "https://public-ubiservices.ubi.com/v1/profiles/me/uplay/graphql";
pub const SESSIONS_URL: &str = "https://public-ubiservices.ubi.com/v3/profiles/sessions";
const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
const TIMEOUT: Duration = Duration::from_secs(20);

pub struct UbiLogin {
    pub ticket: String,
    pub session_id: String,
    pub remember_ticket: String,
    pub user_id: String,
    pub display_name: String,
    /// All secret/personal strings for scrubbing (empty ones are dropped).
    pub secrets: Vec<String>,
}

impl Drop for UbiLogin {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        self.ticket.zeroize();
        self.session_id.zeroize();
        self.remember_ticket.zeroize();
    }
}

pub fn parse_login(code: &str) -> Result<UbiLogin, String> {
    let v: Value = serde_json::from_str(code).map_err(|_| "giriş verisi JSON değil".to_string())?;
    let s = |v: &Value, k: &str| v[k].as_str().unwrap_or("").to_string();
    let login = &v["login"];
    let ticket = s(login, "ticket");
    if ticket.is_empty() {
        return Err("giriş verisinde ticket yok".into());
    }
    let remember_ticket = s(&v["remember"], "rememberMeTicket");
    let mut secrets: Vec<String> = Vec::new();
    for src in [login, &v["remember"]] {
        for k in ["ticket", "sessionId", "userId", "profileId", "rememberMeTicket", "nameOnPlatform", "email", "username", "sessionKey"] {
            let x = s(src, k);
            if !x.is_empty() && !secrets.contains(&x) {
                secrets.push(x);
            }
        }
    }
    let display_name = ["nameOnPlatform", "username"]
        .iter()
        .map(|k| s(login, k))
        .find(|x| !x.is_empty())
        .unwrap_or_else(|| "Ubisoft".to_string());
    Ok(UbiLogin {
        ticket,
        session_id: s(login, "sessionId"),
        remember_ticket,
        user_id: s(login, "userId"),
        display_name,
        secrets,
    })
}

const OWNED_QUERY: &str = "query AllGames {viewer {    id    ...ownedGamesList  }}fragment gameProps on Game {  id  spaceId  name  coverUrl  backgroundUrl  bannerUrl}fragment ownedGameProps on Game {  ...gameProps  viewer {    meta {      id      ownedPlatformGroups {        id        name        type      }    }  }}fragment ownedGamesList on User {  ownedGames: games(filterBy: {isOwned: true}) {    totalCount    nodes {      ...ownedGameProps    }  }}";

pub fn owned_games_body() -> String {
    serde_json::json!({"operationName": "AllGames", "variables": {"owned": true}, "query": OWNED_QUERY}).to_string()
}

pub trait UbiApi: Send + Sync {
    /// `rm_v1` refresh; returns the raw session JSON. 400/401 → `InvalidGrant`.
    fn refresh(&self, remember_ticket: &str) -> Result<String, AuthError>;
    /// AllGames GraphQL; returns the raw JSON.
    fn owned_games(&self, ticket: &str, session_id: &str) -> Result<String, AuthError>;
    /// Owned products over Ubisoft's demux protocol (read-only), unfiltered.
    fn owned_products(&self, ticket: &str) -> Result<Vec<DemuxGame>, AuthError>;
    /// Best-effort server-side logout.
    fn logout(&self, ticket: &str, session_id: &str) -> Result<(), AuthError>;
    /// Debug exploration: posts an arbitrary GraphQL body, returns the raw response (error bodies too).
    #[cfg(debug_assertions)]
    fn graphql(&self, _ticket: &str, _session_id: &str, _body: &str) -> Result<String, AuthError> {
        Err(AuthError::Network("desteklenmiyor".into()))
    }
}

pub struct HttpUbiApi;

// Never Display/Debug the ureq error: it embeds the request URL.
fn net_err(e: &ureq::Error) -> AuthError {
    match e {
        ureq::Error::Status(code, _) => AuthError::Network(format!("durum {code}")),
        ureq::Error::Transport(t) => AuthError::Network(format!("{:?}", t.kind())),
    }
}

fn body(r: ureq::Response) -> Result<String, AuthError> {
    r.into_string().map_err(|_| AuthError::Network("okuma".into()))
}

impl UbiApi for HttpUbiApi {
    fn refresh(&self, remember_ticket: &str) -> Result<String, AuthError> {
        let r = ureq::post(SESSIONS_URL)
            .timeout(TIMEOUT)
            .set("Authorization", &format!("rm_v1 t={remember_ticket}"))
            .set("Ubi-AppId", UBI_LAUNCHER_APP_ID)
            .set("Content-Type", "application/json")
            .set("User-Agent", UA)
            .send_string(r#"{"rememberMe":true}"#);
        match r {
            Ok(r) => body(r),
            Err(ureq::Error::Status(400 | 401, _)) => Err(AuthError::InvalidGrant),
            Err(e) => Err(net_err(&e)),
        }
    }

    fn owned_games(&self, ticket: &str, session_id: &str) -> Result<String, AuthError> {
        let r = ureq::post(GRAPHQL_URL)
            .timeout(TIMEOUT)
            .set("Authorization", &format!("Ubi_v1 t={ticket}"))
            .set("Ubi-AppId", UBI_LAUNCHER_APP_ID)
            .set("Ubi-SessionId", session_id)
            .set("Content-Type", "application/json")
            .set("User-Agent", UA)
            .send_string(&owned_games_body())
            .map_err(|e| net_err(&e))?;
        body(r)
    }

    fn owned_products(&self, ticket: &str) -> Result<Vec<DemuxGame>, AuthError> {
        super::ubi_demux::fetch_products(ticket).map_err(AuthError::Network)
    }

    #[cfg(debug_assertions)]
    fn graphql(&self, ticket: &str, session_id: &str, req: &str) -> Result<String, AuthError> {
        let r = ureq::post(GRAPHQL_URL)
            .timeout(TIMEOUT)
            .set("Authorization", &format!("Ubi_v1 t={ticket}"))
            .set("Ubi-AppId", UBI_LAUNCHER_APP_ID)
            .set("Ubi-SessionId", session_id)
            .set("Content-Type", "application/json")
            .set("User-Agent", UA)
            .send_string(req);
        match r {
            Ok(r) | Err(ureq::Error::Status(_, r)) => body(r),
            Err(e) => Err(net_err(&e)),
        }
    }

    fn logout(&self, ticket: &str, session_id: &str) -> Result<(), AuthError> {
        ureq::delete(SESSIONS_URL)
            .timeout(TIMEOUT)
            .set("Authorization", &format!("Ubi_v1 t={ticket}"))
            .set("Ubi-AppId", UBI_LAUNCHER_APP_ID)
            .set("Ubi-SessionId", session_id)
            .set("User-Agent", UA)
            .call()
            .map(|_| ())
            .map_err(|e| net_err(&e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parse_login_collects_secrets() {
        let l = parse_login(r#"{"login":{"ticket":"T","sessionId":"S","userId":"U","nameOnPlatform":"N"},"remember":{"rememberMeTicket":"R"}}"#).unwrap();
        assert_eq!((l.ticket.as_str(), l.session_id.as_str(), l.remember_ticket.as_str()), ("T", "S", "R"));
        assert_eq!((l.user_id.as_str(), l.display_name.as_str()), ("U", "N"));
        for s in ["T", "S", "U", "N", "R"] {
            assert!(l.secrets.contains(&s.to_string()));
        }
        assert!(parse_login(r#"{"login":{}}"#).is_err());
    }
}
