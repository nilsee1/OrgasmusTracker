//! SQLite database persistence layer for orgasm tracking entries.

use std::path::Path;
use std::str::FromStr;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Error as SqlxError, Row, SqlitePool};
use thiserror::Error;

use crate::models::{Entry, RankedStatItem, StatsSummary, TimeOfDayDistribution};

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

enum TopField {
    Location,
    PartnerName,
}

impl TopField {
    fn column_name(&self) -> &'static str {
        match self {
            Self::Location => "location",
            Self::PartnerName => "partner_name",
        }
    }

    fn extra_filter(&self) -> &'static str {
        match self {
            Self::Location => "",
            Self::PartnerName => " AND (company IS NULL OR company != 'Alleine')",
        }
    }
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

        // Non-destructive indices for per-user top location & partner suggestions
        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_entries_user_location
            ON entries (user_id, location);
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_entries_user_partner
            ON entries (user_id, partner_name);
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

    async fn get_top_field_values(
        &self,
        field: TopField,
        user_id: i64,
        limit: i64,
    ) -> Result<Vec<String>, DbError> {
        if limit <= 0 {
            return Ok(Vec::new());
        }

        let col = field.column_name();
        let extra_filter = field.extra_filter();
        let sql = format!(
            r#"
            SELECT TRIM({col}) AS name
            FROM entries
            WHERE user_id = ?{extra_filter} AND {col} IS NOT NULL AND TRIM({col}) != ''
            GROUP BY TRIM({col})
            ORDER BY COUNT(*) DESC, MAX(timestamp) DESC, MAX(id) DESC, TRIM({col}) ASC
            LIMIT ?;
            "#
        );

        let rows = sqlx::query(&sql)
            .bind(user_id)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?;

        let mut results = Vec::with_capacity(rows.len());
        for row in rows {
            results.push(row.try_get("name")?);
        }
        Ok(results)
    }

    /// Retrieve the most frequently used distinct locations for a specific user.
    pub async fn get_top_locations(
        &self,
        user_id: i64,
        limit: i64,
    ) -> Result<Vec<String>, DbError> {
        self.get_top_field_values(TopField::Location, user_id, limit)
            .await
    }

    /// Retrieve the most frequently logged distinct partner names for a specific user.
    pub async fn get_top_partners(
        &self,
        user_id: i64,
        limit: i64,
    ) -> Result<Vec<String>, DbError> {
        self.get_top_field_values(TopField::PartnerName, user_id, limit)
            .await
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

        let loc_rows = sqlx::query(
            r#"
            SELECT
                TRIM(location) AS name,
                CAST(COUNT(*) AS INTEGER) AS cnt,
                AVG(quality) AS avg_q
            FROM entries
            WHERE user_id = ? AND location IS NOT NULL AND TRIM(location) != ''
            GROUP BY TRIM(location)
            ORDER BY cnt DESC, avg_q DESC, TRIM(location) ASC
            LIMIT 3;
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        let mut top_locations = Vec::with_capacity(loc_rows.len());
        for row in loc_rows {
            let name: String = row.try_get("name")?;
            let count: i64 = row.try_get("cnt")?;
            let avg_q: Option<f64> = row.try_get("avg_q").ok().flatten();
            let avg_quality = avg_q.map(|q| (q * 10.0).round() / 10.0);
            top_locations.push(RankedStatItem {
                name,
                count,
                avg_quality,
            });
        }

        let partner_rows = sqlx::query(
            r#"
            SELECT
                TRIM(partner_name) AS name,
                CAST(COUNT(*) AS INTEGER) AS cnt,
                AVG(quality) AS avg_q
            FROM entries
            WHERE user_id = ? AND company = 'Mit anderer Person' AND partner_name IS NOT NULL AND TRIM(partner_name) != ''
            GROUP BY TRIM(partner_name)
            ORDER BY cnt DESC, avg_q DESC, TRIM(partner_name) ASC
            LIMIT 3;
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        let mut top_partners = Vec::with_capacity(partner_rows.len());
        for row in partner_rows {
            let name: String = row.try_get("name")?;
            let count: i64 = row.try_get("cnt")?;
            let avg_q: Option<f64> = row.try_get("avg_q").ok().flatten();
            let avg_quality = avg_q.map(|q| (q * 10.0).round() / 10.0);
            top_partners.push(RankedStatItem {
                name,
                count,
                avg_quality,
            });
        }

        let ts_rows = sqlx::query(
            r#"
            SELECT timestamp
            FROM entries
            WHERE user_id = ?
            ORDER BY timestamp ASC;
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        let mut parsed_datetimes: Vec<chrono::NaiveDateTime> = Vec::new();
        for row in ts_rows {
            let ts_str: String = row.try_get("timestamp")?;
            if let Some(ndt) = crate::utils::parse_german_datetime(&ts_str) {
                parsed_datetimes.push(ndt);
            } else if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(ts_str.trim()) {
                parsed_datetimes.push(dt.with_timezone(&self.timezone).naive_local());
            }
        }
        parsed_datetimes.sort();

        use chrono::Timelike;
        let mut time_of_day = TimeOfDayDistribution::default();
        for dt in &parsed_datetimes {
            match dt.hour() {
                6..=11 => time_of_day.morning += 1,
                12..=17 => time_of_day.afternoon += 1,
                18..=23 => time_of_day.evening += 1,
                0..=5 => time_of_day.night += 1,
                _ => unreachable!(),
            }
        }

        use chrono::Weekday;
        let most_active_weekday = if parsed_datetimes.is_empty() {
            None
        } else {
            let mut weekday_counts = [0i64; 7]; // Mon=0 .. Sun=6
            for dt in &parsed_datetimes {
                let idx = match dt.weekday() {
                    Weekday::Mon => 0,
                    Weekday::Tue => 1,
                    Weekday::Wed => 2,
                    Weekday::Thu => 3,
                    Weekday::Fri => 4,
                    Weekday::Sat => 5,
                    Weekday::Sun => 6,
                };
                weekday_counts[idx] += 1;
            }

            let german_names = [
                "Montag",
                "Dienstag",
                "Mittwoch",
                "Donnerstag",
                "Freitag",
                "Samstag",
                "Sonntag",
            ];

            let mut best_day = "";
            let mut max_count = 0i64;

            for (name, count) in german_names.iter().zip(weekday_counts.iter()) {
                if *count > max_count {
                    max_count = *count;
                    best_day = name;
                }
            }

            if max_count > 0 {
                Some((best_day.to_string(), max_count))
            } else {
                None
            }
        };

        use chrono::NaiveDate;
        let mut unique_dates: Vec<NaiveDate> = parsed_datetimes.iter().map(|dt| dt.date()).collect();
        unique_dates.sort();
        unique_dates.dedup();

        let days_since_last = unique_dates
            .last()
            .map(|last_date| (now.date_naive() - *last_date).num_days().max(0));

        let longest_streak_days = if unique_dates.is_empty() {
            0
        } else {
            let mut max_streak = 1i64;
            let mut current_run = 1i64;
            for i in 1..unique_dates.len() {
                if (unique_dates[i] - unique_dates[i - 1]).num_days() == 1 {
                    current_run += 1;
                } else {
                    current_run = 1;
                }
                if current_run > max_streak {
                    max_streak = current_run;
                }
            }
            max_streak
        };

        let current_streak_days = if unique_dates.is_empty() {
            0
        } else {
            let today = now.date_naive();
            let yesterday = today.pred_opt();
            let last_date = *unique_dates.last().unwrap();

            let is_active = last_date == today || Some(last_date) == yesterday;
            if !is_active {
                0
            } else {
                let mut streak = 1i64;
                for i in (1..unique_dates.len()).rev() {
                    if (unique_dates[i] - unique_dates[i - 1]).num_days() == 1 {
                        streak += 1;
                    } else {
                        break;
                    }
                }
                streak
            }
        };

        let (avg_days_between, longest_gap_days) = if parsed_datetimes.len() >= 2 {
            let n = parsed_datetimes.len();
            let first = parsed_datetimes.first().unwrap();
            let last = parsed_datetimes.last().unwrap();
            let total_seconds = (*last - *first).num_seconds() as f64;
            let avg_days = (total_seconds / (n - 1) as f64) / 86400.0;
            let rounded_avg = (avg_days * 10.0).round() / 10.0;

            let mut max_gap = 0i64;
            for i in 1..n {
                let gap = (parsed_datetimes[i].date() - parsed_datetimes[i - 1].date()).num_days();
                if gap > max_gap {
                    max_gap = gap;
                }
            }

            (Some(rounded_avg), Some(max_gap))
        } else {
            (None, None)
        };

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
            top_locations,
            top_partners,
            current_streak_days,
            longest_streak_days,
            days_since_last,
            longest_gap_days,
            avg_days_between,
            most_active_weekday,
            time_of_day,
        })
    }
}
