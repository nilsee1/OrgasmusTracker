//! Configuration management for the Telegram Bot.

use std::collections::HashSet;
use std::env;
use std::str::FromStr;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("TELEGRAM_BOT_TOKEN ist nicht gesetzt oder leer. Bitte in der .env angeben.")]
    MissingBotToken,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub bot_token: String,
    pub allowed_user_ids: HashSet<i64>,
    pub db_path: String,
    pub timezone: chrono_tz::Tz,
}

impl Config {
    /// Load configuration from environment variables and optional .env file.
    pub fn from_env() -> Result<Self, ConfigError> {
        let _ = dotenvy::dotenv();

        let bot_token = env::var("TELEGRAM_BOT_TOKEN")
            .unwrap_or_default()
            .trim()
            .to_string();

        if bot_token.is_empty() {
            return Err(ConfigError::MissingBotToken);
        }

        let raw_allowed = env::var("ALLOWED_USER_IDS").unwrap_or_default();
        let allowed_user_ids = Self::parse_allowed_user_ids(&raw_allowed);

        let db_path = env::var("DB_PATH")
            .unwrap_or_else(|_| "/app/data/tracker.db".to_string())
            .trim()
            .to_string();

        let tz_str = env::var("TZ").unwrap_or_else(|_| "Europe/Berlin".to_string());
        let timezone = chrono_tz::Tz::from_str(tz_str.trim()).unwrap_or_else(|_| {
            log::warn!(
                "Konnte Zeitzone '{}' nicht parsen. Fallback auf 'Europe/Berlin'.",
                tz_str
            );
            chrono_tz::Europe::Berlin
        });

        Ok(Self {
            bot_token,
            allowed_user_ids,
            db_path,
            timezone,
        })
    }

    /// Parse comma-separated user IDs into a HashSet of integers.
    pub fn parse_allowed_user_ids(raw_ids: &str) -> HashSet<i64> {
        let mut allowed = HashSet::new();
        for item in raw_ids.split(',') {
            let cleaned = item.trim();
            if cleaned.is_empty() {
                continue;
            }
            match cleaned.parse::<i64>() {
                Ok(id) => {
                    allowed.insert(id);
                }
                Err(_) => {
                    log::warn!(
                        "Ungültige User-ID in ALLOWED_USER_IDS ignoriert: '{}'",
                        cleaned
                    );
                }
            }
        }
        allowed
    }

    /// Check if the given Telegram user ID is authorized.
    pub fn is_user_allowed(&self, user_id: i64) -> bool {
        self.allowed_user_ids.contains(&user_id)
    }
}

