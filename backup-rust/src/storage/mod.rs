use anyhow::Result;
use sqlx::{sqlite::SqlitePool, Row};
use std::path::Path;
use tracing::info;

#[derive(Clone)]
pub struct Storage {
    pool: SqlitePool,
}

impl Storage {
    pub async fn new(database_url: &str) -> Result<Self> {
        // Create database file if it doesn't exist
        if !database_url.starts_with(":memory:") {
            let path = database_url.strip_prefix("sqlite://").unwrap_or(database_url);
            if let Some(parent) = Path::new(path).parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
        }

        let pool = SqlitePool::connect(database_url).await?;
        
        Ok(Self { pool })
    }

    pub async fn run_migrations(&self) -> Result<()> {
        info!("Running database migrations");
        
        // Create tables
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS backup_sessions (
                id TEXT PRIMARY KEY,
                data TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS backup_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                profile TEXT,
                status TEXT NOT NULL,
                started_at DATETIME,
                completed_at DATETIME,
                total_size BIGINT,
                files_count BIGINT,
                directories_count INTEGER,
                errors_count INTEGER,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS backup_logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                level TEXT NOT NULL,
                message TEXT NOT NULL,
                directory TEXT,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_logs_session ON backup_logs(session_id);
            CREATE INDEX IF NOT EXISTS idx_logs_level ON backup_logs(level);
            CREATE INDEX IF NOT EXISTS idx_history_session ON backup_history(session_id);
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn save_session(&self, session: &super::backup::BackupSession) -> Result<()> {
        let data = serde_json::to_string(session)?;
        
        sqlx::query(
            r#"
            INSERT INTO backup_sessions (id, data, updated_at)
            VALUES (?1, ?2, CURRENT_TIMESTAMP)
            ON CONFLICT(id) DO UPDATE SET
                data = excluded.data,
                updated_at = CURRENT_TIMESTAMP
            "#,
        )
        .bind(&session.id)
        .bind(&data)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn load_session(&self, session_id: &str) -> Result<Option<super::backup::BackupSession>> {
        let row = sqlx::query(
            "SELECT data FROM backup_sessions WHERE id = ?1"
        )
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            let data: String = row.get("data");
            let session = serde_json::from_str(&data)?;
            Ok(Some(session))
        } else {
            Ok(None)
        }
    }

    pub async fn get_latest_session(&self) -> Result<Option<super::backup::BackupSession>> {
        let row = sqlx::query(
            "SELECT data FROM backup_sessions ORDER BY updated_at DESC LIMIT 1"
        )
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            let data: String = row.get("data");
            let session = serde_json::from_str(&data)?;
            Ok(Some(session))
        } else {
            Ok(None)
        }
    }

    pub async fn add_log(&self, session_id: &str, level: &str, message: &str, directory: Option<&str>) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO backup_logs (session_id, level, message, directory)
            VALUES (?1, ?2, ?3, ?4)
            "#,
        )
        .bind(session_id)
        .bind(level)
        .bind(message)
        .bind(directory)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_logs(&self, session_id: &str, level: Option<&str>, limit: i32) -> Result<Vec<LogEntry>> {
        let query = if let Some(level) = level {
            sqlx::query_as::<_, LogEntry>(
                r#"
                SELECT level, message, directory, created_at
                FROM backup_logs
                WHERE session_id = ?1 AND level = ?2
                ORDER BY id DESC
                LIMIT ?3
                "#,
            )
            .bind(session_id)
            .bind(level)
            .bind(limit)
        } else {
            sqlx::query_as::<_, LogEntry>(
                r#"
                SELECT level, message, directory, created_at
                FROM backup_logs
                WHERE session_id = ?1
                ORDER BY id DESC
                LIMIT ?2
                "#,
            )
            .bind(session_id)
            .bind(limit)
        };

        let logs = query.fetch_all(&self.pool).await?;
        Ok(logs)
    }
}

#[derive(Debug, sqlx::FromRow)]
pub struct LogEntry {
    pub level: String,
    pub message: String,
    pub directory: Option<String>,
    #[sqlx(rename = "created_at")]
    pub created_at: String,
}