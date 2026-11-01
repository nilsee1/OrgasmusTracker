//! Data models and domain types for the Orgasmus-Tracker Bot.

use serde::{Deserialize, Serialize};

/// Maximum allowed length for partner name input.
pub const MAX_PARTNER_NAME_LENGTH: usize = 100;

/// Maximum allowed length for location input.
pub const MAX_LOCATION_LENGTH: usize = 100;

/// Maximum allowed length for description notes.
pub const MAX_DESCRIPTION_LENGTH: usize = 300;

/// Database model representing an orgasm tracker entry.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, PartialEq, Eq)]
pub struct Entry {
    pub id: i64,
    pub user_id: i64,
    pub r#type: String, // "simple" or "detailed"
    pub timestamp: String,
    pub company: Option<String>,
    pub partner_name: Option<String>,
    pub location: Option<String>,
    pub quality: Option<i64>,
    pub description: Option<String>,
    pub created_at: String,
}

/// Draft entry collected progressively during the conversation flow.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DraftEntry {
    pub r#type: String,
    pub timestamp: Option<String>,
    pub company: Option<String>,
    pub partner_name: Option<String>,
    pub location: Option<String>,
    pub quality: Option<i64>,
    pub description: Option<String>,
}

/// An item in a ranked top-list (e.g. top location or partner).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RankedStatItem {
    pub name: String,
    pub count: i64,
    pub avg_quality: Option<f64>,
}

/// Distribution of entries across times of day.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct TimeOfDayDistribution {
    pub morning: i64,   // 06:00 - 11:59
    pub afternoon: i64, // 12:00 - 17:59
    pub evening: i64,   // 18:00 - 23:59
    pub night: i64,     // 00:00 - 05:59
}

/// Aggregated user statistics summary.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct StatsSummary {
    pub total_count: i64,
    pub count_last_7_days: i64,
    pub count_last_30_days: i64,
    pub count_current_year: i64,
    pub count_last_365_days: i64,
    pub count_previous_year: i64,
    pub count_alone: i64,
    pub count_partner: i64,
    pub avg_quality: Option<f64>,
    pub top_locations: Vec<RankedStatItem>,
    pub top_partners: Vec<RankedStatItem>,
    pub current_streak_days: i64,
    pub longest_streak_days: i64,
    pub days_since_last: Option<i64>,
    pub longest_gap_days: Option<i64>,
    pub avg_days_between: Option<f64>,
    pub most_active_weekday: Option<(String, i64)>,
    pub time_of_day: TimeOfDayDistribution,
}


