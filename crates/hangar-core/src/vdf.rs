use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum Vdf {
    Str(String),
    Obj(Vec<(String, Vdf)>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct VdfError(pub String);

impl fmt::Display for VdfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "vdf parse error: {}", self.0)
    }
}

impl std::error::Error for VdfError {}

impl Vdf {
    /// Case-insensitive child lookup (first match); `None` for non-objects.
    pub fn get(&self, key: &str) -> Option<&Vdf> {
        match self {
            Vdf::Obj(c) => c.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v),
            Vdf::Str(_) => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Vdf::Str(s) => Some(s),
            Vdf::Obj(_) => None,
        }
    }
}

enum Tok {
    Str(String),
    Open,
    Close,
}

fn tokenize(src: &str) -> Result<Vec<Tok>, VdfError> {
    let mut out = Vec::new();
    let mut it = src.chars().peekable();
    while let Some(&c) = it.peek() {
        match c {
            c if c.is_whitespace() => {
                it.next();
            }
            '{' => {
                it.next();
                out.push(Tok::Open);
            }
            '}' => {
                it.next();
                out.push(Tok::Close);
            }
            '#' => {
                while it.next_if(|&c| c != '\n').is_some() {}
            }
            '/' => {
                it.next();
                if it.next() != Some('/') {
                    return Err(VdfError("stray '/'".into()));
                }
                while it.next_if(|&c| c != '\n').is_some() {}
            }
            '"' => {
                it.next();
                let mut s = String::new();
                loop {
                    match it.next() {
                        None => return Err(VdfError("unterminated string".into())),
                        Some('"') => break,
                        Some('\\') => match it.next() {
                            Some('n') => s.push('\n'),
                            Some('t') => s.push('\t'),
                            Some(e @ ('\\' | '"')) => s.push(e),
                            Some(e) => {
                                s.push('\\');
                                s.push(e);
                            }
                            None => return Err(VdfError("unterminated string".into())),
                        },
                        Some(ch) => s.push(ch),
                    }
                }
                out.push(Tok::Str(s));
            }
            _ => {
                let mut s = String::new();
                while let Some(ch) = it.next_if(|&c| !c.is_whitespace() && c != '{' && c != '}') {
                    s.push(ch);
                }
                out.push(Tok::Str(s));
            }
        }
    }
    Ok(out)
}

fn parse_obj(toks: &mut std::vec::IntoIter<Tok>, nested: bool) -> Result<Vdf, VdfError> {
    let mut items = Vec::new();
    loop {
        let key = match toks.next() {
            None if !nested => return Ok(Vdf::Obj(items)),
            None => return Err(VdfError("missing '}'".into())),
            Some(Tok::Close) if nested => return Ok(Vdf::Obj(items)),
            Some(Tok::Close) => return Err(VdfError("unexpected '}'".into())),
            Some(Tok::Open) => return Err(VdfError("unexpected '{'".into())),
            Some(Tok::Str(k)) => k,
        };
        let val = match toks.next() {
            Some(Tok::Str(v)) => Vdf::Str(v),
            Some(Tok::Open) => parse_obj(toks, true)?,
            _ => return Err(VdfError(format!("missing value for key '{key}'"))),
        };
        items.push((key, val));
    }
}

/// Parses VDF text into a root `Vdf::Obj` holding the top-level pairs.
pub fn parse(src: &str) -> Result<Vdf, VdfError> {
    parse_obj(&mut tokenize(src)?.into_iter(), false)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/steam/");

    #[test]
    fn parses_nested_objects() {
        let v = parse(r#""a" { "b" "1" "c" { "d" "x y" } }"#).unwrap();
        let d = v.get("a").unwrap().get("c").unwrap().get("d").unwrap();
        assert_eq!(d.as_str(), Some("x y"));
    }

    #[test]
    fn handles_escapes_and_comments() {
        let v = parse("// yorum\n\"k\" \"C:\\\\Games\\\\Ş\"").unwrap();
        assert_eq!(v.get("k").unwrap().as_str(), Some("C:\\Games\\Ş"));
    }

    #[test]
    fn get_is_case_insensitive_and_bare_tokens_work() {
        let v = parse("#base \"x.vdf\"\nKey { Sub bare }").unwrap();
        assert_eq!(v.get("key").unwrap().get("SUB").unwrap().as_str(), Some("bare"));
    }

    #[test]
    fn rejects_unbalanced() {
        assert!(parse(r#""a" { "b" "1""#).is_err());
    }

    #[test]
    fn parses_fixture_libraryfolders() {
        let src = std::fs::read_to_string(format!("{DIR}synthetic-libraryfolders.vdf")).unwrap();
        let v = parse(&src).unwrap();
        match v.get("libraryfolders").unwrap() {
            Vdf::Obj(c) => assert!(!c.is_empty()),
            _ => panic!("not an object"),
        }
    }

    #[test]
    fn parses_utf8_appmanifest() {
        let src = std::fs::read_to_string(format!("{DIR}synthetic-appmanifest_utf8.acf")).unwrap();
        let v = parse(&src).unwrap();
        assert_eq!(v.get("AppState").unwrap().get("name").unwrap().as_str(), Some("Şeytan İşi"));
    }
}
