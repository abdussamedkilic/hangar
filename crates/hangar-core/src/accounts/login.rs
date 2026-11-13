//! Epic giriş penceresi için saf gezinme kuralları (Tauri'siz).

#[derive(Debug, Clone, PartialEq)]
pub enum NavDecision {
    Callback(String),
    Allow,
    Block,
}

/// Üçüncü taraf girişi (Google/Apple/…) desteklenmez; Epic e-postasıyla giriş yapılır.
pub const ALLOWED_HOSTS: &[&str] = &["epicgames.com", "hcaptcha.com", "arkoselabs.com", "funcaptcha.com"];

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

pub fn decide(url: &url::Url, nonce: &str) -> NavDecision {
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
    if url.scheme() == "https" && ALLOWED_HOSTS.iter().any(|h| host == *h || host.ends_with(&format!(".{h}"))) {
        return NavDecision::Allow;
    }
    NavDecision::Block
}

pub fn window_title(url: &url::Url) -> String {
    format!("Epic Games girişi — {}", url.host_str().unwrap_or(""))
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
        for bad in ["https://accounts.google.com/o", "http://www.epicgames.com/","https://evil-epicgames.com/", "https://example.com/", "https://epicgames.com.evil.io/", "https://epicgames.com./", "https://epicgames.com@evil.io/"] {
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
    #[test] fn title_shows_host() { assert_eq!(window_title(&u("https://www.epicgames.com/id/login")), "Epic Games girişi — www.epicgames.com"); }
    #[test] fn open_error_texts() {
        assert_eq!(open_error("com.epicgames.launcher://apps/x?action=install", "e"), "Epic Games Launcher açılamadı — kurulu mu?");
        assert_eq!(open_error("steam://run/1", "x"), "Açılamadı: x");
    }
}
