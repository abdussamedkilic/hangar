use super::AuthError;
use std::time::Duration;

pub const BASIC_AUTH: &str = "MzRhMDJjZjhmNDQxNGUyOWIxNTkyMTg3NmRhMzZmOWE6ZGFhZmJjY2M3Mzc3NDUwMzlkZmZlNTNkOTRmYzc2Y2Y=";
pub const LOGIN_URL: &str = "https://www.epicgames.com/id/login?redirectUrl=https%3A%2F%2Fwww.epicgames.com%2Fid%2Fapi%2Fredirect%3FclientId%3D34a02cf8f4414e29b15921876da36f9a%26responseType%3Dcode";

const TOKEN_URL: &str = "https://account-public-service-prod03.ol.epicgames.com/account/api/oauth/token";
const KILL_URL: &str = "https://account-public-service-prod03.ol.epicgames.com/account/api/oauth/sessions/kill/";
const LIBRARY_URL: &str = "https://library-service.live.use1a.on.epicgames.com/library/api/public/items?includeMetadata=true";
const TIMEOUT: Duration = Duration::from_secs(15);

pub trait EpicApi: Send + Sync {
    fn token(&self, form: &[(&str, &str)]) -> Result<String, AuthError>;
    fn library_page(&self, access: &str, cursor: Option<&str>) -> Result<String, AuthError>;
    fn catalog(&self, access: &str, namespace: &str, ids: &[String]) -> Result<String, AuthError>;
    fn kill(&self, access: &str) -> Result<(), AuthError>;
}

pub struct HttpEpicApi;

// Never Display/Debug the ureq error: it embeds the request URL (the kill URL carries the access token).
fn net_err(e: &ureq::Error) -> AuthError {
    match e {
        ureq::Error::Status(code, _) => AuthError::Network(format!("durum {code}")),
        // kind + static message + underlying io error only; never t.url() or Display of t.
        ureq::Error::Transport(t) => {
            let src = std::error::Error::source(t).map(|s| format!(": {s}")).unwrap_or_default();
            AuthError::Network(format!("{:?}{}{src}", t.kind(), t.message().map(|m| format!(" {m}")).unwrap_or_default()))
        }
    }
}

fn body(r: ureq::Response) -> Result<String, AuthError> {
    r.into_string().map_err(|_| AuthError::Network("okuma".into()))
}

impl EpicApi for HttpEpicApi {
    fn token(&self, form: &[(&str, &str)]) -> Result<String, AuthError> {
        let r = ureq::post(TOKEN_URL)
            .timeout(TIMEOUT)
            .set("Authorization", &format!("basic {BASIC_AUTH}"))
            .send_form(form);
        match r {
            Ok(r) => body(r),
            Err(ureq::Error::Status(400 | 401, _)) => Err(AuthError::InvalidGrant),
            Err(e) => Err(net_err(&e)),
        }
    }

    fn library_page(&self, access: &str, cursor: Option<&str>) -> Result<String, AuthError> {
        let mut req = ureq::get(LIBRARY_URL).timeout(TIMEOUT).set("Authorization", &format!("bearer {access}"));
        if let Some(c) = cursor {
            req = req.query("cursor", c);
        }
        body(req.call().map_err(|e| net_err(&e))?)
    }

    fn catalog(&self, access: &str, namespace: &str, ids: &[String]) -> Result<String, AuthError> {
        let url = format!("https://catalog-public-service-prod06.ol.epicgames.com/catalog/api/shared/namespace/{namespace}/bulk/items");
        let mut req = ureq::get(&url)
            .timeout(TIMEOUT)
            .set("Authorization", &format!("bearer {access}"));
        // Canlı doğrulandı: virgülle birleştirilmiş kimlikler `{}` döner; her kimlik ayrı id parametresi olmalı.
        for id in ids {
            req = req.query("id", id);
        }
        let r = req
            .query("includeDLCDetails", "true")
            .query("includeMainGameDetails", "true")
            .query("country", "US")
            .query("locale", "en-US")
            .call()
            .map_err(|e| net_err(&e))?;
        body(r)
    }

    fn kill(&self, access: &str) -> Result<(), AuthError> {
        ureq::delete(&format!("{KILL_URL}{access}"))
            .timeout(TIMEOUT)
            .set("Authorization", &format!("bearer {access}"))
            .call()
            .map(|_| ())
            .map_err(|e| net_err(&e))
    }
}
