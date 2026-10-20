use std::collections::BTreeSet;
use orgasmus_tracker::config::Config;
use orgasmus_tracker::keyboards::*;
use orgasmus_tracker::models::{
    Entry, MAX_DESCRIPTION_LENGTH, MAX_LOCATION_LENGTH, MAX_PARTNER_NAME_LENGTH,
};

#[test]
fn test_parse_allowed_user_ids() {
    let empty = Config::parse_allowed_user_ids("");
    assert!(empty.is_empty());

    let valid = Config::parse_allowed_user_ids("123, 456, 789");
    assert_eq!(valid.len(), 3);
    assert!(valid.contains(&123));
    assert!(valid.contains(&456));
    assert!(valid.contains(&789));

    let with_invalid = Config::parse_allowed_user_ids("123, abc, 456, -999, , ");
    assert_eq!(with_invalid.len(), 3);
    assert!(with_invalid.contains(&123));
    assert!(with_invalid.contains(&456));
    assert!(with_invalid.contains(&-999));
}

#[test]
fn test_is_user_allowed() {
    let allowed = Config::parse_allowed_user_ids("12345");
    let config = Config {
        bot_token: "test".to_string(),
        allowed_user_ids: allowed,
        db_path: ":memory:".to_string(),
        timezone: chrono_tz::Europe::Berlin,
    };

    assert!(config.is_user_allowed(12345));
    assert!(!config.is_user_allowed(99999));
}

#[test]
fn test_validation_constants() {
    assert_eq!(MAX_PARTNER_NAME_LENGTH, 100);
    assert_eq!(MAX_LOCATION_LENGTH, 100);
    assert_eq!(MAX_DESCRIPTION_LENGTH, 300);
}

#[test]
fn test_keyboards_structure() {
    // Choose type keyboard
    let choose_kb = get_choose_type_keyboard();
    assert_eq!(choose_kb.inline_keyboard.len(), 3);
    assert_eq!(
        choose_kb.inline_keyboard[0][0].text,
        "⚡ Einfach (Jetzt)"
    );

    // DateTime keyboard
    let dt_kb = get_datetime_keyboard();
    assert_eq!(dt_kb.inline_keyboard.len(), 2);

    // Company keyboard
    let company_kb = get_company_keyboard();
    assert_eq!(company_kb.inline_keyboard.len(), 2);
    assert_eq!(company_kb.inline_keyboard[0].len(), 2);

    // Location keyboard
    let location_kb = get_location_keyboard();
    assert_eq!(location_kb.inline_keyboard.len(), 2);
    assert_eq!(location_kb.inline_keyboard[0].len(), 2);
    assert_eq!(location_kb.inline_keyboard[0][0].text, "Nils");
    assert_eq!(location_kb.inline_keyboard[0][1].text, "Mika");
    assert_eq!(location_kb.inline_keyboard[1].len(), 2);
    assert_eq!(location_kb.inline_keyboard[1][0].text, "✅ Fertigstellen");
    assert_eq!(location_kb.inline_keyboard[1][1].text, "❌ Abbruch");

    // Quality keyboard (10 items in 2 rows + actions row)
    let quality_kb = get_quality_keyboard();
    assert_eq!(quality_kb.inline_keyboard.len(), 3);
    assert_eq!(quality_kb.inline_keyboard[0].len(), 5);
    assert_eq!(quality_kb.inline_keyboard[1].len(), 5);

    // Delete mode keyboard
    let entries = vec![
        Entry {
            id: 10,
            user_id: 1,
            r#type: "simple".to_string(),
            timestamp: "2026-10-19 12:00".to_string(),
            company: None,
            partner_name: None,
            location: None,
            quality: None,
            description: None,
            created_at: "now".to_string(),
        },
        Entry {
            id: 20,
            user_id: 1,
            r#type: "simple".to_string(),
            timestamp: "2026-10-19 13:00".to_string(),
            company: None,
            partner_name: None,
            location: None,
            quality: None,
            description: None,
            created_at: "now".to_string(),
        },
    ];

    let mut selected = BTreeSet::new();
    let select_kb_empty = get_liste_delete_select_keyboard(&entries, &selected);
    // 2 entries in row 1, 1 cancel row -> 2 rows
    assert_eq!(select_kb_empty.inline_keyboard.len(), 2);
    assert!(select_kb_empty.inline_keyboard[0][0].text.contains("⬜ 1"));

    selected.insert(10);
    let select_kb_selected = get_liste_delete_select_keyboard(&entries, &selected);
    // 2 entries in row 1, 1 confirm row, 1 cancel row -> 3 rows
    assert_eq!(select_kb_selected.inline_keyboard.len(), 3);
    assert!(select_kb_selected.inline_keyboard[0][0].text.contains("☑️ 1"));
    assert!(select_kb_selected.inline_keyboard[1][0].text.contains("🗑️ Auswahl löschen (1)"));

    // Confirm dialog keyboard
    let confirm_kb = get_liste_delete_confirm_keyboard();
    assert_eq!(confirm_kb.inline_keyboard.len(), 2);
}

