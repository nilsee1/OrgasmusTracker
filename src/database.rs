//! SQLite database persistence layer for orgasm tracking entries.

use std::path::Path;
use std::str::FromStr;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Error as SqlxError, Row, SqlitePool};
use thiserror::Error;

use crate::models::{Entry, StatsSummary};

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
    timezone: chrono_tz::Tz,
}

impl Database {
    /// Create a new Database connection pool with default Europe/Berlin timezone.
    pub async fn connect(db_path: &str) -> Result<Self, DbError> {
        Self::connect_with_tz(db_path, chrono_tz::Europe::Berlin).await
    }

    /// Create a new Database connection pool with a specific timezone.
    pub async fn connect_with_tz(db_path: &str, tz: chrono_tz::Tz) -> Result<Self, DbError> {
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

        let db = Self { pool, timezone: tz };
        db.init_db().await?;
        Ok(db)
    }

    /// Construct Database from an existing pool (useful for in-memory testing).
    pub fn from_pool(pool: SqlitePool) -> Self {
        Self {
            pool,
            timezone: chrono_tz::Europe::Berlin,
        }
    }

    /// Update or override the timezone configured for statistics.
    pub fn with_timezone(mut self, tz: chrono_tz::Tz) -> Self {
        self.timezone = tz;
        self
    }

    /// Access the configured timezone.
    pub fn timezone(&self) -> chrono_tz::Tz {
        self.timezone
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
                location TEXT NULL,
                quality INTEGER NULL,
                description TEXT NULL,
                created_at TEXT NOT NULL
            );
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Migration for existing tables: ensure location column exists
        let table_info = sqlx::query("PRAGMA table_info(entries);")
            .fetch_all(&self.pool)
            .await?;
        let has_location = table_info.iter().any(|row| {
            let col_name: String = row.get("name");
            col_name == "location"
        });
        if !has_location {
            sqlx::query("ALTER TABLE entries ADD COLUMN location TEXT;")
                .execute(&self.pool)
                .await?;
        }

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
        location: Option<&str>,
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
            INSERT INTO entries (user_id, type, timestamp, company, partner_name, location, quality, description, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?);
            "#,
        )
        .bind(user_id)
        .bind(entry_type)
        .bind(timestamp)
        .bind(company)
        .bind(partner_name)
        .bind(location)
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
            SELECT id, user_id, type, timestamp, company, partner_name, location, quality, description, created_at
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
            SELECT id, user_id, type, timestamp, company, partner_name, location, quality, description, created_at
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

    /// Retrieve all entries belonging to user_id, ordered chronologically (timestamp ASC, id ASC).
    pub async fn get_all_entries(&self, user_id: i64) -> Result<Vec<Entry>, DbError> {
        let entries = sqlx::query_as::<_, Entry>(
            r#"
            SELECT id, user_id, type, timestamp, company, partner_name, location, quality, description, created_at
            FROM entries
            WHERE user_id = ?
            ORDER BY timestamp ASC, id ASC;
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(entries)
    }

    /// Retrieve aggregated statistics for user_id based on configured timezone.
    pub async fn get_statistics(&self, user_id: i64) -> Result<StatsSummary, DbError> {
        let now = chrono::Utc::now().with_timezone(&self.timezone);
        self.get_statistics_at(user_id, now).await
    }

    /// Retrieve aggregated statistics at a specific reference time (useful for deterministic tests).
    pub async fn get_statistics_at(
        &self,
        user_id: i64,
        now: chrono::DateTime<chrono_tz::Tz>,
    ) -> Result<StatsSummary, DbError> {
        use chrono::Datelike;

        let ts_7_days = (now - chrono::Duration::days(7))
            .format("%Y-%m-%d %H:%M")
            .to_string();
        let ts_30_days = (now - chrono::Duration::days(30))
            .format("%Y-%m-%d %H:%M")
            .to_string();
        let ts_365_days = (now - chrono::Duration::days(365))
            .format("%Y-%m-%d %H:%M")
            .to_string();

        let current_year = now.year();
        let ts_curr_year_start = format!("{:04}-01-01 00:00", current_year);

        let prev_year = current_year - 1;
        let ts_prev_year_start = format!("{:04}-01-01 00:00", prev_year);
        let ts_prev_year_end = format!("{:04}-12-31 23:59:59", prev_year);

        let row = sqlx::query(
            r#"
            SELECT
                CAST(COUNT(*) AS INTEGER) AS total_count,
                CAST(COALESCE(SUM(CASE WHEN timestamp >= ? THEN 1 ELSE 0 END), 0) AS INTEGER) AS count_7_days,
                CAST(COALESCE(SUM(CASE WHEN timestamp >= ? THEN 1 ELSE 0 END), 0) AS INTEGER) AS count_30_days,
                CAST(COALESCE(SUM(CASE WHEN timestamp >= ? THEN 1 ELSE 0 END), 0) AS INTEGER) AS count_current_year,
                CAST(COALESCE(SUM(CASE WHEN timestamp >= ? THEN 1 ELSE 0 END), 0) AS INTEGER) AS count_365_days,
                CAST(COALESCE(SUM(CASE WHEN timestamp >= ? AND timestamp <= ? THEN 1 ELSE 0 END), 0) AS INTEGER) AS count_prev_year,
                CAST(COALESCE(SUM(CASE WHEN company = 'Alleine' THEN 1 ELSE 0 END), 0) AS INTEGER) AS count_alone,
                CAST(COALESCE(SUM(CASE WHEN company = 'Mit anderer Person' THEN 1 ELSE 0 END), 0) AS INTEGER) AS count_partner,
                AVG(quality) AS avg_quality
            FROM entries
            WHERE user_id = ?;
            "#,
        )
        .bind(&ts_7_days)
        .bind(&ts_30_days)
        .bind(&ts_curr_year_start)
        .bind(&ts_365_days)
        .bind(&ts_prev_year_start)
        .bind(&ts_prev_year_end)
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;

        let total_count: i64 = row.try_get("total_count").unwrap_or(0);
        let count_last_7_days: i64 = row.try_get("count_7_days").unwrap_or(0);
        let count_last_30_days: i64 = row.try_get("count_30_days").unwrap_or(0);
        let count_current_year: i64 = row.try_get("count_current_year").unwrap_or(0);
        let count_last_365_days: i64 = row.try_get("count_365_days").unwrap_or(0);
        let count_previous_year: i64 = row.try_get("count_prev_year").unwrap_or(0);
        let count_alone: i64 = row.try_get("count_alone").unwrap_or(0);
        let count_partner: i64 = row.try_get("count_partner").unwrap_or(0);
        let raw_avg: Option<f64> = row.try_get("avg_quality").ok().flatten();

        let avg_quality = raw_avg.map(|q| (q * 10.0).round() / 10.0);

        Ok(StatsSummary {
            total_count,
            count_last_7_days,
            count_last_30_days,
            count_current_year,
            count_last_365_days,
            count_previous_year,
            count_alone,
            count_partner,
            avg_quality,
        })
    }
}
