use std::path::PathBuf;

use anyhow::Result;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnProfile {
    pub id: i64,
    pub name: String,
    pub hosts: String,
    pub timeout_ms: i32,
    pub auth_scheme: String,
    pub auth_credential: String,
    pub category: String,
    pub folder_id: Option<i64>,
    pub position: i32,
    pub created_at: String,
    pub last_used_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Folder {
    pub id: i64,
    pub name: String,
    pub parent_id: Option<i64>,
    pub position: i32,
}

pub struct LocalDb {
    conn: Connection,
}

impl LocalDb {
    pub fn new() -> Result<Self> {
        let db_path = Self::db_path();
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(db_path)?;
        conn.execute_batch("
            CREATE TABLE IF NOT EXISTS folders (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                parent_id INTEGER REFERENCES folders(id) ON DELETE CASCADE,
                position INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS connections (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                hosts TEXT NOT NULL DEFAULT '127.0.0.1:2181',
                timeout_ms INTEGER NOT NULL DEFAULT 5000,
                auth_scheme TEXT NOT NULL DEFAULT '',
                auth_credential TEXT NOT NULL DEFAULT '',
                category TEXT NOT NULL DEFAULT '',
                folder_id INTEGER REFERENCES folders(id) ON DELETE SET NULL,
                position INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL,
                last_used_at TEXT
            );
        ")?;
        // Migration: add missing columns for pre-existing databases
        let has_folder_id: bool = conn.prepare("SELECT folder_id FROM connections LIMIT 1")
            .map(|_| true).unwrap_or(false);
        if !has_folder_id {
            let _ = conn.execute_batch("ALTER TABLE connections ADD COLUMN folder_id INTEGER REFERENCES folders(id) ON DELETE SET NULL;");
        }
        let has_position_conn: bool = conn.prepare("SELECT position FROM connections LIMIT 1")
            .map(|_| true).unwrap_or(false);
        if !has_position_conn {
            let _ = conn.execute_batch("ALTER TABLE connections ADD COLUMN position INTEGER NOT NULL DEFAULT 0;");
        }
        let has_position_folder: bool = conn.prepare("SELECT position FROM folders LIMIT 1")
            .map(|_| true).unwrap_or(false);
        if !has_position_folder {
            let _ = conn.execute_batch("ALTER TABLE folders ADD COLUMN position INTEGER NOT NULL DEFAULT 0;");
        }
        conn.execute_batch("
            CREATE INDEX IF NOT EXISTS idx_conn_category ON connections(category);
            CREATE INDEX IF NOT EXISTS idx_conn_folder ON connections(folder_id);
            CREATE INDEX IF NOT EXISTS idx_conn_last_used ON connections(last_used_at DESC);
        ")?;
        Ok(Self { conn })
    }

    fn db_path() -> PathBuf {
        if let Some(dir) = std::env::var_os("XDG_DATA_HOME") {
            PathBuf::from(dir).join("zk-ui/zk-ui.db")
        } else if let Some(dir) = std::env::var_os("HOME") {
            PathBuf::from(dir).join(".local/share/zk-ui/zk-ui.db")
        } else {
            PathBuf::from("zk-ui.db")
        }
    }

    pub fn add_connection(&self, name: &str, hosts: &str, timeout_ms: i32, auth_scheme: &str, auth_credential: &str, folder_id: Option<i64>) -> Result<i64> {
        let now = chrono::Local::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO connections (name, hosts, timeout_ms, auth_scheme, auth_credential, folder_id, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![name, hosts, timeout_ms, auth_scheme, auth_credential, folder_id, now],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn update_connection(&self, id: i64, name: &str, hosts: &str, timeout_ms: i32, auth_scheme: &str, auth_credential: &str, folder_id: Option<i64>) -> Result<()> {
        self.conn.execute(
            "UPDATE connections SET name=?1, hosts=?2, timeout_ms=?3, auth_scheme=?4, auth_credential=?5, folder_id=?6 WHERE id=?7",
            params![name, hosts, timeout_ms, auth_scheme, auth_credential, folder_id, id],
        )?;
        Ok(())
    }

    pub fn delete_connection(&self, id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM connections WHERE id=?1", params![id])?;
        Ok(())
    }

    pub fn touch_connection(&self, id: i64) -> Result<()> {
        let now = chrono::Local::now().to_rfc3339();
        self.conn.execute(
            "UPDATE connections SET last_used_at=?1 WHERE id=?2",
            params![now, id],
        )?;
        Ok(())
    }

    pub fn get_connections_in_folder(&self, folder_id: Option<i64>) -> Result<Vec<ConnProfile>> {
        if let Some(fid) = folder_id {
            let mut stmt = self.conn.prepare("SELECT id, name, hosts, timeout_ms, auth_scheme, auth_credential, category, folder_id, position, created_at, last_used_at FROM connections WHERE folder_id=?1 ORDER BY position, name")?;
            let rows = stmt.query_map(params![fid], Self::map_row)?;
            Ok(rows.filter_map(|r| r.ok()).collect())
        } else {
            let mut stmt = self.conn.prepare("SELECT id, name, hosts, timeout_ms, auth_scheme, auth_credential, category, folder_id, position, created_at, last_used_at FROM connections WHERE folder_id IS NULL ORDER BY position, name")?;
            let rows = stmt.query_map([], Self::map_row)?;
            Ok(rows.filter_map(|r| r.ok()).collect())
        }
    }

    pub fn create_folder(&self, name: &str, parent_id: Option<i64>) -> Result<i64> {
        self.conn.execute("INSERT INTO folders (name, parent_id) VALUES (?1, ?2)", params![name, parent_id])?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn delete_folder(&self, id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM folders WHERE id=?1", params![id])?;
        Ok(())
    }

    pub fn rename_folder(&self, id: i64, name: &str) -> Result<()> {
        self.conn.execute("UPDATE folders SET name=?1 WHERE id=?2", params![name, id])?;
        Ok(())
    }

    pub fn get_subfolders(&self, parent_id: Option<i64>) -> Result<Vec<Folder>> {
        if let Some(pid) = parent_id {
            let mut stmt = self.conn.prepare("SELECT id, name, parent_id, position FROM folders WHERE parent_id=?1 ORDER BY position, name")?;
            let rows = stmt.query_map(params![pid], |row| Ok(Folder { id: row.get(0)?, name: row.get(1)?, parent_id: row.get(2)?, position: row.get(3)? }))?;
            Ok(rows.filter_map(|r| r.ok()).collect())
        } else {
            let mut stmt = self.conn.prepare("SELECT id, name, parent_id, position FROM folders WHERE parent_id IS NULL ORDER BY position, name")?;
            let rows = stmt.query_map([], |row| Ok(Folder { id: row.get(0)?, name: row.get(1)?, parent_id: row.get(2)?, position: row.get(3)? }))?;
            Ok(rows.filter_map(|r| r.ok()).collect())
        }
    }

    pub fn reorder_folder(&self, id: i64, before_id: Option<i64>, parent_id: Option<i64>) -> Result<()> {
        let siblings = self.get_subfolders(parent_id)?;
        let new_pos = if let Some(bid) = before_id {
            let before = siblings.iter().find(|f| f.id == bid);
            let after_prev = siblings.iter().filter(|f| f.id < bid).last();
            match (before, after_prev) {
                (Some(b), Some(p)) => (b.position + p.position) / 2,
                (Some(b), None) => b.position / 2,
                _ => siblings.last().map_or(1024, |f| f.position + 1024),
            }
        } else {
            siblings.last().map_or(1024, |f| f.position + 1024)
        };
        self.conn.execute("UPDATE folders SET position=?1, parent_id=?2 WHERE id=?3", params![new_pos, parent_id, id])?;
        Ok(())
    }

    pub fn reorder_connection(&self, id: i64, before_id: Option<i64>, folder_id: Option<i64>) -> Result<()> {
        let siblings = self.get_connections_in_folder(folder_id)?;
        let new_pos = if let Some(bid) = before_id {
            let before = siblings.iter().find(|c| c.id == bid);
            let after_prev = siblings.iter().filter(|c| c.id < bid).last();
            match (before, after_prev) {
                (Some(b), Some(p)) => (b.position + p.position) / 2,
                (Some(b), None) => b.position / 2,
                _ => siblings.last().map_or(1024, |c| c.position + 1024),
            }
        } else {
            siblings.last().map_or(1024, |c| c.position + 1024)
        };
        self.conn.execute("UPDATE connections SET position=?1, folder_id=?2 WHERE id=?3", params![new_pos, folder_id, id])?;
        Ok(())
    }

    fn map_row(row: &rusqlite::Row) -> rusqlite::Result<ConnProfile> {
        Ok(ConnProfile {
            id: row.get(0)?,
            name: row.get(1)?,
            hosts: row.get(2)?,
            timeout_ms: row.get(3)?,
            auth_scheme: row.get(4)?,
            auth_credential: row.get(5)?,
            category: row.get(6)?,
            folder_id: row.get(7)?,
            position: row.get(8)?,
            created_at: row.get(9)?,
            last_used_at: row.get(10)?,
        })
    }
}
