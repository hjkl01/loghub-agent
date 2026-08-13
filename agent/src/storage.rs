use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlite::{Connection, State};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct QueueItem {
    pub id: i64,
    pub payload: Value,
}

pub struct QueueStore {
    conn: Connection,
}

impl QueueStore {
    pub fn open(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS queue (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                payload TEXT NOT NULL,
                created_at INTEGER NOT NULL DEFAULT (strftime('%s','now'))
            )",
        )?;
        Ok(Self { conn })
    }

    pub fn push(&self, payload: &Value) -> Result<()> {
        let mut stmt = self.conn.prepare("INSERT INTO queue(payload) VALUES(?)")?;
        stmt.bind((1, payload.to_string().as_str()))?;
        stmt.next()?;
        Ok(())
    }

    pub fn list(&self, limit: i32) -> Result<Vec<QueueItem>> {
        let mut stmt = self.conn.prepare("SELECT id,payload FROM queue ORDER BY id LIMIT ?")?;
        stmt.bind((1, limit))?;
        let mut result = Vec::new();
        while let State::Row = stmt.next()? {
            result.push(QueueItem {
                id: stmt.read::<i64, _>(0)?,
                payload: serde_json::from_str(stmt.read::<String, _>(1)?.as_str())?,
            });
        }
        Ok(result)
    }

    pub fn remove(&self, id: i64) -> Result<()> {
        let mut stmt = self.conn.prepare("DELETE FROM queue WHERE id=?")?;
        stmt.bind((1, id))?;
        stmt.next()?;
        Ok(())
    }
}
