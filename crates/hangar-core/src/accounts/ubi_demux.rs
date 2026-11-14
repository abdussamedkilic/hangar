//! Reads the owned/claimed games list over Ubisoft's demux protocol.
//! Protocol and field numbers come from YoobieRE/ubisoft-demux-node (proto/proto_demux/demux.proto,
//! proto/proto_ownership/ownership.proto, src/constants.ts). READ-ONLY: only the messages below are sent.
//! Never prints, logs or writes the ticket.

use rustls::pki_types::ServerName;
use rustls::{ClientConfig, ClientConnection, RootCertStore, StreamOwned};
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::time::{Duration, Instant};
use zeroize::Zeroizing;

pub const API_VERSION: u32 = 13416; // build of the installed upc.exe (175.1.13416); 10931 from demux-node is rejected
const HOST: &str = "dmx.upc.ubisoft.com";
const TIMEOUT: Duration = Duration::from_secs(15);
const TOTAL: Duration = Duration::from_secs(45);
const MAX_FRAME: usize = 64 * 1024 * 1024;

// ---------- minimal protobuf ----------

pub fn put_varint(buf: &mut Vec<u8>, mut v: u64) {
    while v >= 0x80 {
        buf.push((v as u8 & 0x7f) | 0x80);
        v >>= 7;
    }
    buf.push(v as u8);
}

pub fn get_varint(b: &[u8], pos: &mut usize) -> Option<u64> {
    let (mut v, mut shift) = (0u64, 0u32);
    loop {
        let byte = *b.get(*pos)?;
        *pos += 1;
        if shift >= 64 {
            return None;
        }
        v |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Some(v);
        }
        shift += 7;
    }
}

pub fn put_uint(buf: &mut Vec<u8>, field: u32, v: u64) {
    put_varint(buf, u64::from(field) << 3);
    put_varint(buf, v);
}

pub fn put_bytes(buf: &mut Vec<u8>, field: u32, data: &[u8]) {
    put_varint(buf, u64::from(field) << 3 | 2);
    put_varint(buf, data.len() as u64);
    buf.extend_from_slice(data);
}

/// 4-byte big-endian length prefix (socket framing and inner connection data both use it).
pub fn frame(payload: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(payload.len() + 4);
    v.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    v.extend_from_slice(payload);
    v
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Val<'a> {
    Varint(u64),
    Bytes(&'a [u8]),
    Other,
}

/// All fields of one message; unknown wire types are skipped (fixed32/64) or the parse fails (groups).
pub fn fields(b: &[u8]) -> Option<Vec<(u32, Val<'_>)>> {
    let (mut pos, mut out) = (0usize, Vec::new());
    while pos < b.len() {
        let key = get_varint(b, &mut pos)?;
        let num = (key >> 3) as u32;
        match key & 7 {
            0 => out.push((num, Val::Varint(get_varint(b, &mut pos)?))),
            1 => {
                pos = pos.checked_add(8).filter(|p| *p <= b.len())?;
                out.push((num, Val::Other));
            }
            2 => {
                let n = usize::try_from(get_varint(b, &mut pos)?).ok()?;
                let end = pos.checked_add(n).filter(|e| *e <= b.len())?;
                out.push((num, Val::Bytes(&b[pos..end])));
                pos = end;
            }
            5 => {
                pos = pos.checked_add(4).filter(|p| *p <= b.len())?;
                out.push((num, Val::Other));
            }
            _ => return None,
        }
    }
    Some(out)
}

fn sub<'a>(b: &'a [u8], num: u32) -> Option<&'a [u8]> {
    fields(b)?.into_iter().find_map(|(n, v)| match v {
        Val::Bytes(x) if n == num => Some(x),
        _ => None,
    })
}
fn subs<'a>(b: &'a [u8], num: u32) -> Vec<&'a [u8]> {
    fields(b)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(n, v)| match v {
            Val::Bytes(x) if n == num => Some(x),
            _ => None,
        })
        .collect()
}
fn num(b: &[u8], n: u32) -> Option<u64> {
    fields(b)?.into_iter().find_map(|(k, v)| match v {
        Val::Varint(x) if k == n => Some(x),
        _ => None,
    })
}

// ---------- messages (demux.proto / ownership.proto) ----------

/// Upstream{ push(2){ client_version(4){ version(1) } } }
pub fn msg_client_version(version: u32) -> Vec<u8> {
    let mut cv = Vec::new();
    put_uint(&mut cv, 1, u64::from(version));
    let mut push = Vec::new();
    put_bytes(&mut push, 4, &cv);
    let mut up = Vec::new();
    put_bytes(&mut up, 2, &push);
    up
}

/// Upstream{ request(1){ request_id(1), <field>(inner) } }
fn request(id: u32, field: u32, inner: &[u8]) -> Vec<u8> {
    let mut r = Vec::new();
    put_uint(&mut r, 1, u64::from(id));
    put_bytes(&mut r, field, inner);
    let mut up = Vec::new();
    put_bytes(&mut up, 1, &r);
    up
}

/// AuthenticateReq{ token(1){ ubi_ticket(3) }, client_id(2) } as Req.authenticate_req(2).
pub fn msg_authenticate(id: u32, ticket: &str) -> Zeroizing<Vec<u8>> {
    let mut token = Zeroizing::new(Vec::new());
    put_bytes(&mut token, 3, ticket.as_bytes());
    let mut auth = Zeroizing::new(Vec::new());
    put_bytes(&mut auth, 1, &token);
    put_bytes(&mut auth, 2, b"uplay_pc");
    Zeroizing::new(request(id, 2, &auth))
}

/// OpenConnectionReq{ service_name(2) } as Req.open_connection_req(3).
pub fn msg_open(id: u32, service: &str) -> Vec<u8> {
    let mut o = Vec::new();
    put_bytes(&mut o, 2, service.as_bytes());
    request(id, 3, &o)
}

/// demux Upstream{ push(2){ data(1){ connection_id(1), data(2) } } } carrying the ownership
/// Upstream{ request(1){ request_id(1), initialize_req(2){ get_associations(2)=true, proto_version(3)=7 } } },
/// which is length-prefixed inside `data`.
pub fn msg_initialize(connection_id: u32, id: u32) -> Vec<u8> {
    let mut init = Vec::new();
    put_uint(&mut init, 2, 1);
    put_uint(&mut init, 3, 7);
    let inner = frame(&request(id, 2, &init));
    let mut dm = Vec::new();
    put_uint(&mut dm, 1, u64::from(connection_id));
    put_bytes(&mut dm, 2, &inner);
    let mut push = Vec::new();
    put_bytes(&mut push, 1, &dm);
    let mut up = Vec::new();
    put_bytes(&mut up, 2, &push);
    up
}

/// One entry of the ownership list, already decoded.
#[derive(Debug, Clone, PartialEq)]
pub struct DemuxGame {
    pub product_id: u32,
    pub owned: bool,
    pub product_type: u32, // 0 Game, 1 AddOn, 2 PreOrderGame, 6 Bundle, 7 SeasonPass ...
    pub space_id: Option<String>,
    pub name: Option<String>,
    pub thumb_image: Option<String>,
}

// ---------- configuration YAML (Uplay product configuration) ----------

fn unquote(s: &str) -> String {
    let t = s.trim();
    for q in ['"', '\''] {
        if t.len() >= 2 && t.starts_with(q) && t.ends_with(q) {
            return t[1..t.len() - 1].to_string();
        }
    }
    t.to_string()
}

fn indent_of(line: &str) -> usize {
    line.len() - line.trim_start_matches(' ').len()
}

fn split_kv(line: &str) -> Option<(&str, String)> {
    let (k, v) = line.trim().split_once(':')?;
    Some((k.trim(), unquote(v)))
}

fn is_blank(line: &str) -> bool {
    let t = line.trim();
    t.is_empty() || t.starts_with('#')
}

/// Direct children (`key: value`) of the top-level block `block:`.
fn block_children<'a>(lines: &[&'a str], block: &str) -> Vec<(&'a str, String)> {
    let head = format!("{block}:");
    let Some(start) = lines.iter().position(|l| indent_of(l) == 0 && l.trim_end() == head) else {
        return Vec::new();
    };
    let mut child_indent = None;
    let mut out = Vec::new();
    for l in &lines[start + 1..] {
        if is_blank(l) {
            continue;
        }
        let ind = indent_of(l);
        if ind == 0 {
            break;
        }
        let ci = *child_indent.get_or_insert(ind);
        if ind == ci {
            if let Some(kv) = split_kv(l) {
                out.push(kv);
            }
        }
    }
    out
}

/// Entries of `localizations:` -> `default:`.
fn default_localizations(lines: &[&str]) -> Vec<(String, String)> {
    let Some(start) = lines.iter().position(|l| indent_of(l) == 0 && l.trim_end() == "localizations:") else {
        return Vec::new();
    };
    let mut def_indent = None;
    let mut entry_indent = None;
    let mut in_default = false;
    let mut out = Vec::new();
    for l in &lines[start + 1..] {
        if is_blank(l) {
            continue;
        }
        let ind = indent_of(l);
        if ind == 0 {
            break;
        }
        let di = *def_indent.get_or_insert(ind);
        if ind == di {
            in_default = l.trim() == "default:";
        } else if in_default && ind > di {
            let ei = *entry_indent.get_or_insert(ind);
            if ind == ei {
                if let Some((k, v)) = split_kv(l) {
                    out.push((k.to_string(), v));
                }
            }
        }
    }
    out
}

fn is_placeholder(v: &str) -> bool {
    let caps = v.chars().any(|c| c.is_ascii_uppercase())
        && v.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
    let lkey = v.len() > 1 && v.starts_with('l') && v[1..].chars().all(|c| c.is_ascii_digit());
    caps || lkey
}

/// Reads `name` and `thumb_image` that are direct children of `root:` in an Uplay product
/// configuration, resolving localization keys. Empty values and unresolvable placeholders give None.
pub fn parse_config(cfg: &str) -> (Option<String>, Option<String>) {
    let lines: Vec<&str> = cfg.lines().collect();
    let root = block_children(&lines, "root");
    let loc = default_localizations(&lines);
    let resolve = |key: &str| -> Option<String> {
        let (_, v) = root.iter().find(|(k, _)| *k == key)?;
        if v.is_empty() {
            return None;
        }
        let r = match loc.iter().find(|(k, _)| k == v) {
            Some((_, lv)) => lv.clone(),
            None if is_placeholder(v) => return None,
            None => v.clone(),
        };
        (!r.is_empty()).then_some(r)
    };
    (resolve("name"), resolve("thumb_image"))
}

// ---------- decoding ----------

fn parse_games(list: &[u8]) -> Vec<DemuxGame> {
    subs(list, 1)
        .into_iter()
        .map(|g| {
            let s = |n| sub(g, n).map(|b| String::from_utf8_lossy(b).into_owned());
            let (name, thumb_image) = s(17).map_or((None, None), |c| parse_config(&c));
            DemuxGame {
                product_id: num(g, 1).and_then(|v| u32::try_from(v).ok()).unwrap_or(0),
                owned: num(g, 20).is_some_and(|v| v != 0),
                product_type: num(g, 8).and_then(|v| u32::try_from(v).ok()).unwrap_or(0),
                space_id: s(30).filter(|v| !v.is_empty()),
                name,
                thumb_image,
            }
        })
        .collect()
}

/// InitializeRsp: success(1), owned_games(2), claimed_games(7), each an OwnedGames{ owned_games(1)* }.
pub fn parse_initialize_rsp(rsp: &[u8]) -> (bool, Vec<DemuxGame>, Vec<DemuxGame>) {
    let ok = num(rsp, 1).is_some_and(|v| v != 0);
    let owned = sub(rsp, 2).map(parse_games).unwrap_or_default();
    let claimed = sub(rsp, 7).map(parse_games).unwrap_or_default();
    (ok, owned, claimed)
}

#[cfg(debug_assertions)]
pub fn summarize(games: &[DemuxGame]) -> String {
    let owned = games.iter().filter(|g| g.owned).count();
    let mut s = format!("kayıt: {}, sahip olunan: {owned}\n", games.len());
    for g in games.iter().filter(|g| g.owned && g.product_type == 0) {
        s.push_str(&format!(
            "{} | {} | {} | {}\n",
            g.product_id,
            g.space_id.as_deref().unwrap_or("-"),
            g.name.as_deref().unwrap_or("-"),
            g.thumb_image.as_deref().unwrap_or("-")
        ));
    }
    s
}

// ---------- network ----------

/// Only the io error kind is kept; never message text.
fn neterr(e: std::io::Error) -> String {
    format!("ağ hatası ({:?})", e.kind())
}

struct Conn {
    tls: StreamOwned<ClientConnection, TcpStream>,
    deadline: Instant,
}

impl Conn {
    fn send(&mut self, payload: &[u8]) -> Result<(), String> {
        let f = Zeroizing::new(frame(payload)); // may contain the ticket
        self.tls.write_all(&f).and_then(|_| self.tls.flush()).map_err(neterr)
    }

    fn read_frame(&mut self) -> Result<Vec<u8>, String> {
        if Instant::now() > self.deadline {
            return Err("zaman aşımı".into());
        }
        let mut len = [0u8; 4];
        self.tls.read_exact(&mut len).map_err(neterr)?;
        let n = u32::from_be_bytes(len) as usize;
        if n > MAX_FRAME {
            return Err("yanıt anlaşılamadı (boyut)".into());
        }
        let mut body = vec![0u8; n];
        self.tls.read_exact(&mut body).map_err(neterr)?;
        Ok(body)
    }

    /// Reads downstream frames, ignoring pushes, until a response with `id`; returns the Rsp bytes.
    fn response(&mut self, id: u32) -> Result<Vec<u8>, String> {
        loop {
            let f = self.read_frame()?;
            if let Some(rsp) = sub(&f, 1) {
                if num(rsp, 1) == Some(u64::from(id)) {
                    return Ok(rsp.to_vec());
                }
            }
        }
    }

    /// Reads connection data pushes for `conn_id`, reassembling the length-prefixed inner message,
    /// until an ownership response with `id`; returns the ownership Rsp bytes.
    fn inner_response(&mut self, conn_id: u32, id: u32) -> Result<Vec<u8>, String> {
        let mut acc: Vec<u8> = Vec::new();
        loop {
            let f = self.read_frame()?;
            let Some(dm) = sub(&f, 2).and_then(|p| sub(p, 1)) else { continue };
            if num(dm, 1) != Some(u64::from(conn_id)) {
                continue;
            }
            acc.extend_from_slice(sub(dm, 2).unwrap_or_default());
            while acc.len() >= 4 {
                let n = u32::from_be_bytes([acc[0], acc[1], acc[2], acc[3]]) as usize;
                if n > MAX_FRAME {
                    return Err("yanıt anlaşılamadı (boyut)".into());
                }
                if acc.len() < 4 + n {
                    break;
                }
                let msg: Vec<u8> = acc.drain(..4 + n).skip(4).collect();
                if let Some(rsp) = sub(&msg, 1) {
                    if num(rsp, 1) == Some(u64::from(id)) {
                        return Ok(rsp.to_vec());
                    }
                }
            }
        }
    }
}

fn connect_once() -> Result<Conn, String> {
    let mut roots = RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    // The demux server drops the handshake for a default client hello; it accepts TLS 1.2 with
    // ECDHE-ECDSA-AES256-GCM-SHA384 (what it negotiates with openssl), so offer only that.
    let provider = rustls::crypto::CryptoProvider {
        cipher_suites: vec![rustls::crypto::ring::cipher_suite::TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384],
        ..rustls::crypto::ring::default_provider()
    };
    let cfg = ClientConfig::builder_with_provider(Arc::new(provider))
        .with_protocol_versions(&[&rustls::version::TLS12])
        .map_err(|_| "TLS ayarlanamadı".to_string())?
        .with_root_certificates(roots)
        .with_no_client_auth();
    let name = ServerName::try_from(HOST).map_err(|_| "TLS ayarlanamadı".to_string())?;
    let conn = ClientConnection::new(Arc::new(cfg), name).map_err(|_| "TLS ayarlanamadı".to_string())?;
    let addr = (HOST, 443)
        .to_socket_addrs()
        .map_err(neterr)?
        .next()
        .ok_or("ağ hatası (adres bulunamadı)")?;
    let tcp = TcpStream::connect_timeout(&addr, TIMEOUT).map_err(neterr)?;
    tcp.set_read_timeout(Some(TIMEOUT)).map_err(neterr)?;
    tcp.set_write_timeout(Some(TIMEOUT)).map_err(neterr)?;
    let mut tls = StreamOwned::new(conn, tcp);
    while tls.conn.is_handshaking() {
        tls.conn
            .complete_io(&mut tls.sock)
            .map_err(|e| format!("TLS el sıkışması ({:?})", e.kind()))?;
    }
    Ok(Conn { tls, deadline: Instant::now() + TOTAL })
}

/// The demux edge often drops the TLS handshake; retry only that step (nothing has been sent yet).
fn connect() -> Result<Conn, String> {
    let mut last = String::new();
    for attempt in 0..6 {
        if attempt > 0 {
            std::thread::sleep(Duration::from_millis(400));
        }
        match connect_once() {
            Ok(c) => return Ok(c),
            Err(e) => last = e,
        }
    }
    Err(last)
}

/// One read-only connection: version -> authenticate -> open ownership_service -> initialize -> close.
/// Returns every entry of owned_games and claimed_games, unfiltered.
pub fn fetch_products(ticket: &str) -> Result<Vec<DemuxGame>, String> {
    let mut c = connect()?;
    c.send(&msg_client_version(API_VERSION))?;

    c.send(&msg_authenticate(1, ticket))?;
    let rsp = c.response(1)?;
    let auth = sub(&rsp, 2).ok_or("Ubisoft demux yanıtı anlaşılamadı")?;
    if num(auth, 1) != Some(1) {
        return Err("Ubisoft demux girişi reddedildi".into());
    }

    c.send(&msg_open(2, "ownership_service"))?;
    let rsp = c.response(2)?;
    let open = sub(&rsp, 3).ok_or("Ubisoft demux yanıtı anlaşılamadı")?;
    if num(open, 2) != Some(1) {
        return Err("Ubisoft sahiplik servisi açılamadı".into());
    }
    let conn_id = num(open, 1)
        .and_then(|v| u32::try_from(v).ok())
        .ok_or("Ubisoft demux yanıtı anlaşılamadı")?;

    c.send(&msg_initialize(conn_id, 1))?;
    let rsp = c.inner_response(conn_id, 1)?;
    let init = sub(&rsp, 2).ok_or("Ubisoft demux yanıtı anlaşılamadı")?;
    let (ok, mut owned, claimed) = parse_initialize_rsp(init);
    drop(c); // closes the connection
    if !ok {
        return Err("Ubisoft sahiplik listesi alınamadı".into());
    }
    owned.extend(claimed);
    Ok(owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Live, no secret: a bogus ticket must reach the auth response (proves TLS + framing).
    #[test]
    #[ignore]
    fn live_demux_bogus_ticket() {
        let r = fetch_products("bogus");
        eprintln!("{r:?}");
        assert_eq!(r, Err("Ubisoft demux girişi reddedildi".to_string()));
    }

    #[test]
    fn varint_roundtrip() {
        let mut b = Vec::new();
        put_varint(&mut b, 300);
        assert_eq!(b, vec![0xac, 0x02]);
        let mut p = 0;
        assert_eq!(get_varint(&b, &mut p), Some(300));
        assert_eq!(p, 2);
        assert_eq!(get_varint(&[0x80], &mut 0), None);
    }

    #[test]
    fn frame_and_messages() {
        assert_eq!(frame(&[1, 2, 3]), vec![0, 0, 0, 3, 1, 2, 3]);
        // Upstream{push{client_version{version=10931}}}
        let m = msg_client_version(10931);
        let push = sub(&m, 2).unwrap();
        let cv = sub(push, 4).unwrap();
        assert_eq!(num(cv, 1), Some(10931));
        // initialize: inner data is length-prefixed ownership request
        let m = msg_initialize(7, 1);
        let dm = sub(sub(&m, 2).unwrap(), 1).unwrap();
        assert_eq!(num(dm, 1), Some(7));
        let inner = sub(dm, 2).unwrap();
        let n = u32::from_be_bytes([inner[0], inner[1], inner[2], inner[3]]) as usize;
        assert_eq!(n, inner.len() - 4);
        let req = sub(&inner[4..], 1).unwrap();
        let init = sub(req, 2).unwrap();
        assert_eq!((num(req, 1), num(init, 2), num(init, 3)), (Some(1), Some(1), Some(7)));
    }

    #[test]
    fn decode_initialize_rsp_skips_unknown() {
        let cfg = "root:\n  name: l123\n  thumb_image: abc.jpg\nlocalizations:\n  default:\n    l123: \"Assassin's Creed Origins\"\n";
        let mut g = Vec::new();
        put_uint(&mut g, 1, 3539);
        put_uint(&mut g, 20, 1);
        put_uint(&mut g, 13, 3);
        put_bytes(&mut g, 17, cfg.as_bytes());
        put_bytes(&mut g, 30, b"space-1");
        put_varint(&mut g, 99 << 3 | 5); // unknown fixed32 field
        g.extend_from_slice(&[0, 0, 0, 0]);
        let mut g2 = Vec::new();
        put_uint(&mut g2, 1, 7);
        put_uint(&mut g2, 8, 1);
        let mut list = Vec::new();
        put_bytes(&mut list, 1, &g);
        let mut list2 = Vec::new();
        put_bytes(&mut list2, 1, &g2);
        let mut rsp = Vec::new();
        put_uint(&mut rsp, 1, 1);
        put_bytes(&mut rsp, 2, &list);
        put_bytes(&mut rsp, 7, &list2);
        let (ok, owned, claimed) = parse_initialize_rsp(&rsp);
        assert!(ok);
        assert_eq!(
            owned[0],
            DemuxGame {
                product_id: 3539,
                owned: true,
                product_type: 0,
                space_id: Some("space-1".into()),
                name: Some("Assassin's Creed Origins".into()),
                thumb_image: Some("abc.jpg".into())
            }
        );
        assert_eq!(
            claimed[0],
            DemuxGame { product_id: 7, owned: false, product_type: 1, space_id: None, name: None, thumb_image: None }
        );
        let all: Vec<DemuxGame> = owned.into_iter().chain(claimed).collect();
        let s = summarize(&all);
        assert!(s.contains("3539 | space-1 | Assassin's Creed Origins | abc.jpg"));
        assert!(!s.contains("7 | -"));
    }

    #[test]
    fn config_literal_name() {
        let c = "version: 2.0\nroot:\n  name: \"Far Cry 4\"\n  thumb_image: 1a2b3c.jpg\n";
        assert_eq!(parse_config(c), (Some("Far Cry 4".into()), Some("1a2b3c.jpg".into())));
    }

    #[test]
    fn config_name_via_localization_keys() {
        let c = "root:\n  name: GAMENAME\n  thumb_image: THUMBIMAGE\nlocalizations:\n  default:\n    GAMENAME: Far Cry® 4\n    THUMBIMAGE: 1a2b3c.jpg\n";
        assert_eq!(parse_config(c), (Some("Far Cry® 4".into()), Some("1a2b3c.jpg".into())));
        let c = "root:\n  name: l1\nlocalizations:\n  default:\n    l1: 'Watch Dogs'\n";
        assert_eq!(parse_config(c).0, Some("Watch Dogs".into()));
    }

    #[test]
    fn config_nested_name_ignored() {
        let c = "root:\n  start_game:\n    online:\n      name: Wrong\n  name: Right\n";
        assert_eq!(parse_config(c).0, Some("Right".into()));
        let c = "root:\n  start_game:\n    name: Wrong\n";
        assert_eq!(parse_config(c).0, None);
    }

    #[test]
    fn config_thumb_via_key_and_unresolved() {
        let c = "root:\n  name: GAMENAME\n  thumb_image: THUMB\nlocalizations:\n  default:\n    THUMB: x.png\n";
        assert_eq!(parse_config(c), (None, Some("x.png".into())));
        assert_eq!(parse_config("root:\n  name:\n"), (None, None));
        assert_eq!(parse_config("nothing"), (None, None));
    }
}
