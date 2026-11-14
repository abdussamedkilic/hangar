//! Epic giriş penceresi için saf gezinme kuralları (Tauri'siz).

#[derive(Debug, Clone, PartialEq)]
pub enum NavDecision {
    Callback(String),
    Allow,
    Block,
}

/// Gezinme kuralları: `(host, tam_host, yol_önekleri)`. `tam_host` true ise yalnızca o host,
/// false ise host ve alt alan adları; önek listesi boşsa her yol, doluysa yol bu öneklerden biriyle
/// başlamalı. Epic + captcha (alt alanlarıyla, her yol); Google/Apple/Microsoft giriş hostları
/// (tam host, her yol); Sony hesabı (alt alanlarıyla, her yol); Nintendo (tam host, her yol);
/// Steam yalnızca `/openid/` ve `/login/`; Facebook (www/m/web) yalnızca giriş/yetki/doğrulama yolları ve `/v<sürüm>/dialog/oauth`.
/// Yol önekleri bölüm sınırında eşleşir (`/login` = `/login` veya `/login/...`); açık port her zaman engellenir.
/// Sitenin geri kalanı engellenir. Geri dönüş nonce'la bağlı olduğundan izinli sayfalar kod enjekte edemez.
pub const NAV_RULES: &[(&str, bool, &[&str])] = &[
    ("epicgames.com", false, &[]),
    ("hcaptcha.com", false, &[]),
    ("arkoselabs.com", false, &[]),
    ("funcaptcha.com", false, &[]),
    ("accounts.google.com", true, &[]),
    ("appleid.apple.com", true, &[]),
    ("idmsa.apple.com", true, &[]),
    ("login.live.com", true, &[]),
    ("account.live.com", true, &[]),
    ("login.microsoftonline.com", true, &[]),
    ("account.sony.com", false, &[]),
    ("accounts.nintendo.com", true, &[]),
    ("steamcommunity.com", true, &["/openid/", "/login/"]),
    ("www.facebook.com", true, FB_PATHS),
    ("m.facebook.com", true, FB_PATHS),
    ("web.facebook.com", true, FB_PATHS),
];

const FB_PATHS: &[&str] = &["/login", "/login.php", "/dialog/oauth", "/checkpoint", "/two_step_verification", "/privacy/consent"];

/// `/v<digits>.<digits>/dialog/oauth...` (Facebook's versioned OAuth dialog).
fn fb_versioned_oauth(path: &str) -> bool {
    let Some(rest) = path.strip_prefix("/v") else { return false };
    let Some((ver, tail)) = rest.split_once('/') else { return false };
    let Some((maj, min)) = ver.split_once('.') else { return false };
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    digits(maj) && digits(min) && (tail == "dialog/oauth" || tail.starts_with("dialog/oauth/"))
}

fn is_facebook(host: &str) -> bool {
    matches!(host, "www.facebook.com" | "m.facebook.com" | "web.facebook.com")
}

/// Ubisoft Connect girişi (U0 keşfi): tam konak connect/account.ubisoft.com + captcha; port yok, yalnızca https.
pub const UBI_NAV_RULES: &[(&str, bool, &[&str])] = &[
    ("connect.ubisoft.com", true, &[]),
    ("account.ubisoft.com", true, &[]),
    ("hcaptcha.com", false, &[]),
    ("arkoselabs.com", false, &[]),
    ("funcaptcha.com", false, &[]),
];

/// Ubisoft Connect web uygulaması kimliği (dönebilir; tek yerde dursun).
pub const UBI_APP_ID: &str = "82b650c0-6cb3-40c0-9f41-25a53b62b206";
/// Ubisoft Connect PC launcher app id. The demux service only accepts tickets issued for it, while the
/// web login page only accepts `UBI_APP_ID`. An `rm_v1` refresh with this id converts the web
/// remember-me ticket (and invalidates the web one).
pub const UBI_LAUNCHER_APP_ID: &str = "f68a4bb5-608a-4ff2-8123-be8ef797e0a6";
pub const UBI_GENOME_ID: &str = "42d07c95-9914-4450-8b38-267c4e462b21";

pub fn ubi_login_url() -> String {
    format!(
        "https://connect.ubisoft.com/login?appId={UBI_APP_ID}&genomeId={UBI_GENOME_ID}&lang=en-US&nextUrl=https:%2F%2Fconnect.ubisoft.com%2Fready"
    )
}

fn nav_allowed_with(url: &url::Url, rules: &[(&str, bool, &[&str])]) -> bool {
    let host = url.host_str().unwrap_or("");
    let path = url.path();
    url.scheme() == "https"
        && url.port().is_none()
        && rules.iter().any(|(h, exact, prefixes)| {
            let host_ok = host == *h || (!*exact && host.ends_with(&format!(".{h}")));
            host_ok
                && (prefixes.is_empty()
                    || prefixes.iter().any(|p| {
                        let p = p.trim_end_matches('/');
                        path == p || path.starts_with(&format!("{p}/"))
                    })
                    || (is_facebook(host) && fb_versioned_oauth(path)))
        })
}

/// Her giriş denemesi için bir kez üretilen anti-CSRF etiketi (gizli anahtar değil).
/// Yakalama adresi bu etiketi taşımıyorsa kabul edilmez; böylece izinli bir sayfa
/// kendi kodunu enjekte edemez.
pub fn new_nonce() -> String {
    use std::collections::hash_map::RandomState;
    use std::hash::BuildHasher;
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let c = COUNTER.fetch_add(1, Ordering::Relaxed);
    let hi = RandomState::new().hash_one((t, c));
    let lo = RandomState::new().hash_one((c, t));
    format!("{hi:016x}{lo:016x}")
}

/// `nonce` [0-9a-f]{32} olmalı; aksi halde panikler (JS enjeksiyonunu önler).
pub fn init_script(nonce: &str) -> String {
    assert!(
        nonce.len() == 32 && nonce.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')),
        "invalid nonce"
    );
    format!(
        r#"(function(){{try{{
  if (window.top !== window) return;
  if (location.origin !== 'https://www.epicgames.com' || location.pathname !== '/id/api/redirect') return;
  var N = '{nonce}';
  var run = function(){{ try {{
    var j = JSON.parse(document.body.innerText);
    if (j && j.authorizationCode) location.replace('https://hangar.invalid/callback?n=' + N + '&code=' + encodeURIComponent(j.authorizationCode));
  }} catch(e){{}} }};
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', run); else run();
}}catch(e){{}}}})();"#
    )
}

/// Ubisoft girişi için betik: `connect.ubisoft.com` üst çerçevesinde localStorage'ı yoklar;
/// `PRODloginData` içinde `ticket` görününce JSON'u nonce'lu geri dönüş adresiyle yollar.
pub fn ubi_init_script(nonce: &str) -> String {
    assert!(
        nonce.len() == 32 && nonce.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')),
        "invalid nonce"
    );
    format!(
        r#"(function(){{try{{
  if (window.top !== window) return;
  if (location.origin !== 'https://connect.ubisoft.com') return;
  var N = '{nonce}';
  var n = 0;
  var t = setInterval(function(){{ try {{
    if (++n > 600) {{ clearInterval(t); return; }}
    var raw = localStorage.getItem('PRODloginData');
    if (!raw) return;
    var login = JSON.parse(raw);
    if (!login || !login.ticket) return;
    clearInterval(t);
    var rem = null;
    try {{ rem = JSON.parse(localStorage.getItem('PRODrememberMe')); }} catch(e){{}}
    location.replace('https://hangar.invalid/callback?n=' + N + '&code=' + encodeURIComponent(JSON.stringify({{login: login, remember: rem}})));
  }} catch(e){{}} }}, 500);
}}catch(e){{}}}})();"#
    )
}

pub fn decide(url: &url::Url, nonce: &str) -> NavDecision {
    decide_with(url, nonce, NAV_RULES)
}

pub fn decide_with(url: &url::Url, nonce: &str, rules: &[(&str, bool, &[&str])]) -> NavDecision {
    let host = url.host_str().unwrap_or("");
    if host == "hangar.invalid" {
        if url.scheme() == "https" && url.path() == "/callback" {
            let n_ok = url.query_pairs().any(|(k, v)| k == "n" && v == nonce);
            if n_ok {
                if let Some((_, code)) = url.query_pairs().find(|(k, _)| k == "code") {
                    if !code.is_empty() {
                        return NavDecision::Callback(code.into_owned());
                    }
                }
            }
        }
        return NavDecision::Block;
    }
    if nav_allowed_with(url, rules) {
        return NavDecision::Allow;
    }
    NavDecision::Block
}

pub fn window_title(url: &url::Url) -> String {
    format!("Epic Games girişi — {}", url.host_str().unwrap_or(""))
}

pub fn ubi_window_title(url: &url::Url) -> String {
    format!("Ubisoft Connect girişi — {}", url.host_str().unwrap_or(""))
}

pub fn open_error(uri: &str, err: &str) -> String {
    if uri.starts_with("com.epicgames.launcher://") {
        "Epic Games Launcher açılamadı — kurulu mu?".to_string()
    } else {
        format!("Açılamadı: {err}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const N: &str = "0123456789abcdef0123456789abcdef";
    fn u(s: &str) -> url::Url { url::Url::parse(s).unwrap() }
    #[test] fn callback_extracts_code() { assert!(matches!(decide(&u(&format!("https://hangar.invalid/callback?n={N}&code=abc%2B1")), N), NavDecision::Callback(c) if c == "abc+1")); }
    #[test] fn bad_callbacks_block() {
        for s in [format!("https://hangar.invalid/callback?n={N}&code="), format!("https://hangar.invalid/other?n={N}&code=x"), format!("http://hangar.invalid/callback?n={N}&code=x"), format!("https://hangar.invalid/callback/?n={N}&code=x")] {
            assert!(matches!(decide(&u(&s), N), NavDecision::Block), "{s}");
        }
    }
    #[test] fn wrong_or_missing_nonce_blocks() {
        for s in ["https://hangar.invalid/callback?code=x".to_string(), "https://hangar.invalid/callback?n=other&code=x".to_string()] {
            assert!(matches!(decide(&u(&s), N), NavDecision::Block), "{s}");
        }
    }
    #[test] fn allowlist() {
        for ok in ["https://www.epicgames.com/id/login", "https://newassets.hcaptcha.com/x", "https://epicgames.com/", "https://WWW.EPICGAMES.COM/"] {
            assert!(matches!(decide(&u(ok), N), NavDecision::Allow), "{ok}");
        }
        for ok in ["https://accounts.google.com/o/oauth2", "https://appleid.apple.com/auth", "https://login.live.com/oauth20", "https://my.account.sony.com/x", "https://steamcommunity.com/openid/login", "https://accounts.nintendo.com/x", "https://www.facebook.com/dialog/oauth", "https://www.facebook.com/login/?x=1", "https://www.facebook.com/login.php", "https://www.facebook.com/v19.0/dialog/oauth", "https://web.facebook.com/login", "https://steamcommunity.com/login/home", "https://login.microsoftonline.com/common"] {
            assert!(matches!(decide(&u(ok), N), NavDecision::Allow), "{ok}");
        }
        for bad in ["https://sites.google.com/x", "https://www.google.com/", "https://apple.com/", "https://evil.facebook.com/", "http://www.epicgames.com/","https://evil-epicgames.com/", "https://example.com/", "https://epicgames.com.evil.io/", "https://epicgames.com./", "https://epicgames.com@evil.io/", "https://steamcommunity.com/id/someone", "https://www.facebook.com/somepage", "https://evil.steamcommunity.com/openid/login", "https://evil.accounts.google.com/", "https://www.facebook.com/loginx", "https://www.facebook.com/video", "https://www.facebook.com:8443/login", "https://www.facebook.com/v19/dialog/oauth", "https://www.facebook.com/v19.0/dialog/oauthx", "https://steamcommunity.com/openidx", "https://www.epicgames.com:8443/id/login", "https://sub.accounts.nintendo.com/x"] {
            assert!(matches!(decide(&u(bad), N), NavDecision::Block), "{bad}");
        }
    }
    #[test] fn init_script_embeds_nonce() {
        let s = init_script(N);
        for p in [N, "window.top", "https://www.epicgames.com", "/id/api/redirect", "DOMContentLoaded", "https://hangar.invalid/callback?n="] { assert!(s.contains(p), "{p}"); }
    }
    #[test] #[should_panic] fn init_script_rejects_bad_nonce() { init_script("a';alert(1);'"); }
    #[test] fn new_nonce_shape() {
        let (a, b) = (new_nonce(), new_nonce());
        assert_eq!(a.len(), 32);
        assert!(a.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')));
        assert_ne!(a, b);
    }
    #[test] fn ubi_allowlist() {
        let d = |s: &str| decide_with(&u(s), N, UBI_NAV_RULES);
        for ok in ["https://connect.ubisoft.com/login?x=1", "https://account.ubisoft.com/en-US/login", "https://newassets.hcaptcha.com/x", "https://client-api.arkoselabs.com/"] {
            assert!(matches!(d(ok), NavDecision::Allow), "{ok}");
        }
        for bad in ["http://connect.ubisoft.com/", "https://connect.ubisoft.com:8443/", "https://evil-ubisoft.com/", "https://forum.ubisoft.com/", "https://store.ubisoft.com/", "https://static.ubisoft.com/a", "https://public-ubiservices.ubi.com/v3", "https://ubisoft.com.evil.io/", "https://www.epicgames.com/", "https://example.com/"] {
            assert!(matches!(d(bad), NavDecision::Block), "{bad}");
        }
        assert!(matches!(decide(&u("https://connect.ubisoft.com/"), N), NavDecision::Block));
    }
    #[test] fn ubi_callback_carries_json() {
        let s = format!("https://hangar.invalid/callback?n={N}&code=%7B%22login%22%3A%7B%22ticket%22%3A%22t%22%7D%7D");
        assert!(matches!(decide_with(&u(&s), N, UBI_NAV_RULES), NavDecision::Callback(c) if c == r#"{"login":{"ticket":"t"}}"#));
    }
    #[test] fn ubi_init_script_scoped() {
        let s = ubi_init_script(N);
        for p in [N, "window.top", "https://connect.ubisoft.com", "PRODloginData", "PRODrememberMe", "500", "https://hangar.invalid/callback?n="] { assert!(s.contains(p), "{p}"); }
        assert!(!s.contains("epicgames"));
    }
    #[test] #[should_panic] fn ubi_init_script_rejects_bad_nonce() { ubi_init_script("a';alert(1);'"); }
    #[test] fn ubi_login_url_has_app_id() { assert!(ubi_login_url().contains(UBI_APP_ID)); }
    #[test] fn title_shows_host() { assert_eq!(window_title(&u("https://www.epicgames.com/id/login")), "Epic Games girişi — www.epicgames.com"); }
    #[test] fn open_error_texts() {
        assert_eq!(open_error("com.epicgames.launcher://apps/x?action=install", "e"), "Epic Games Launcher açılamadı — kurulu mu?");
        assert_eq!(open_error("steam://run/1", "x"), "Açılamadı: x");
    }
}
