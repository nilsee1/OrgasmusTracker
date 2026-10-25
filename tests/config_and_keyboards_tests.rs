use std::collections::BTreeSet;
use orgasmus_tracker::config::Config;
use orgasmus_tracker::handlers::get_location_prompt_text;
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
        location_presets: vec![],
    };

    assert!(config.is_user_allowed(12345));
    assert!(!config.is_user_allowed(99999));
}

#[test]
fn test_parse_location_presets() {
    let empty = Config::parse_location_presets("");
    assert!(empty.is_empty());

    let whitespace = Config::parse_location_presets("   ,   , ");
    assert!(whitespace.is_empty());

    let valid = Config::parse_location_presets(" Home , Hotel , Work ");
    assert_eq!(valid, vec!["Home", "Hotel", "Work"]);

    let with_empty_and_spaces = Config::parse_location_presets("Zuhause, , Hotel, ,");
    assert_eq!(with_empty_and_spaces, vec!["Zuhause", "Hotel"]);

    let too_long_entry = "A".repeat(MAX_LOCATION_LENGTH + 1);
    let with_too_long = format!("Valid, {}, AlsoValid", too_long_entry);
    let filtered = Config::parse_location_presets(&with_too_long);
    assert_eq!(filtered, vec!["Valid", "AlsoValid"]);
}

#[test]
fn test_location_prompt_text() {
    let empty: Vec<String> = vec![];
    assert_eq!(
        get_location_prompt_text(&empty),
        "📍 Wo fand es statt? Bitte gib den Ort als Text ein (oder klicke auf Fertigstellen):"
    );

    let populated = vec!["Home".to_string()];
    assert_eq!(
        get_location_prompt_text(&populated),
        "📍 Wo fand es statt? Wähle einen Button oder gib den Ort als Text ein:"
    );
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

    // Location keyboard - empty presets (default)
    let location_kb_empty = get_location_keyboard(&[]);
    assert_eq!(location_kb_empty.inline_keyboard.len(), 1);
    assert_eq!(location_kb_empty.inline_keyboard[0].len(), 2);
    assert_eq!(location_kb_empty.inline_keyboard[0][0].text, "✅ Fertigstellen");
    assert_eq!(location_kb_empty.inline_keyboard[0][1].text, "❌ Abbruch");

    // Location keyboard - populated presets
    let presets = vec!["Home".to_string(), "Hotel".to_string(), "Work".to_string()];
    let location_kb_populated = get_location_keyboard(&presets);
    assert_eq!(location_kb_populated.inline_keyboard.len(), 3); // 2 preset rows + 1 action row
    assert_eq!(location_kb_populated.inline_keyboard[0].len(), 2);
    assert_eq!(location_kb_populated.inline_keyboard[0][0].text, "Home");
    assert_eq!(
        location_kb_populated.inline_keyboard[0][0].kind,
        teloxide::types::InlineKeyboardButtonKind::CallbackData("LOC_IDX_0".to_string())
    );
    assert_eq!(location_kb_populated.inline_keyboard[0][1].text, "Hotel");
    assert_eq!(
        location_kb_populated.inline_keyboard[0][1].kind,
        teloxide::types::InlineKeyboardButtonKind::CallbackData("LOC_IDX_1".to_string())
    );

    assert_eq!(location_kb_populated.inline_keyboard[1].len(), 1);
    assert_eq!(location_kb_populated.inline_keyboard[1][0].text, "Work");
    assert_eq!(
        location_kb_populated.inline_keyboard[1][0].kind,
        teloxide::types::InlineKeyboardButtonKind::CallbackData("LOC_IDX_2".to_string())
    );

    assert_eq!(location_kb_populated.inline_keyboard[2].len(), 2);
    assert_eq!(location_kb_populated.inline_keyboard[2][0].text, "✅ Fertigstellen");
    assert_eq!(location_kb_populated.inline_keyboard[2][1].text, "❌ Abbruch");

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

