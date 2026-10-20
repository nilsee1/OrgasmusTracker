//! Inline keyboard generators and callback constants for the bot.

use std::collections::BTreeSet;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup};
use crate::models::Entry;

pub const TYPE_SIMPLE: &str = "TYPE_SIMPLE";
pub const TYPE_DETAILED: &str = "TYPE_DETAILED";

pub const TIME_NOW: &str = "TIME_NOW";

pub const COMPANY_ALONE: &str = "COMPANY_ALONE";
pub const COMPANY_PARTNER: &str = "COMPANY_PARTNER";

pub const LOCATION_NILS: &str = "LOC_NILS";
pub const LOCATION_MIKA: &str = "LOC_MIKA";

pub const QUALITY_PREFIX: &str = "QUALITY_";

pub const ACTION_FINISH: &str = "ACTION_FINISH";
pub const ACTION_CANCEL: &str = "ACTION_CANCEL";

pub const LIST_DELETE_MODE: &str = "LIST_DEL_MODE";
pub const LIST_TOGGLE_PREFIX: &str = "LIST_TOGGLE_";
pub const LIST_DELETE_CONFIRM: &str = "LIST_DEL_CONFIRM";
pub const LIST_DELETE_EXECUTE: &str = "LIST_DEL_EXEC";
pub const LIST_DELETE_CANCEL: &str = "LIST_DEL_CANCEL";

pub fn get_choose_type_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![InlineKeyboardButton::callback("⚡ Einfach (Jetzt)", TYPE_SIMPLE)],
        vec![InlineKeyboardButton::callback("📝 Ausführlich", TYPE_DETAILED)],
        vec![InlineKeyboardButton::callback("❌ Abbruch", ACTION_CANCEL)],
    ])
}

pub fn get_datetime_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![InlineKeyboardButton::callback("🕒 Jetzt (Aktuelle Zeit)", TIME_NOW)],
        vec![
            InlineKeyboardButton::callback("✅ Fertigstellen", ACTION_FINISH),
            InlineKeyboardButton::callback("❌ Abbruch", ACTION_CANCEL),
        ],
    ])
}

pub fn get_company_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::callback("👤 Alleine", COMPANY_ALONE),
            InlineKeyboardButton::callback("👥 Mit anderer Person", COMPANY_PARTNER),
        ],
        vec![
            InlineKeyboardButton::callback("✅ Fertigstellen", ACTION_FINISH),
            InlineKeyboardButton::callback("❌ Abbruch", ACTION_CANCEL),
        ],
    ])
}

pub fn get_partner_name_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback("✅ Fertigstellen", ACTION_FINISH),
        InlineKeyboardButton::callback("❌ Abbruch", ACTION_CANCEL),
    ]])
}

pub fn get_location_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::callback("Nils", LOCATION_NILS),
            InlineKeyboardButton::callback("Mika", LOCATION_MIKA),
        ],
        vec![
            InlineKeyboardButton::callback("✅ Fertigstellen", ACTION_FINISH),
            InlineKeyboardButton::callback("❌ Abbruch", ACTION_CANCEL),
        ],
    ])
}

pub fn get_quality_keyboard() -> InlineKeyboardMarkup {
    let row1 = (1..=5)
        .map(|i| InlineKeyboardButton::callback(format!("⭐ {}", i), format!("{}{}", QUALITY_PREFIX, i)))
        .collect::<Vec<_>>();

    let row2 = (6..=10)
        .map(|i| InlineKeyboardButton::callback(format!("⭐ {}", i), format!("{}{}", QUALITY_PREFIX, i)))
        .collect::<Vec<_>>();

    let row3 = vec![
        InlineKeyboardButton::callback("✅ Fertigstellen", ACTION_FINISH),
        InlineKeyboardButton::callback("❌ Abbruch", ACTION_CANCEL),
    ];

    InlineKeyboardMarkup::new(vec![row1, row2, row3])
}

pub fn get_description_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback("✅ Fertigstellen", ACTION_FINISH),
        InlineKeyboardButton::callback("❌ Abbruch", ACTION_CANCEL),
    ]])
}

pub fn get_liste_main_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![InlineKeyboardButton::callback(
        "🗑️ Einträge löschen",
        LIST_DELETE_MODE,
    )]])
}

pub fn get_liste_delete_select_keyboard(
    entries: &[Entry],
    selected_ids: &BTreeSet<i64>,
) -> InlineKeyboardMarkup {
    let mut buttons = Vec::new();

    for (idx, entry) in entries.iter().enumerate() {
        let is_selected = selected_ids.contains(&entry.id);
        let icon = if is_selected { "☑️" } else { "⬜" };
        let text = format!("{} {}", icon, idx + 1);
        buttons.push(InlineKeyboardButton::callback(
            text,
            format!("{}{}", LIST_TOGGLE_PREFIX, entry.id),
        ));
    }

    let mut rows: Vec<Vec<InlineKeyboardButton>> = buttons
        .chunks(5)
        .map(|chunk| chunk.to_vec())
        .collect();

    if !selected_ids.is_empty() {
        rows.push(vec![InlineKeyboardButton::callback(
            format!("🗑️ Auswahl löschen ({})", selected_ids.len()),
            LIST_DELETE_CONFIRM,
        )]);
    }

    rows.push(vec![InlineKeyboardButton::callback(
        "❌ Abbrechen",
        LIST_DELETE_CANCEL,
    )]);

    InlineKeyboardMarkup::new(rows)
}

pub fn get_liste_delete_confirm_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![InlineKeyboardButton::callback(
            "✅ Ja, endgültig löschen",
            LIST_DELETE_EXECUTE,
        )],
        vec![
            InlineKeyboardButton::callback("🔙 Zurück zur Auswahl", LIST_DELETE_MODE),
            InlineKeyboardButton::callback("❌ Abbrechen", LIST_DELETE_CANCEL),
        ],
    ])
}

