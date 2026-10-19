//! Data models and domain types for the Orgasmus-Tracker Bot.

use serde::{Deserialize, Serialize};

/// Maximum allowed length for partner name input.
pub const MAX_PARTNER_NAME_LENGTH: usize = 100;

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
    pub quality: Option<i64>,
    pub description: Option<String>,
}

