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
}


