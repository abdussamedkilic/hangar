use serde_json::Value;

const KEYS: &[&str] = &[
    "access_token", "refresh_token", "id_token", "account_id", "accountId", "displayName", "email", "ownerId",
    "authorizationCode", "sid",
];

fn redact(v: &mut Value) {
    match v {
        Value::Object(m) => {
            for (k, val) in m.iter_mut() {
                if KEYS.contains(&k.as_str()) {
                    *val = Value::String("REDACTED".into());
                } else {
                    redact(val);
                }
            }
        }
        Value::Array(a) => a.iter_mut().for_each(redact),
        _ => {}
    }
}

pub fn sanitize(body: &str, secrets: &[&str]) -> Result<String, String> {
    let mut out = match serde_json::from_str::<Value>(body) {
        Ok(mut v) => {
            redact(&mut v);
            v.to_string()
        }
        Err(_) => body.to_string(),
    };
    for s in secrets.iter().filter(|s| !s.is_empty()) {
        out = out.replace(s, "REDACTED");
    }
    if secrets.iter().any(|s| !s.is_empty() && out.contains(s)) {
        return Err("sızıntı".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_keys_nested_and_secrets() {
        let out = sanitize(r#"{"account_id":"acc1","displayName":"Samet","records":[{"appName":"Fig","note":"acc1","x":{"authorizationCode":"C0DE"}}]}"#, &["acc1", "Samet"]).unwrap();
        for bad in ["acc1", "Samet", "C0DE"] {
            assert!(!out.contains(bad));
        }
        assert!(out.contains("Fig"));
    }
    #[test]
    fn non_json_and_empty_secret() {
        assert_eq!(sanitize("token=RT-SECRET", &["RT-SECRET"]).unwrap(), "token=REDACTED");
        assert_eq!(sanitize("abc", &[""]).unwrap(), "abc");
    }
}
