//! SQLite database persistence layer for orgasm tracking entries.

use std::path::Path;
use std::str::FromStr;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Error as SqlxError, SqlitePool};
use thiserror::Error;

use crate::models::Entry;

#[derive(Debug, Error)]
pub enum DbError {
    #[error("SQLx-Fehler: {0}")]
    Sqlx(#[from] SqlxError),

    #[error("Ungültiger Eintragstyp: '{0}'. Erlaubt sind nur 'simple' oder 'detailed'.")]
    InvalidType(String),

    #[error("Ungültige Qualität: {0}. Erlaubt sind Werte von 1 bis 10.")]
    InvalidQuality(i64),

    #[error("I/O-Fehler beim Erstellen des Datenbankverzeichnisses: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Clone, Debug)]
pub struct Database {
    pool: SqlitePool,
}

impl Database {
    /// Create a new Database connection pool and ensure parent directory exists.
    pub async fn connect(db_path: &str) -> Result<Self, DbError> {
        if let Some(parent) = Path::new(db_path).parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                tokio::fs::create_dir_all(parent).await?;
            }
        }

        let options = SqliteConnectOptions::from_str(&format!("sqlite://{}", db_path))?
            .create_if_missing(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await?;

        let db = Self { pool };
        db.init_db().await?;
        Ok(db)
    }

    /// Construct Database from an existing pool (useful for in-memory testing).
    pub fn from_pool(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Access the underlying connection pool.
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Initialize SQLite tables, pragmas, and indices.
    pub async fn init_db(&self) -> Result<(), DbError> {
        // Enable WAL mode and foreign keys
        sqlx::query("PRAGMA journal_mode = WAL;")
            .execute(&self.pool)
            .await?;
        sqlx::query("PRAGMA foreign_keys = ON;")
            .execute(&self.pool)
            .await?;

        // Create entries table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS entries (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL,
                type TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                company TEXT NULL,
                partner_name TEXT NULL,
                quality INTEGER NULL,
                description TEXT NULL,
                created_at TEXT NOT NULL
            );
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Create index for fast user history queries
        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_entries_user_ts 
            ON entries (user_id, timestamp DESC, id DESC);
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Insert a new orgasm tracking entry and return its generated ID.
    pub async fn add_entry(
        &self,
        user_id: i64,
        entry_type: &str,
        timestamp: &str,
        company: Option<&str>,
        partner_name: Option<&str>,
        quality: Option<i64>,
        description: Option<&str>,
        created_at: Option<&str>,
    ) -> Result<i64, DbError> {
        if entry_type != "simple" && entry_type != "detailed" {
            return Err(DbError::InvalidType(entry_type.to_string()));
        }

        if let Some(q) = quality {
            if !(1..=10).contains(&q) {
                return Err(DbError::InvalidQuality(q));
            }
        }

        let default_created_at;
        let created_at_val = match created_at {
            Some(ca) => ca,
            None => {
                default_created_at = chrono::Utc::now().to_rfc3339();
                &default_created_at
            }
        };

        let result = sqlx::query(
            r#"
            INSERT INTO entries (user_id, type, timestamp, company, partner_name, quality, description, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?);
            "#,
        )
        .bind(user_id)
        .bind(entry_type)
        .bind(timestamp)
        .bind(company)
        .bind(partner_name)
        .bind(quality)
        .bind(description)
        .bind(created_at_val)
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    /// Retrieve the most recent entries for a specific user.
    pub async fn get_last_entries(&self, user_id: i64, limit: i64) -> Result<Vec<Entry>, DbError> {
        let entries = sqlx::query_as::<_, Entry>(
            r#"
            SELECT id, user_id, type, timestamp, company, partner_name, quality, description, created_at
            FROM entries
            WHERE user_id = ?
            ORDER BY timestamp DESC, id DESC
            LIMIT ?;
            "#,
        )
        .bind(user_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(entries)
    }

    /// Fetch a single entry by its ID and user ID.
    pub async fn get_entry_by_id(&self, entry_id: i64, user_id: i64) -> Result<Option<Entry>, DbError> {
        let entry = sqlx::query_as::<_, Entry>(
            r#"
            SELECT id, user_id, type, timestamp, company, partner_name, quality, description, created_at
            FROM entries
            WHERE id = ? AND user_id = ?;
            "#,
        )
        .bind(entry_id)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(entry)
    }

    /// Delete a single entry by its ID and user ID. Returns true if deleted.
    pub async fn delete_entry(&self, entry_id: i64, user_id: i64) -> Result<bool, DbError> {
        let result = sqlx::query(
            r#"
            DELETE FROM entries
            WHERE id = ? AND user_id = ?;
            "#,
        )
        .bind(entry_id)
        .bind(user_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Delete multiple entries belonging to user_id in a single query. Returns count of deleted entries.
    pub async fn delete_entries(&self, entry_ids: &[i64], user_id: i64) -> Result<u64, DbError> {
        if entry_ids.is_empty() {
            return Ok(0);
        }

        let placeholders = entry_ids.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
        let query_str = format!(
            "DELETE FROM entries WHERE user_id = ? AND id IN ({});",
            placeholders
        );

        let mut query = sqlx::query(&query_str).bind(user_id);
        for id in entry_ids {
            query = query.bind(id);
        }

        let result = query.execute(&self.pool).await?;
        Ok(result.rows_affected())
    }
}
