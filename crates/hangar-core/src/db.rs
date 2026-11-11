use crate::merge::Override;
use crate::model::{Game, Platform, PlatformEntry};
use rusqlite::{params, Connection, OptionalExtension, Result};
use std::collections::HashMap;
use std::path::Path;

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
                merged INTEGER NOT NULL);",
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
