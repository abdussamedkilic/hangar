use crate::accounts::epic::CatalogInfo;
use crate::merge::{display_base, edition_of, normalize, Override};
use crate::model::{Game, Platform, PlatformEntry};
use rusqlite::{params, Connection, OptionalExtension, Result};
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Collection {
    pub id: i64,
    pub name: String,
    pub entries: Vec<(Platform, String)>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct SavedFilter {
    pub id: i64,
    pub name: String,
    /// JSON text owned by the UI.
    pub rule: String,
}

#[derive(Debug, PartialEq)]
pub enum NameError {
    Empty,
    Duplicate,
}

/// Trim + Turkish-aware lowercase (`İ→i`, `I→ı`), used for collection name uniqueness.
pub fn fold_name(s: &str) -> String {
    s.trim()
        .chars()
        .flat_map(|c| match c {
            'İ' => vec!['i'],
            'I' => vec!['ı'],
            c => c.to_lowercase().collect(),
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq)]
pub struct LocalGame {
    pub id: i64,
    pub title: String,
    pub exe_path: String,
    pub args: Vec<String>,
    pub source: String,
}

pub struct Db {
    conn: Connection,
}

fn platform_from(s: &str) -> Option<Platform> {
    Platform::ALL.into_iter().find(|p| p.as_str() == s)
}

fn rank(p: Platform) -> usize {
    Platform::ALL
        .iter()
        .position(|x| *x == p)
        .unwrap_or(usize::MAX)
}

/// Menu order: installed first, then preferred, then entries without a copy marker, then platform
/// priority, then platform_id.
fn sort_menu(entries: &mut [PlatformEntry], preferred: Option<&(Platform, String)>) {
    entries.sort_by_key(|e| {
        (
            !e.installed,
            !preferred.is_some_and(|(p, id)| *p == e.platform && *id == e.platform_id),
            e.copy.is_some(),
            rank(e.platform),
            e.platform_id.clone(),
        )
    });
}

/// "platform:id" (an id may itself contain ':'), or the legacy bare "platform" = first entry of it.
fn resolve_preferred(entries: &[PlatformEntry], stored: &str) -> Option<(Platform, String)> {
    if let Some((p, id)) = stored.split_once(':') {
        let p = platform_from(p)?;
        return entries
            .iter()
            .find(|e| e.platform == p && e.platform_id == id)
            .map(|_| (p, id.to_string()));
    }
    let p = platform_from(stored)?;
    entries
        .iter()
        .find(|e| e.platform == p)
        .map(|e| (p, e.platform_id.clone()))
}

/// Preferred, else an installed Steam entry if any, else the installed entry with the latest
/// last_played (ties: menu order; among Steam entries too), else (nothing installed) non-Original
/// editions first, then menu order. `entries` must be in menu order.
fn default_entry(
    entries: &[PlatformEntry],
    preferred: Option<&(Platform, String)>,
) -> Option<(Platform, String)> {
    if let Some(p) = preferred {
        return Some(p.clone());
    }
    let key = |e: &PlatformEntry| (e.platform, e.platform_id.clone());
    let mut best: Option<&PlatformEntry> = None;
    let any_steam = entries.iter().any(|e| e.installed && e.platform == Platform::Steam);
    for e in entries.iter().filter(|e| e.installed && (!any_steam || e.platform == Platform::Steam)) {
        if best.is_none_or(|b| e.last_played > b.last_played) {
            best = Some(e);
        }
    }
    if let Some(b) = best {
        return Some(key(b));
    }
    entries.iter().min_by_key(|e| e.edition.is_none()).map(key)
}

impl Db {
    pub fn open(path: &Path) -> Result<Db> {
        Self::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> Result<Db> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Db> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS games(
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL,
                norm_title TEXT NOT NULL UNIQUE,
                cover_path TEXT,
                preferred TEXT,
                last_launched INTEGER);
             CREATE TABLE IF NOT EXISTS platform_entries(
                platform TEXT NOT NULL,
                platform_id TEXT NOT NULL,
                game_id INTEGER NOT NULL,
                json TEXT NOT NULL,
                PRIMARY KEY(platform, platform_id));
             CREATE TABLE IF NOT EXISTS settings(key TEXT PRIMARY KEY, value TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS merge_overrides(
                a_platform TEXT NOT NULL, a_id TEXT NOT NULL,
                b_platform TEXT NOT NULL, b_id TEXT NOT NULL,
                merged INTEGER NOT NULL);
             CREATE TABLE IF NOT EXISTS local_games(
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL,
                exe_path TEXT NOT NULL,
                args TEXT NOT NULL DEFAULT '[]',
                source TEXT NOT NULL,
                added_at INTEGER NOT NULL);
             CREATE TABLE IF NOT EXISTS epic_catalog(
                item_id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                is_game INTEGER NOT NULL,
                cover_url TEXT);
             CREATE TABLE IF NOT EXISTS owned_cache(
                platform TEXT PRIMARY KEY,
                json TEXT NOT NULL,
                fetched_at INTEGER NOT NULL);
             CREATE TABLE IF NOT EXISTS collections(
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                position INTEGER NOT NULL,
                created_at INTEGER NOT NULL);
             CREATE TABLE IF NOT EXISTS collection_entries(
                collection_id INTEGER NOT NULL,
                platform TEXT NOT NULL,
                platform_id TEXT NOT NULL,
                PRIMARY KEY(collection_id, platform, platform_id));
             CREATE TABLE IF NOT EXISTS filters(
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                rule TEXT NOT NULL,
                position INTEGER NOT NULL);",
        )?;
        // Older DBs lack the column.
        let has_cover: bool = conn
            .prepare("PRAGMA table_info(epic_catalog)")?
            .query_map([], |r| r.get::<_, String>(1))?
            .any(|n| n.is_ok_and(|n| n == "cover_url"));
        if !has_cover {
            conn.execute("ALTER TABLE epic_catalog ADD COLUMN cover_url TEXT", [])?;
        }
        // One-time (settings flag, also for DBs that already have the column): drop cached catalog
        // rows so the next scan refetches them with cover urls.
        let flag = "migr.epic_catalog_cover_v1";
        let done: Option<String> = conn
            .query_row("SELECT value FROM settings WHERE key = ?1", [flag], |r| r.get(0))
            .optional()?;
        if done.is_none() {
            conn.execute("DELETE FROM epic_catalog", [])?;
            conn.execute("INSERT OR REPLACE INTO settings(key, value) VALUES(?1, '1')", [flag])?;
        }
        Ok(Db { conn })
    }

    pub fn replace_entries(&self, groups: Vec<(String, Vec<PlatformEntry>)>) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute("DELETE FROM platform_entries", [])?;
        for (norm, entries) in &groups {
            // Display title: Steam entry if present, else first by Platform::ALL order.
            // Title from an entry that belongs under this key (a hand-merged mixed-edition card is
            // named after the entry whose own key it carries); else the best-ranked entry.
            let own = |e: &&PlatformEntry| {
                let ed = edition_of(&e.title);
                let k = if ed.label == "Orijinal" {
                    normalize(&e.title)
                } else {
                    format!("{}|{}", normalize(&e.title), ed.label)
                };
                k == *norm
            };
            let display = entries
                .iter()
                .filter(own)
                .min_by_key(|e| rank(e.platform))
                .or_else(|| entries.iter().min_by_key(|e| rank(e.platform)));
            let Some(display) = display else {
                continue;
            };
            let ed = edition_of(&display.title);
            let title = if ed.label == "Orijinal" {
                display_base(&display.title)
            } else {
                format!("{} {}", display_base(&display.title), ed.label)
            };
            tx.execute(
                "INSERT INTO games(title, norm_title) VALUES(?1, ?2)
                 ON CONFLICT(norm_title) DO UPDATE SET title = excluded.title",
                params![title, norm],
            )?;
            let id: i64 =
                tx.query_row("SELECT id FROM games WHERE norm_title = ?1", [norm], |r| {
                    r.get(0)
                })?;
            for e in entries {
                let json = serde_json::to_string(e)
                    .map_err(|err| rusqlite::Error::ToSqlConversionFailure(Box::new(err)))?;
                tx.execute(
                    "INSERT OR REPLACE INTO platform_entries(platform, platform_id, game_id, json)
                     VALUES(?1, ?2, ?3, ?4)",
                    params![e.platform.as_str(), e.platform_id, id, json],
                )?;
            }
        }
        tx.execute(
            "DELETE FROM games WHERE id NOT IN (SELECT game_id FROM platform_entries)",
            [],
        )?;
        tx.commit()
    }

    pub fn games(&self) -> Result<Vec<Game>> {
        let mut by_game: HashMap<i64, Vec<PlatformEntry>> = HashMap::new();
        let mut st = self
            .conn
            .prepare("SELECT game_id, json FROM platform_entries")?;
        for row in st.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))? {
            let (id, json) = row?;
            let mut e: PlatformEntry = serde_json::from_str(&json).map_err(|err| {
                rusqlite::Error::FromSqlConversionFailure(
                    1,
                    rusqlite::types::Type::Text,
                    Box::new(err),
                )
            })?;
            let ed = edition_of(&e.title);
            e.edition = (ed.label != "Orijinal").then_some(ed.label);
            e.copy = ed.copy;
            by_game.entry(id).or_default().push(e);
        }
        let mut st = self.conn.prepare(
            "SELECT id, title, norm_title, cover_path, preferred, last_launched
             FROM games ORDER BY title COLLATE NOCASE, id",
        )?;
        let rows = st.query_map([], |r| {
            Ok((
                Game {
                    id: r.get(0)?,
                    title: r.get(1)?,
                    norm_title: r.get(2)?,
                    cover_path: r.get(3)?,
                    preferred: None,
                    default_entry: None,
                    last_launched: r.get(5)?,
                    entries: Vec::new(),
                },
                r.get::<_, Option<String>>(4)?,
            ))
        })?;
        let mut games = Vec::new();
        for row in rows {
            let (mut g, pref) = row?;
            g.entries = by_game.remove(&g.id).unwrap_or_default();
            sort_menu(&mut g.entries, None);
            g.preferred = pref.as_deref().and_then(|p| resolve_preferred(&g.entries, p));
            sort_menu(&mut g.entries, g.preferred.as_ref());
            g.default_entry = default_entry(&g.entries, g.preferred.as_ref());
            games.push(g);
        }
        Ok(games)
    }

    /// Stores the preference per entry. `platform_id` omitted: first entry of that platform in menu order.
    pub fn set_preferred(&self, game_id: i64, p: Platform, platform_id: Option<&str>) -> Result<()> {
        let id = match platform_id {
            Some(id) => id.to_string(),
            None => {
                let games = self.games()?;
                let first = games
                    .iter()
                    .find(|g| g.id == game_id)
                    .and_then(|g| g.entries.iter().find(|e| e.platform == p));
                match first {
                    Some(e) => e.platform_id.clone(),
                    None => return Ok(()),
                }
            }
        };
        self.conn.execute(
            "UPDATE games SET preferred = ?1 WHERE id = ?2",
            params![format!("{}:{}", p.as_str(), id), game_id],
        )?;
        Ok(())
    }

    pub fn record_launch(&self, game_id: i64, at: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE games SET last_launched = ?1 WHERE id = ?2",
            params![at, game_id],
        )?;
        Ok(())
    }

    pub fn set_cover(&self, game_id: i64, path: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE games SET cover_path = ?1 WHERE id = ?2",
            params![path, game_id],
        )?;
        Ok(())
    }

    pub fn get_setting(&self, k: &str) -> Result<Option<String>> {
        self.conn
            .query_row("SELECT value FROM settings WHERE key = ?1", [k], |r| {
                r.get(0)
            })
            .optional()
    }

    pub fn set_setting(&self, k: &str, v: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO settings(key, value) VALUES(?1, ?2)",
            params![k, v],
        )?;
        Ok(())
    }

    pub fn catalog_get(&self, ids: &[String]) -> Result<HashMap<String, CatalogInfo>> {
        let mut st = self
            .conn
            .prepare("SELECT title, is_game, cover_url FROM epic_catalog WHERE item_id = ?1")?;
        let mut out = HashMap::new();
        for id in ids {
            let row = st
                .query_row([id], |r| {
                    Ok(CatalogInfo {
                        title: r.get(0)?,
                        is_game: r.get(1)?,
                        cover_url: r.get(2)?,
                    })
                })
                .optional()?;
            if let Some(info) = row {
                out.insert(id.clone(), info);
            }
        }
        Ok(out)
    }

    pub fn catalog_put(&self, items: &HashMap<String, CatalogInfo>) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        for (id, info) in items {
            tx.execute(
                "INSERT OR REPLACE INTO epic_catalog(item_id, title, is_game, cover_url) VALUES(?1, ?2, ?3, ?4)",
                params![id, info.title, info.is_game, info.cover_url],
            )?;
        }
        tx.commit()
    }

    pub fn catalog_clear(&self) -> Result<()> {
        self.conn.execute("DELETE FROM epic_catalog", [])?;
        Ok(())
    }

    pub fn owned_cache_get(&self, p: Platform) -> Result<Option<Vec<PlatformEntry>>> {
        let json: Option<String> = self
            .conn
            .query_row(
                "SELECT json FROM owned_cache WHERE platform = ?1",
                [p.as_str()],
                |r| r.get(0),
            )
            .optional()?;
        json.map(|j| {
            serde_json::from_str(&j).map_err(|err| {
                rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(err))
            })
        })
        .transpose()
    }

    pub fn owned_cache_put(&self, p: Platform, v: &[PlatformEntry], at: i64) -> Result<()> {
        let json = serde_json::to_string(v)
            .map_err(|err| rusqlite::Error::ToSqlConversionFailure(Box::new(err)))?;
        self.conn.execute(
            "INSERT OR REPLACE INTO owned_cache(platform, json, fetched_at) VALUES(?1, ?2, ?3)",
            params![p.as_str(), json, at],
        )?;
        Ok(())
    }

    pub fn owned_cache_delete(&self, p: Platform) -> Result<()> {
        self.conn
            .execute("DELETE FROM owned_cache WHERE platform = ?1", [p.as_str()])?;
        Ok(())
    }

    pub fn account_expired(&self, p: Platform) -> Result<bool> {
        Ok(self
            .get_setting(&format!("account_state.{}", p.as_str()))?
            .as_deref()
            == Some("expired"))
    }

    pub fn set_account_expired(&self, p: Platform, on: bool) -> Result<()> {
        let key = format!("account_state.{}", p.as_str());
        if on {
            self.set_setting(&key, "expired")
        } else {
            self.conn.execute("DELETE FROM settings WHERE key = ?1", [key])?;
            Ok(())
        }
    }

    pub fn add_local_game(&self, title: &str, exe: &str, source: &str) -> Result<i64> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        self.conn.execute(
            "INSERT INTO local_games(title, exe_path, source, added_at) VALUES(?1, ?2, ?3, ?4)",
            params![title, exe, source, now],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Inserts all items in one transaction: either every row is added or none.
    pub fn add_local_games(&self, items: &[(String, String)], source: &str) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        for (title, exe) in items {
            tx.execute(
                "INSERT INTO local_games(title, exe_path, source, added_at) VALUES(?1, ?2, ?3, ?4)",
                params![title, exe, source, now],
            )?;
        }
        tx.commit()
    }

    pub fn update_local_game(&self, id: i64, title: &str, exe: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE local_games SET title = ?1, exe_path = ?2 WHERE id = ?3",
            params![title, exe, id],
        )?;
        Ok(())
    }

    pub fn remove_local_game(&self, id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM local_games WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn local_games(&self) -> Result<Vec<LocalGame>> {
        let mut st = self
            .conn
            .prepare("SELECT id, title, exe_path, args, source FROM local_games ORDER BY id")?;
        let rows = st.query_map([], |r| {
            let args: String = r.get(3)?;
            Ok(LocalGame {
                id: r.get(0)?,
                title: r.get(1)?,
                exe_path: r.get(2)?,
                args: serde_json::from_str(&args).unwrap_or_default(),
                source: r.get(4)?,
            })
        })?;
        rows.collect()
    }

    pub fn overrides(&self) -> Result<Vec<Override>> {
        let mut st = self.conn.prepare(
            "SELECT a_platform, a_id, b_platform, b_id, merged FROM merge_overrides ORDER BY rowid",
        )?;
        let rows = st.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, bool>(4)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (ap, aid, bp, bid, merged) = row?;
            if let (Some(ap), Some(bp)) = (platform_from(&ap), platform_from(&bp)) {
                out.push(Override {
                    a: (ap, aid),
                    b: (bp, bid),
                    merged,
                });
            }
        }
        Ok(out)
    }

    pub fn collections(&self) -> Result<Vec<Collection>> {
        let mut out: Vec<Collection> = Vec::new();
        let mut st = self
            .conn
            .prepare("SELECT id, name FROM collections ORDER BY position, id")?;
        for row in st.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))? {
            let (id, name) = row?;
            out.push(Collection { id, name, entries: Vec::new() });
        }
        let mut st = self.conn.prepare(
            "SELECT collection_id, platform, platform_id FROM collection_entries
             ORDER BY platform, platform_id",
        )?;
        let rows = st.query_map([], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
        })?;
        for row in rows {
            let (cid, plat, pid) = row?;
            let Some(p) = Platform::ALL.into_iter().find(|p| p.as_str() == plat) else {
                continue;
            };
            if let Some(c) = out.iter_mut().find(|c| c.id == cid) {
                c.entries.push((p, pid));
            }
        }
        Ok(out)
    }

    /// `except`: the collection being renamed (its own name does not count as a duplicate).
    fn check_collection_name(
        &self,
        name: &str,
        except: Option<i64>,
    ) -> Result<std::result::Result<String, NameError>> {
        let name = name.trim();
        if name.is_empty() {
            return Ok(Err(NameError::Empty));
        }
        let folded = fold_name(name);
        let mut st = self.conn.prepare("SELECT id, name FROM collections")?;
        for row in st.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))? {
            let (id, other) = row?;
            if Some(id) != except && fold_name(&other) == folded {
                return Ok(Err(NameError::Duplicate));
            }
        }
        Ok(Ok(name.to_string()))
    }

    pub fn filters(&self) -> Result<Vec<SavedFilter>> {
        let mut st = self.conn.prepare("SELECT id, name, rule FROM filters ORDER BY position, id")?;
        let rows = st.query_map([], |r| Ok(SavedFilter { id: r.get(0)?, name: r.get(1)?, rule: r.get(2)? }))?;
        rows.collect()
    }

    fn check_filter_name(&self, name: &str, except: Option<i64>) -> Result<std::result::Result<String, NameError>> {
        let name = name.trim();
        if name.is_empty() {
            return Ok(Err(NameError::Empty));
        }
        let folded = fold_name(name);
        let mut st = self.conn.prepare("SELECT id, name FROM filters")?;
        for row in st.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))? {
            let (id, other) = row?;
            if Some(id) != except && fold_name(&other) == folded {
                return Ok(Err(NameError::Duplicate));
            }
        }
        Ok(Ok(name.to_string()))
    }

    pub fn create_filter(&self, name: &str, rule: &str) -> Result<std::result::Result<i64, NameError>> {
        let name = match self.check_filter_name(name, None)? {
            Ok(n) => n,
            Err(e) => return Ok(Err(e)),
        };
        self.conn.execute(
            "INSERT INTO filters(name, rule, position)
             VALUES(?1, ?2, (SELECT COALESCE(MAX(position), 0) + 1 FROM filters))",
            params![name, rule],
        )?;
        Ok(Ok(self.conn.last_insert_rowid()))
    }

    pub fn update_filter(&self, id: i64, name: &str, rule: &str) -> Result<std::result::Result<(), NameError>> {
        let name = match self.check_filter_name(name, Some(id))? {
            Ok(n) => n,
            Err(e) => return Ok(Err(e)),
        };
        self.conn
            .execute("UPDATE filters SET name = ?1, rule = ?2 WHERE id = ?3", params![name, rule, id])?;
        Ok(Ok(()))
    }

    pub fn delete_filter(&self, id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM filters WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn create_collection(&self, name: &str) -> Result<std::result::Result<i64, NameError>> {
        let name = match self.check_collection_name(name, None)? {
            Ok(n) => n,
            Err(e) => return Ok(Err(e)),
        };
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        self.conn.execute(
            "INSERT INTO collections(name, position, created_at)
             VALUES(?1, (SELECT COALESCE(MAX(position), 0) + 1 FROM collections), ?2)",
            params![name, now],
        )?;
        Ok(Ok(self.conn.last_insert_rowid()))
    }

    pub fn rename_collection(&self, id: i64, name: &str) -> Result<std::result::Result<(), NameError>> {
        let name = match self.check_collection_name(name, Some(id))? {
            Ok(n) => n,
            Err(e) => return Ok(Err(e)),
        };
        self.conn
            .execute("UPDATE collections SET name = ?1 WHERE id = ?2", params![name, id])?;
        Ok(Ok(()))
    }

    pub fn delete_collection(&self, id: i64) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute("DELETE FROM collection_entries WHERE collection_id = ?1", [id])?;
        tx.execute("DELETE FROM collections WHERE id = ?1", [id])?;
        tx.commit()
    }

    /// Adds or removes the given entries (all entries of one card) to/from a collection.
    pub fn set_in_collection(&self, id: i64, entries: &[(Platform, String)], member: bool) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        for (p, pid) in entries {
            if member {
                tx.execute(
                    "INSERT OR IGNORE INTO collection_entries(collection_id, platform, platform_id)
                     VALUES(?1, ?2, ?3)",
                    params![id, p.as_str(), pid],
                )?;
            } else {
                tx.execute(
                    "DELETE FROM collection_entries
                     WHERE collection_id = ?1 AND platform = ?2 AND platform_id = ?3",
                    params![id, p.as_str(), pid],
                )?;
            }
        }
        tx.commit()
    }

    /// Ignores an exact duplicate.
    pub fn add_override(&self, o: Override) -> Result<()> {
        let (ap, bp) = (o.a.0.as_str(), o.b.0.as_str());
        // Last action wins: drop the opposite override for the same unordered pair.
        self.conn.execute(
            "DELETE FROM merge_overrides WHERE merged=?5 AND (
               (a_platform=?1 AND a_id=?2 AND b_platform=?3 AND b_id=?4) OR
               (a_platform=?3 AND a_id=?4 AND b_platform=?1 AND b_id=?2))",
            params![ap, o.a.1, bp, o.b.1, !o.merged],
        )?;
        let exists: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM merge_overrides
             WHERE a_platform=?1 AND a_id=?2 AND b_platform=?3 AND b_id=?4 AND merged=?5)",
            params![ap, o.a.1, bp, o.b.1, o.merged],
            |r| r.get(0),
        )?;
        if !exists {
            self.conn.execute(
                "INSERT INTO merge_overrides(a_platform, a_id, b_platform, b_id, merged)
                 VALUES(?1, ?2, ?3, ?4, ?5)",
                params![ap, o.a.1, bp, o.b.1, o.merged],
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_game_round_trip() {
        let db = Db::open_in_memory().unwrap();
        assert!(db.local_games().unwrap().is_empty());
        let id = db.add_local_game("Hades", r"C:\Games\Hades\Hades.exe", "manual").unwrap();
        let list = db.local_games().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, id);
        assert_eq!(list[0].title, "Hades");
        assert_eq!(list[0].exe_path, r"C:\Games\Hades\Hades.exe");
        assert_eq!(list[0].source, "manual");
        assert!(list[0].args.is_empty());
        db.update_local_game(id, "Hades II", r"D:\Hades2.exe").unwrap();
        let list = db.local_games().unwrap();
        assert_eq!((list[0].title.as_str(), list[0].exe_path.as_str()), ("Hades II", r"D:\Hades2.exe"));
        db.remove_local_game(id).unwrap();
        assert!(db.local_games().unwrap().is_empty());
    }

    #[test]
    fn catalog_roundtrip() {
        let db = Db::open_in_memory().unwrap();
        let mut m = HashMap::new();
        m.insert("a".to_string(), CatalogInfo { title: "Alpha".into(), is_game: true, cover_url: Some("https://x/a.jpg".into()) });
        m.insert("b".to_string(), CatalogInfo { title: "Beta DLC".into(), is_game: false, cover_url: None });
        db.catalog_put(&m).unwrap();
        let ids = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let got = db.catalog_get(&ids).unwrap();
        assert_eq!(got.len(), 2);
        assert_eq!(got["a"], m["a"]);
        assert_eq!(got["b"], m["b"]);
        assert!(db.catalog_get(&[]).unwrap().is_empty());
        db.catalog_clear().unwrap();
        assert!(db.catalog_get(&ids).unwrap().is_empty());
    }

    #[test]
    fn catalog_migrates_old_table() {
        let p = std::env::temp_dir().join(format!("hangar-migr-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&p);
        {
            let c = Connection::open(&p).unwrap();
            c.execute_batch(
                "CREATE TABLE epic_catalog(item_id TEXT PRIMARY KEY, title TEXT NOT NULL, is_game INTEGER NOT NULL);
                 INSERT INTO epic_catalog VALUES('a','Alpha',1);",
            )
            .unwrap();
        }
        let db = Db::open(&p).unwrap();
        assert!(db.catalog_get(&["a".to_string()]).unwrap().is_empty()); // purged once
        let mut m = HashMap::new();
        m.insert("b".to_string(), CatalogInfo { title: "B".into(), is_game: true, cover_url: None });
        db.catalog_put(&m).unwrap();
        drop(db);
        let db = Db::open(&p).unwrap(); // second open: column exists, rows kept
        assert_eq!(db.catalog_get(&["b".to_string()]).unwrap().len(), 1);
        drop(db);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn catalog_purge_runs_once_even_with_existing_column() {
        let p = std::env::temp_dir().join(format!("hangar-migr2-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&p);
        {
            // Already has cover_url (from the first release) but no flag.
            let c = Connection::open(&p).unwrap();
            c.execute_batch(
                "CREATE TABLE epic_catalog(item_id TEXT PRIMARY KEY, title TEXT NOT NULL, is_game INTEGER NOT NULL, cover_url TEXT);
                 INSERT INTO epic_catalog VALUES('a','Alpha',1,NULL);",
            )
            .unwrap();
        }
        let db = Db::open(&p).unwrap();
        assert!(db.catalog_get(&["a".to_string()]).unwrap().is_empty());
        assert_eq!(db.get_setting("migr.epic_catalog_cover_v1").unwrap().as_deref(), Some("1"));
        let mut m = HashMap::new();
        m.insert("b".to_string(), CatalogInfo { title: "B".into(), is_game: true, cover_url: None });
        db.catalog_put(&m).unwrap();
        drop(db);
        let db = Db::open(&p).unwrap();
        assert_eq!(db.catalog_get(&["b".to_string()]).unwrap().len(), 1);
        drop(db);
        let _ = std::fs::remove_file(&p);
        // Fresh DB: flag set, nothing to purge.
        let db = Db::open_in_memory().unwrap();
        assert_eq!(db.get_setting("migr.epic_catalog_cover_v1").unwrap().as_deref(), Some("1"));
    }

    #[test]
    fn owned_cache_roundtrip() {
        let db = Db::open_in_memory().unwrap();
        assert!(db.owned_cache_get(Platform::Epic).unwrap().is_none());
        let v = vec![PlatformEntry {
            platform: Platform::Epic,
            platform_id: "ns:item".into(),
            title: "Alpha".into(),
            installed: false,
            install_path: None,
            size_bytes: None,
            last_played: None,
            playtime_min: None,
            launch_uri: "x".into(),
            install_uri: None,
            launch_args: vec![],
            cover_url: None,
            edition: None,
            copy: None,
        }];
        db.owned_cache_put(Platform::Epic, &v, 100).unwrap();
        assert_eq!(db.owned_cache_get(Platform::Epic).unwrap(), Some(v));
        db.owned_cache_delete(Platform::Epic).unwrap();
        assert!(db.owned_cache_get(Platform::Epic).unwrap().is_none());
    }

    #[test]
    fn account_expired_flag() {
        let db = Db::open_in_memory().unwrap();
        assert!(!db.account_expired(Platform::Epic).unwrap());
        db.set_account_expired(Platform::Epic, true).unwrap();
        assert!(db.account_expired(Platform::Epic).unwrap());
        db.set_account_expired(Platform::Epic, false).unwrap();
        assert!(!db.account_expired(Platform::Epic).unwrap());
        assert_eq!(db.get_setting("account_state.epic").unwrap(), None);
    }

    #[test]
    fn local_games_survive_replace_entries() {
        let db = Db::open_in_memory().unwrap();
        db.add_local_game("Hades", "a.exe", "manual").unwrap();
        db.replace_entries(vec![]).unwrap();
        assert_eq!(db.local_games().unwrap().len(), 1);
    }

    #[test]
    fn add_local_games_is_all_or_nothing() {
        let db = Db::open_in_memory().unwrap();
        db.conn
            .execute_batch(
                "CREATE TRIGGER no_empty BEFORE INSERT ON local_games WHEN NEW.title = ''
                 BEGIN SELECT RAISE(ABORT, 'bad'); END;",
            )
            .unwrap();
        let bad = vec![("A".to_string(), "a.exe".to_string()), (String::new(), "b.exe".to_string())];
        assert!(db.add_local_games(&bad, "scan").is_err());
        assert!(db.local_games().unwrap().is_empty());
        db.add_local_games(&bad[..1], "scan").unwrap();
        assert_eq!(db.local_games().unwrap().len(), 1);
    }

    fn pe(p: Platform, id: &str, title: &str, installed: bool, last: Option<i64>) -> PlatformEntry {
        PlatformEntry {
            platform: p,
            platform_id: id.into(),
            title: title.into(),
            installed,
            install_path: None,
            size_bytes: None,
            last_played: last,
            playtime_min: None,
            launch_uri: format!("l:{id}"),
            install_uri: None,
            launch_args: vec![],
            cover_url: None,
            edition: None,
            copy: None,
        }
    }

    fn one_card(db: &Db, entries: Vec<PlatformEntry>) -> i64 {
        db.replace_entries(vec![("k".to_string(), entries)]).unwrap();
        db.games().unwrap()[0].id
    }

    #[test]
    fn mixed_edition_card_is_titled_from_key_entry() {
        let db = Db::open_in_memory().unwrap();
        db.replace_entries(vec![(
            "bioshock".to_string(),
            vec![
                pe(Platform::Steam, "r", "BioShock Remastered", false, None),
                pe(Platform::Gog, "o", "BioShock", false, None),
            ],
        )])
        .unwrap();
        assert_eq!(db.games().unwrap()[0].title, "BioShock");
    }

    #[test]
    fn legacy_preferred_reads_as_first_entry_of_platform() {
        let db = Db::open_in_memory().unwrap();
        let id = one_card(
            &db,
            vec![pe(Platform::Gog, "b", "X", false, None), pe(Platform::Gog, "a", "X", false, None), pe(Platform::Steam, "s", "X", false, None)],
        );
        db.conn.execute("UPDATE games SET preferred = 'gog' WHERE id = ?1", [id]).unwrap();
        assert_eq!(db.games().unwrap()[0].preferred, Some((Platform::Gog, "a".to_string())));
        db.conn.execute("UPDATE games SET preferred = 'riot' WHERE id = ?1", [id]).unwrap();
        assert_eq!(db.games().unwrap()[0].preferred, None);
    }

    #[test]
    fn preferred_roundtrip_and_menu_order() {
        let db = Db::open_in_memory().unwrap();
        let id = one_card(
            &db,
            vec![
                pe(Platform::Steam, "s", "X", false, None),
                pe(Platform::Epic, "ns:item", "X", false, None),
                pe(Platform::Gog, "g", "X - Amazon Prime", false, None),
            ],
        );
        db.set_preferred(id, Platform::Epic, Some("ns:item")).unwrap();
        let g = &db.games().unwrap()[0];
        assert_eq!(g.preferred, Some((Platform::Epic, "ns:item".to_string())));
        assert_eq!(g.default_entry, g.preferred);
        let order: Vec<&str> = g.entries.iter().map(|e| e.platform_id.as_str()).collect();
        assert_eq!(order, ["ns:item", "s", "g"]);
        assert_eq!(g.entries[2].copy.as_deref(), Some("Prime"));
        db.set_preferred(id, Platform::Gog, None).unwrap();
        assert_eq!(db.games().unwrap()[0].preferred, Some((Platform::Gog, "g".to_string())));
        // A preference for a vanished entry is ignored.
        db.set_preferred(id, Platform::Gog, Some("gone")).unwrap();
        assert_eq!(db.games().unwrap()[0].preferred, None);
    }

    #[test]
    fn default_entry_rules() {
        let db = Db::open_in_memory().unwrap();
        // Installed Steam wins even when another store was played later.
        one_card(
            &db,
            vec![pe(Platform::Steam, "s", "X", true, Some(10)), pe(Platform::Gog, "g", "X", true, Some(20)), pe(Platform::Epic, "e", "X", false, Some(99))],
        );
        assert_eq!(db.games().unwrap()[0].default_entry, Some((Platform::Steam, "s".to_string())));
        // No Steam: latest played installed wins over platform priority.
        one_card(&db, vec![pe(Platform::Gog, "g", "X", true, Some(10)), pe(Platform::Epic, "e", "X", true, Some(20))]);
        assert_eq!(db.games().unwrap()[0].default_entry, Some((Platform::Epic, "e".to_string())));
        // Two installed Steam copies: the latest played one.
        one_card(&db, vec![pe(Platform::Steam, "s1", "X", true, Some(10)), pe(Platform::Steam, "s2", "X", true, Some(20))]);
        assert_eq!(db.games().unwrap()[0].default_entry, Some((Platform::Steam, "s2".to_string())));
        // Tie: platform priority.
        one_card(&db, vec![pe(Platform::Gog, "g", "X", true, None), pe(Platform::Steam, "s", "X", true, None)]);
        assert_eq!(db.games().unwrap()[0].default_entry, Some((Platform::Steam, "s".to_string())));
        // Nothing installed: non-Original first, then platform priority.
        one_card(
            &db,
            vec![pe(Platform::Steam, "s", "X", false, None), pe(Platform::Gog, "g", "X Remastered", false, None), pe(Platform::Epic, "e", "X Remastered", false, None)],
        );
        assert_eq!(db.games().unwrap()[0].default_entry, Some((Platform::Epic, "e".to_string())));
        // (Last: the preference sticks to the card row.) A preference wins over Steam.
        let id = one_card(&db, vec![pe(Platform::Steam, "s", "X", true, Some(10)), pe(Platform::Gog, "g", "X", true, Some(20))]);
        db.set_preferred(id, Platform::Gog, Some("g")).unwrap();
        assert_eq!(db.games().unwrap()[0].default_entry, Some((Platform::Gog, "g".to_string())));
    }

    #[test]
    fn original_card_keeps_row_and_cover_across_rescan() {
        let db = Db::open_in_memory().unwrap();
        let es = |extra: bool| {
            let mut v = vec![pe(Platform::Steam, "7670", "BioShock", false, None)];
            if extra {
                v.push(pe(Platform::Steam, "409710", "BioShock Remastered", false, None));
            }
            v
        };
        let groups = |es: Vec<PlatformEntry>| crate::merge::group(es, &[]);
        db.replace_entries(groups(es(false))).unwrap();
        let g = &db.games().unwrap()[0];
        assert_eq!(g.title, "BioShock");
        let id = g.id;
        db.set_cover(id, "c.webp").unwrap();
        db.replace_entries(groups(es(true))).unwrap();
        let games = db.games().unwrap();
        assert_eq!(games.len(), 2);
        let orig = games.iter().find(|g| g.norm_title == "bioshock").unwrap();
        assert_eq!((orig.id, orig.cover_path.as_deref()), (id, Some("c.webp")));
        let rem = games.iter().find(|g| g.norm_title == "bioshock|Remastered").unwrap();
        assert_eq!(rem.title, "BioShock Remastered");
        assert_eq!(rem.cover_path, None);
    }

    #[test]
    fn card_titles_from_display_base_and_label() {
        let db = Db::open_in_memory().unwrap();
        db.replace_entries(crate::merge::group(
            vec![
                pe(Platform::Gog, "1", "BioShock™ Remastered - Amazon Prime", false, None),
                pe(Platform::Gog, "2", "BioShock Infinite: Complete Edition", false, None),
                pe(Platform::Gog, "3", "BioShock™", false, None),
            ],
            &[],
        ))
        .unwrap();
        let mut t: Vec<String> = db.games().unwrap().into_iter().map(|g| g.title).collect();
        t.sort();
        assert_eq!(t, ["BioShock", "BioShock Infinite Complete Edition", "BioShock Remastered"]);
    }

    #[test]
    fn collections_crud() {
        let db = Db::open_in_memory().unwrap();
        let a = db.create_collection("A").unwrap().unwrap();
        let b = db.create_collection("B").unwrap().unwrap();
        let names: Vec<String> = db.collections().unwrap().into_iter().map(|c| c.name).collect();
        assert_eq!(names, ["A", "B"]);
        assert_eq!(db.rename_collection(a, "C").unwrap(), Ok(()));
        assert_eq!(db.rename_collection(a, "B").unwrap(), Err(NameError::Duplicate));
        assert_eq!(db.rename_collection(a, "C").unwrap(), Ok(()));
        assert_eq!(db.rename_collection(a, "A").unwrap(), Ok(()));
        db.set_in_collection(b, &[(Platform::Steam, "1".into())], true).unwrap();
        db.delete_collection(b).unwrap();
        assert_eq!(db.collections().unwrap().len(), 1);
        let n: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM collection_entries", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn filters_crud() {
        let db = Db::open_in_memory().unwrap();
        let a = db.create_filter("A", "{}").unwrap().unwrap();
        let b = db.create_filter("B", "{\"installed\":\"yes\"}").unwrap().unwrap();
        let got = db.filters().unwrap();
        assert_eq!(got.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["A", "B"]);
        assert_eq!(got[1].rule, "{\"installed\":\"yes\"}");
        assert_eq!(db.update_filter(a, "C", "{\"played\":\"no\"}").unwrap(), Ok(()));
        assert_eq!(db.update_filter(a, "B", "{}").unwrap(), Err(NameError::Duplicate));
        assert_eq!(db.update_filter(a, "C", "{}").unwrap(), Ok(()));
        db.delete_filter(b).unwrap();
        let left = db.filters().unwrap();
        assert_eq!(left.len(), 1);
        assert_eq!((left[0].name.as_str(), left[0].rule.as_str()), ("C", "{}"));
    }

    #[test]
    fn filter_names_turkish_fold() {
        let db = Db::open_in_memory().unwrap();
        db.create_filter("İndie", "{}").unwrap().unwrap();
        assert_eq!(db.create_filter("indie", "{}").unwrap(), Err(NameError::Duplicate));
        db.create_filter("Işık", "{}").unwrap().unwrap();
        assert_eq!(db.create_filter("ışık", "{}").unwrap(), Err(NameError::Duplicate));
        assert_eq!(db.create_filter("   ", "{}").unwrap(), Err(NameError::Empty));
        db.create_filter("  AC  ", "{}").unwrap().unwrap();
        assert!(db.filters().unwrap().iter().any(|f| f.name == "AC"));
    }

    #[test]
    fn collection_names_turkish_fold() {
        let db = Db::open_in_memory().unwrap();
        db.create_collection("İndie").unwrap().unwrap();
        assert_eq!(db.create_collection("indie").unwrap(), Err(NameError::Duplicate));
        db.create_collection("Işık").unwrap().unwrap();
        assert_eq!(db.create_collection("ışık").unwrap(), Err(NameError::Duplicate));
        assert_eq!(db.create_collection("   ").unwrap(), Err(NameError::Empty));
        db.create_collection("  AC  ").unwrap().unwrap();
        assert!(db.collections().unwrap().iter().any(|c| c.name == "AC"));
    }

    #[test]
    fn membership_survives_split_and_merge() {
        let db = Db::open_in_memory().unwrap();
        let entries = vec![
            pe(Platform::Steam, "1", "Game", false, None),
            pe(Platform::Ubisoft, "541", "Game", false, None),
        ];
        one_card(&db, entries.clone());
        let c = db.create_collection("X").unwrap().unwrap();
        let keys = [(Platform::Steam, "1".to_string()), (Platform::Ubisoft, "541".to_string())];
        db.set_in_collection(c, &keys, true).unwrap();
        // Split, then re-write the library through the override mechanism.
        db.add_override(Override { a: keys[0].clone(), b: keys[1].clone(), merged: false }).unwrap();
        db.replace_entries(crate::merge::group(entries.clone(), &db.overrides().unwrap())).unwrap();
        assert_eq!(db.games().unwrap().len(), 2);
        assert_eq!(db.collections().unwrap()[0].entries.len(), 2);
        // Merge back.
        db.add_override(Override { a: keys[0].clone(), b: keys[1].clone(), merged: true }).unwrap();
        db.replace_entries(crate::merge::group(entries, &db.overrides().unwrap())).unwrap();
        assert_eq!(db.games().unwrap().len(), 1);
        let got = db.collections().unwrap()[0].entries.clone();
        assert!(keys.iter().all(|k| got.contains(k)) && got.len() == 2);
    }

    #[test]
    fn rescan_keeps_membership() {
        let db = Db::open_in_memory().unwrap();
        one_card(&db, vec![pe(Platform::Steam, "1", "Game", false, None)]);
        let c = db.create_collection("X").unwrap().unwrap();
        db.set_in_collection(c, &[(Platform::Steam, "1".into())], true).unwrap();
        db.replace_entries(vec![]).unwrap();
        assert!(db.games().unwrap().is_empty());
        assert_eq!(db.collections().unwrap()[0].entries, [(Platform::Steam, "1".to_string())]);
    }

    #[test]
    fn set_in_collection_idempotent() {
        let db = Db::open_in_memory().unwrap();
        let c = db.create_collection("X").unwrap().unwrap();
        let e = [(Platform::Steam, "1".to_string()), (Platform::Gog, "2".to_string())];
        db.set_in_collection(c, &e[..1], true).unwrap();
        db.set_in_collection(c, &e[..1], true).unwrap();
        assert_eq!(db.collections().unwrap()[0].entries.len(), 1);
        db.set_in_collection(c, &e, true).unwrap();
        assert_eq!(db.collections().unwrap()[0].entries.len(), 2);
        db.set_in_collection(c, &e, false).unwrap();
        assert!(db.collections().unwrap()[0].entries.is_empty());
    }

    /// Dry run on a COPY of the live DB: `HANGAR_DB=<copy> HANGAR_DRY_OUT=<report.md> cargo test -p
    /// hangar-core dry_run_live_cards -- --ignored`. Never opens the live file for writing.
    #[test]
    #[ignore]
    fn dry_run_live_cards() {
        use std::collections::BTreeMap;
        use std::fmt::Write;
        let (Ok(src), Ok(out)) = (std::env::var("HANGAR_DB"), std::env::var("HANGAR_DRY_OUT")) else {
            return;
        };
        let db = Db::open(Path::new(&src)).unwrap();
        let before = db.games().unwrap();
        let entries: Vec<PlatformEntry> = before.iter().flat_map(|g| g.entries.clone()).collect();
        let overrides = db.overrides().unwrap();
        let groups = crate::merge::group(entries.clone(), &overrides);
        let mut cards: Vec<(String, String, Vec<String>)> = groups
            .iter()
            .map(|(k, es)| {
                let display = es.iter().min_by_key(|e| rank(e.platform)).unwrap();
                let ed = edition_of(&display.title);
                let title = if ed.label == "Orijinal" {
                    display_base(&display.title)
                } else {
                    format!("{} {}", display_base(&display.title), ed.label)
                };
                let list = es.iter().map(|e| format!("{}:{} ({})", e.platform.as_str(), e.platform_id, e.title)).collect();
                (title, k.clone(), list)
            })
            .collect();
        cards.sort();
        let mut r = String::new();
        writeln!(r, "# Dry run: kartlar

Entries: {}
Cards before: {}
Cards after: {}
", entries.len(), before.len(), cards.len()).unwrap();
        writeln!(r, "## Cards with more than one entry
").unwrap();
        for (title, key, list) in cards.iter().filter(|c| c.2.len() > 1) {
            writeln!(r, "- **{title}** `{key}` ({})", list.len()).unwrap();
            for l in list {
                writeln!(r, "    - {l}").unwrap();
            }
        }
        let mut by_base: BTreeMap<String, Vec<&str>> = BTreeMap::new();
        for (title, key, _) in &cards {
            let base = key.split(['|', '#']).next().unwrap_or(key).to_string();
            by_base.entry(base).or_default().push(title.as_str());
        }
        writeln!(r, "
## Base names with several cards
").unwrap();
        for (base, titles) in by_base.iter().filter(|(_, t)| t.len() > 1) {
            writeln!(r, "- {base}: {}", titles.join(" | ")).unwrap();
        }
        std::fs::write(out, r).unwrap();
    }
}
