use super::AuthError;
use std::time::Duration;

pub const CLIENT_ID: &str = "46899977096215655";
// Galaxy istemcisinin herkese açık gizli anahtarı (Heroic/Playnite/lgogdownloader aynısını kullanır).
// GOG belirteç ucu bunu sorgu dizesinde ister.
const CLIENT_SECRET: &str = "9d85c43b1482497dbbce61f6e4aa173a433796eeae2ca8c5f6129f2dc4de46d9";
pub const REDIRECT: &str = "https://embed.gog.com/on_login_success?origin=client";
const TOKEN_URL: &str = "https://auth.gog.com/token";
const FILTERED_URL: &str = "https://embed.gog.com/account/getFilteredProducts";
const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
const TIMEOUT: Duration = Duration::from_secs(20);

pub trait GogApi: Send + Sync {
    /// Authorization code -> token response body.
    fn exchange(&self, code: &str) -> Result<String, AuthError>;
    /// Refresh token -> token response body.
    fn refresh(&self, rt: &str) -> Result<String, AuthError>;
    /// getFilteredProducts page (1-based) body.
    fn filtered_page(&self, token: &str, page: u32) -> Result<String, AuthError>;
}

pub struct HttpGogApi;

// Never Display/Debug the ureq error: it embeds the request URL (the token URL carries the code/secret).
fn net_err(e: &ureq::Error) -> AuthError {
    match e {
        ureq::Error::Status(code, _) => AuthError::Network(format!("durum {code}")),
        ureq::Error::Transport(t) => {
            let src = std::error::Error::source(t).map(|s| format!(": {s}")).unwrap_or_default();
            AuthError::Network(format!("{:?}{}{src}", t.kind(), t.message().map(|m| format!(" {m}")).unwrap_or_default()))
        }
    }
}

fn body(r: ureq::Response) -> Result<String, AuthError> {
    r.into_string().map_err(|_| AuthError::Network("okuma".into()))
}

fn token_call(grant: &[(&str, &str)]) -> Result<String, AuthError> {
    let mut req = ureq::get(TOKEN_URL)
        .timeout(TIMEOUT)
        .set("User-Agent", UA)
        .query("client_id", CLIENT_ID)
        .query("client_secret", CLIENT_SECRET);
    for (k, v) in grant {
        req = req.query(k, v);
    }
    match req.call() {
        Ok(r) => body(r),
        Err(ureq::Error::Status(400 | 401, _)) => Err(AuthError::InvalidGrant),
        Err(e) => Err(net_err(&e)),
    }
}

impl GogApi for HttpGogApi {
    fn exchange(&self, code: &str) -> Result<String, AuthError> {
        token_call(&[("grant_type", "authorization_code"), ("code", code), ("redirect_uri", REDIRECT)])
    }

    fn refresh(&self, rt: &str) -> Result<String, AuthError> {
        token_call(&[("grant_type", "refresh_token"), ("refresh_token", rt)])
    }

    fn filtered_page(&self, token: &str, page: u32) -> Result<String, AuthError> {
        let r = ureq::get(FILTERED_URL)
            .timeout(TIMEOUT)
            .set("User-Agent", UA)
            .set("Authorization", &format!("Bearer {token}"))
            .query("mediaType", "1")
            .query("page", &page.to_string())
            .call()
            .map_err(|e| net_err(&e))?;
        body(r)
    }
}
