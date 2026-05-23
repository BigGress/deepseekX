use rusqlite::{Connection, Result as SqliteResult};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

pub struct Database {
    pub conn: Mutex<Connection>,
}

impl Database {
    pub fn new(app_data_dir: PathBuf) -> SqliteResult<Self> {
        fs::create_dir_all(&app_data_dir)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

        let db_path = app_data_dir.join("deepseekx.db");
        let conn = Connection::open(&db_path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;

        let db = Database {
            conn: Mutex::new(conn),
        };
        db.init_tables()?;
        Ok(db)
    }

    fn init_tables(&self) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS projects (
                id          TEXT PRIMARY KEY,
                name        TEXT NOT NULL,
                description TEXT NOT NULL DEFAULT '',
                root_path   TEXT NOT NULL,
                instructions TEXT NOT NULL DEFAULT '',
                model       TEXT NOT NULL DEFAULT 'deepseek',
                pinned_files TEXT NOT NULL DEFAULT '',
                skills      TEXT NOT NULL DEFAULT '[]',
                mcp_servers TEXT NOT NULL DEFAULT '[]',
                created_at  INTEGER NOT NULL,
                updated_at  INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS conversations (
                id          TEXT PRIMARY KEY,
                project_id  TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                title       TEXT NOT NULL DEFAULT '新对话',
                created_at  INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS messages (
                id              TEXT PRIMARY KEY,
                conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
                role            TEXT NOT NULL,
                content         TEXT NOT NULL,
                timestamp       INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_conv_project ON conversations(project_id);
            CREATE INDEX IF NOT EXISTS idx_msg_conv ON messages(conversation_id);

            CREATE TABLE IF NOT EXISTS settings (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL DEFAULT ''
            );
            ",
        )?;

        // 兼容已有数据库：检查 pinned_files 列是否存在，不存在则添加
        let has_column: bool = conn
            .prepare("SELECT COUNT(*) FROM pragma_table_info('projects') WHERE name='pinned_files'")?
            .query_row([], |row| row.get::<_, i64>(0))
            .map(|n| n > 0)?;
        if !has_column {
            conn.execute_batch(
                "ALTER TABLE projects ADD COLUMN pinned_files TEXT NOT NULL DEFAULT '';",
            )?;
        }

        // 兼容已有数据库：检查 skills / mcp_servers 列是否存在
        for col in &["skills", "mcp_servers"] {
            let has: bool = conn
                .prepare(&format!(
                    "SELECT COUNT(*) FROM pragma_table_info('projects') WHERE name='{}'",
                    col
                ))?
                .query_row([], |row| row.get::<_, i64>(0))
                .map(|n| n > 0)?;
            if !has {
                conn.execute_batch(&format!(
                    "ALTER TABLE projects ADD COLUMN {} TEXT NOT NULL DEFAULT '[]';",
                    col
                ))?;
            }
        }

        Ok(())
    }

    // ---------- Project CRUD ----------

    pub fn create_project(
        &self,
        id: &str,
        name: &str,
        description: &str,
        root_path: &str,
        instructions: &str,
        model: &str,
        pinned_files: &str,
        skills: &str,
        mcp_servers: &str,
    ) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().timestamp_millis();
        conn.execute(
            "INSERT INTO projects (id, name, description, root_path, instructions, model, pinned_files, skills, mcp_servers, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            rusqlite::params![id, name, description, root_path, instructions, model, pinned_files, skills, mcp_servers, now, now],
        )?;
        Ok(())
    }

    pub fn list_projects(&self) -> SqliteResult<Vec<ProjectRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, description, root_path, instructions, model, pinned_files, skills, mcp_servers, created_at, updated_at
             FROM projects ORDER BY updated_at DESC",
        )?;
        let rows = stmt
            .query_map([], |row| {
                Ok(ProjectRow {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    root_path: row.get(3)?,
                    instructions: row.get(4)?,
                    model: row.get(5)?,
                    pinned_files: row.get(6)?,
                    skills: row.get(7)?,
                    mcp_servers: row.get(8)?,
                    created_at: row.get(9)?,
                    updated_at: row.get(10)?,
                })
            })?
            .collect::<SqliteResult<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn get_project(&self, id: &str) -> SqliteResult<Option<ProjectRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, description, root_path, instructions, model, pinned_files, skills, mcp_servers, created_at, updated_at
             FROM projects WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(rusqlite::params![id], |row| {
            Ok(ProjectRow {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                root_path: row.get(3)?,
                instructions: row.get(4)?,
                model: row.get(5)?,
                pinned_files: row.get(6)?,
                skills: row.get(7)?,
                mcp_servers: row.get(8)?,
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
            })
        })?;
        Ok(rows.next().transpose()?)
    }

    pub fn update_project(
        &self,
        id: &str,
        name: &str,
        description: &str,
        instructions: &str,
        model: &str,
        pinned_files: &str,
        skills: &str,
        mcp_servers: &str,
    ) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().timestamp_millis();
        conn.execute(
            "UPDATE projects SET name=?1, description=?2, instructions=?3, model=?4, pinned_files=?5, skills=?6, mcp_servers=?7, updated_at=?8 WHERE id=?9",
            rusqlite::params![name, description, instructions, model, pinned_files, skills, mcp_servers, now, id],
        )?;
        Ok(())
    }

    pub fn delete_project(&self, id: &str) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM projects WHERE id = ?1", rusqlite::params![id])?;
        Ok(())
    }

    // ---------- Conversation CRUD ----------

    pub fn create_conversation(
        &self,
        id: &str,
        project_id: &str,
        title: &str,
    ) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().timestamp_millis();
        conn.execute(
            "INSERT INTO conversations (id, project_id, title, created_at) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![id, project_id, title, now],
        )?;
        Ok(())
    }

    pub fn list_conversations(&self, project_id: &str) -> SqliteResult<Vec<ConversationRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, project_id, title, created_at FROM conversations WHERE project_id = ?1 ORDER BY created_at DESC",
        )?;
        let rows = stmt
            .query_map(rusqlite::params![project_id], |row| {
                Ok(ConversationRow {
                    id: row.get(0)?,
                    project_id: row.get(1)?,
                    title: row.get(2)?,
                    created_at: row.get(3)?,
                })
            })?
            .collect::<SqliteResult<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn delete_conversation(&self, id: &str) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM conversations WHERE id = ?1", rusqlite::params![id])?;
        Ok(())
    }

    // ---------- Message CRUD ----------

    pub fn save_message(
        &self,
        id: &str,
        conversation_id: &str,
        role: &str,
        content: &str,
        timestamp: i64,
    ) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO messages (id, conversation_id, role, content, timestamp) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![id, conversation_id, role, content, timestamp],
        )?;
        Ok(())
    }

    pub fn list_messages(&self, conversation_id: &str) -> SqliteResult<Vec<MessageRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, conversation_id, role, content, timestamp FROM messages WHERE conversation_id = ?1 ORDER BY timestamp ASC",
        )?;
        let rows = stmt
            .query_map(rusqlite::params![conversation_id], |row| {
                Ok(MessageRow {
                    id: row.get(0)?,
                    conversation_id: row.get(1)?,
                    role: row.get(2)?,
                    content: row.get(3)?,
                    timestamp: row.get(4)?,
                })
            })?
            .collect::<SqliteResult<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn delete_messages_by_conversation(&self, conversation_id: &str) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM messages WHERE conversation_id = ?1",
            rusqlite::params![conversation_id],
        )?;
        Ok(())
    }

    // ---------- Settings ----------

    pub fn get_setting(&self, key: &str) -> SqliteResult<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT value FROM settings WHERE key = ?1")?;
        let mut rows = stmt.query_map(rusqlite::params![key], |row| row.get(0))?;
        Ok(rows.next().transpose()?)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            rusqlite::params![key, value],
        )?;
        Ok(())
    }
}

// ---------- Row types ----------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProjectRow {
    pub id: String,
    pub name: String,
    pub description: String,
    pub root_path: String,
    pub instructions: String,
    pub model: String,
    pub pinned_files: String,
    pub skills: String,
    pub mcp_servers: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConversationRow {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MessageRow {
    pub id: String,
    pub conversation_id: String,
    pub role: String,
    pub content: String,
    pub timestamp: i64,
}
