use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlite::{Connection, State};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct QueueItem {
    pub id: i64,
    pub payload: Value,
    pub retry_count: i64,
}

pub struct QueueStore {
    conn: Connection,
}

impl QueueStore {
    pub fn open(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute(
            "PRAGMA journal_mode=WAL;
             CREATE TABLE IF NOT EXISTS queue (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                payload TEXT NOT NULL,
                retry_count INTEGER NOT NULL DEFAULT 0,
                last_retry_at INTEGER,
                locked_until INTEGER,
                created_at INTEGER NOT NULL DEFAULT (strftime('%s','now'))
             );",
        )?;
        Ok(Self { conn })
    }

    pub fn push(&self, payload: &Value) -> Result<()> {
        let payload = payload.to_string();
        let mut stmt = self.conn.prepare("INSERT INTO queue(payload) VALUES(?)")?;
        stmt.bind((1, payload.as_str()))?;
        stmt.next()?;
        Ok(())
    }

    pub fn reserve_batch(&self, limit: i32, lease_seconds: i64) -> Result<Vec<QueueItem>> {
        let mut stmt = self.conn.prepare(
            "UPDATE queue SET locked_until = strftime('%s','now') + ?
             WHERE id IN (
               SELECT id FROM queue
               WHERE locked_until IS NULL OR locked_until < strftime('%s','now')
               ORDER BY id LIMIT ?
             )",
        )?;
        stmt.bind((1, lease_seconds))?;
        stmt.bind((2, limit))?;
        stmt.next()?;
        self.list(limit)
    }

    pub fn list(&self, limit: i32) -> Result<Vec<QueueItem>> {
        let mut stmt = self.conn.prepare("SELECT id,payload,retry_count FROM queue ORDER BY id LIMIT ?")?;
        stmt.bind((1, limit))?;
        let mut result = Vec::new();
        while let State::Row = stmt.next()? {
            result.push(QueueItem {
                id: stmt.read::<i64, _>(0)?,
                payload: serde_json::from_str(stmt.read::<String, _>(1)?.as_str())?,
                retry_count: stmt.read::<i64, _>(2)?,
            });
        }
        Ok(result)
    }

    pub fn ack(&self, id: i64) -> Result<()> {
        self.remove(id)
    }

    pub fn remove(&self, id: i64) -> Result<()> {
        let mut stmt = self.conn.prepare("DELETE FROM queue WHERE id=?")?;
        stmt.bind((1, id))?;
        stmt.next()?;
        Ok(())
    }

    pub fn increase_retry(&self, id: i64) -> Result<()> {
        let mut stmt = self.conn.prepare("UPDATE queue SET retry_count = retry_count + 1, last_retry_at = strftime('%s','now'), locked_until = NULL WHERE id=?")?;
        stmt.bind((1, id))?;
        stmt.next()?;
        Ok(())
    }

    pub fn count(&self) -> Result<i64> {
        let mut stmt = self.conn.prepare("SELECT COUNT(*) FROM queue")?;
        stmt.next()?;
        Ok(stmt.read::<i64, _>(0)?)
    }

    pub fn trim_oldest(&self, keep: i64) -> Result<()> {
        let mut stmt = self.conn.prepare("DELETE FROM queue WHERE id NOT IN (SELECT id FROM queue ORDER BY id DESC LIMIT ?)")?;
        stmt.bind((1, keep))?;
        stmt.next()?;
        Ok(())
    }
}
