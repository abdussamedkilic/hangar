use crate::accounts::epic::CatalogInfo;
use crate::merge::Override;
use crate::model::{Game, Platform, PlatformEntry};
use rusqlite::{params, Connection, OptionalExtension, Result};
use std::collections::HashMap;
use std::path::Path;

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
                is_game INTEGER NOT NULL);
             CREATE TABLE IF NOT EXISTS owned_cache(
                platform TEXT PRIMARY KEY,
                json TEXT NOT NULL,
                fetched_at INTEGER NOT NULL);",
        )?;
        Ok(Db { conn })
    }

    pub fn replace_entries(&self, groups: Vec<(String, Vec<PlatformEntry>)>) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute("DELETE FROM platform_entries", [])?;
        for (norm, entries) in &groups {
            // Display title: Steam entry if present, else first by Platform::ALL order.
            let Some(display) = entries.iter().min_by_key(|e| rank(e.platform)) else {
                continue;
            };
            tx.execute(
                "INSERT INTO games(title, norm_title) VALUES(?1, ?2)
                 ON CONFLICT(norm_title) DO UPDATE SET title = excluded.title",
                params![display.title, norm],
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
            let e: PlatformEntry = serde_json::from_str(&json).map_err(|err| {
                rusqlite::Error::FromSqlConversionFailure(
                    1,
                    rusqlite::types::Type::Text,
                    Box::new(err),
                )
            })?;
            by_game.entry(id).or_default().push(e);
        }
        let mut st = self.conn.prepare(
            "SELECT id, title, norm_title, cover_path, preferred, last_launched
             FROM games ORDER BY title COLLATE NOCASE, id",
        )?;
        let rows = st.query_map([], |r| {
            let id: i64 = r.get(0)?;
            let preferred: Option<String> = r.get(4)?;
            Ok(Game {
                id,
                title: r.get(1)?,
                norm_title: r.get(2)?,
                cover_path: r.get(3)?,
                preferred: preferred.as_deref().and_then(platform_from),
                last_launched: r.get(5)?,
                entries: Vec::new(),
            })
        })?;
        let mut games = Vec::new();
        for g in rows {
            let mut g = g?;
            g.entries = by_game.remove(&g.id).unwrap_or_default();
            g.entries.sort_by_key(|e| rank(e.platform));
            games.push(g);
        }
        Ok(games)
    }

    pub fn set_preferred(&self, game_id: i64, p: Platform) -> Result<()> {
        self.conn.execute(
            "UPDATE games SET preferred = ?1 WHERE id = ?2",
            params![p.as_str(), game_id],
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
            .prepare("SELECT title, is_game FROM epic_catalog WHERE item_id = ?1")?;
        let mut out = HashMap::new();
        for id in ids {
            let row = st
                .query_row([id], |r| {
                    Ok(CatalogInfo {
                        title: r.get(0)?,
                        is_game: r.get(1)?,
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
                "INSERT OR REPLACE INTO epic_catalog(item_id, title, is_game) VALUES(?1, ?2, ?3)",
                params![id, info.title, info.is_game],
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
        m.insert("a".to_string(), CatalogInfo { title: "Alpha".into(), is_game: true });
        m.insert("b".to_string(), CatalogInfo { title: "Beta DLC".into(), is_game: false });
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
}
