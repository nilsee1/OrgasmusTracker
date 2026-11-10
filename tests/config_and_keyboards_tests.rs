use std::collections::BTreeSet;
use std::sync::Mutex;
use orgasmus_tracker::config::{get_commit_sha, resolve_commit_sha, Config};
use orgasmus_tracker::handlers::{get_location_prompt_text, get_partner_name_prompt_text};
use orgasmus_tracker::keyboards::*;
use orgasmus_tracker::models::{
    Entry, MAX_DESCRIPTION_LENGTH, MAX_LOCATION_LENGTH, MAX_PARTNER_NAME_LENGTH,
};
use teloxide::types::InlineKeyboardButtonKind;

static ENV_MUTEX: Mutex<()> = Mutex::new(());

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
fn test_config_from_env_without_location_presets() {
    let _guard = ENV_MUTEX.lock().unwrap();
    let prev_token = std::env::var("TELEGRAM_BOT_TOKEN").ok();
    let prev_allowed = std::env::var("ALLOWED_USER_IDS").ok();
    let prev_presets = std::env::var("LOCATION_PRESETS").ok();

    std::env::set_var("TELEGRAM_BOT_TOKEN", "123456:TEST_TOKEN");
    std::env::set_var("ALLOWED_USER_IDS", "111,222");
    std::env::remove_var("LOCATION_PRESETS");

    let cfg = Config::from_env().expect("Config should load without LOCATION_PRESETS");
    assert_eq!(cfg.bot_token, "123456:TEST_TOKEN");
    assert!(cfg.is_user_allowed(111));
    assert!(cfg.is_user_allowed(222));

    // Restore original environment variables
    match prev_token {
        Some(v) => std::env::set_var("TELEGRAM_BOT_TOKEN", v),
        None => std::env::remove_var("TELEGRAM_BOT_TOKEN"),
    }
    match prev_allowed {
        Some(v) => std::env::set_var("ALLOWED_USER_IDS", v),
        None => std::env::remove_var("ALLOWED_USER_IDS"),
    }
    match prev_presets {
        Some(v) => std::env::set_var("LOCATION_PRESETS", v),
        None => std::env::remove_var("LOCATION_PRESETS"),
    }
}

#[test]
fn test_prompt_texts() {
    let empty: Vec<String> = vec![];
    assert_eq!(
        get_location_prompt_text(&empty),
        "📍 Wo fand es statt? Bitte gib den Ort als Text ein (oder klicke auf Fertigstellen):"
    );
    assert_eq!(
        get_partner_name_prompt_text(&empty),
        "💬 Bitte gib den Namen der Person als Text ein:"
    );

    let populated = vec!["Home".to_string()];
    assert_eq!(
        get_location_prompt_text(&populated),
        "📍 Wo fand es statt? Wähle einen Button oder gib den Ort als Text ein:"
    );
    assert_eq!(
        get_partner_name_prompt_text(&populated),
        "👥 Mit welcher Person? Wähle einen Vorschlag oder gib einen neuen Namen als Text ein:"
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

    // Location keyboard - 0 items (empty)
    let location_kb_empty = get_location_keyboard(&[]);
    assert_eq!(location_kb_empty.inline_keyboard.len(), 1);
    assert_eq!(location_kb_empty.inline_keyboard[0].len(), 2);
    assert_eq!(location_kb_empty.inline_keyboard[0][0].text, "✅ Fertigstellen");
    assert_eq!(location_kb_empty.inline_keyboard[0][1].text, "❌ Abbruch");

    // Location keyboard - 1..=4 items (plus manual entry + finish/cancel)
    for count in 1..=4usize {
        let items: Vec<String> = (0..count).map(|i| format!("Ort {}", i + 1)).collect();
        let kb = get_location_keyboard(&items);
        let expected_item_rows = count.div_ceil(2);
        assert_eq!(kb.inline_keyboard.len(), expected_item_rows + 2);

        let mut flat_buttons = Vec::new();
        for row in &kb.inline_keyboard[..expected_item_rows] {
            flat_buttons.extend(row.iter());
        }
        assert_eq!(flat_buttons.len(), count);
        for (i, btn) in flat_buttons.iter().enumerate() {
            assert_eq!(btn.text, items[i]);
            assert_eq!(
                btn.kind,
                InlineKeyboardButtonKind::CallbackData(format!("{}{}", LOCATION_PREFIX, i))
            );
        }

        let manual_row = &kb.inline_keyboard[expected_item_rows];
        assert_eq!(manual_row.len(), 1);
        assert_eq!(manual_row[0].text, "✏️ Neuen Ort eingeben");
        assert_eq!(
            manual_row[0].kind,
            InlineKeyboardButtonKind::CallbackData(LOCATION_MANUAL.to_string())
        );

        let action_row = &kb.inline_keyboard[expected_item_rows + 1];
        assert_eq!(action_row.len(), 2);
        assert_eq!(action_row[0].text, "✅ Fertigstellen");
        assert_eq!(action_row[1].text, "❌ Abbruch");
    }

    // Location keyboard caps at 4 suggestions even if >4 items are passed
    let five_locations: Vec<String> = (1..=5).map(|i| format!("Loc {}", i)).collect();
    let capped_loc_kb = get_location_keyboard(&five_locations);
    assert_eq!(capped_loc_kb.inline_keyboard.len(), 4); // 2 item rows + 1 manual row + 1 action row
    assert_eq!(capped_loc_kb.inline_keyboard[0].len(), 2);
    assert_eq!(capped_loc_kb.inline_keyboard[1].len(), 2);

    // Partner keyboard - 0 items (empty)
    let partner_kb_empty = get_partner_name_keyboard(&[]);
    assert_eq!(partner_kb_empty.inline_keyboard.len(), 1);
    assert_eq!(partner_kb_empty.inline_keyboard[0].len(), 2);
    assert_eq!(partner_kb_empty.inline_keyboard[0][0].text, "✅ Fertigstellen");
    assert_eq!(partner_kb_empty.inline_keyboard[0][1].text, "❌ Abbruch");

    // Partner keyboard - 1..=4 items (plus manual entry + finish/cancel)
    for count in 1..=4usize {
        let items: Vec<String> = (0..count).map(|i| format!("Person {}", i + 1)).collect();
        let kb = get_partner_name_keyboard(&items);
        let expected_item_rows = count.div_ceil(2);
        assert_eq!(kb.inline_keyboard.len(), expected_item_rows + 2);

        let mut flat_buttons = Vec::new();
        for row in &kb.inline_keyboard[..expected_item_rows] {
            flat_buttons.extend(row.iter());
        }
        assert_eq!(flat_buttons.len(), count);
        for (i, btn) in flat_buttons.iter().enumerate() {
            assert_eq!(btn.text, items[i]);
            assert_eq!(
                btn.kind,
                InlineKeyboardButtonKind::CallbackData(format!("{}{}", PARTNER_PREFIX, i))
            );
        }

        let manual_row = &kb.inline_keyboard[expected_item_rows];
        assert_eq!(manual_row.len(), 1);
        assert_eq!(manual_row[0].text, "✏️ Neue Person eingeben");
        assert_eq!(
            manual_row[0].kind,
            InlineKeyboardButtonKind::CallbackData(PARTNER_MANUAL.to_string())
        );

        let action_row = &kb.inline_keyboard[expected_item_rows + 1];
        assert_eq!(action_row.len(), 2);
        assert_eq!(action_row[0].text, "✅ Fertigstellen");
        assert_eq!(action_row[1].text, "❌ Abbruch");
    }

    // Partner keyboard caps at 4 suggestions even if >4 items are passed
    let five_partners: Vec<String> = (1..=5).map(|i| format!("Partner {}", i)).collect();
    let capped_partner_kb = get_partner_name_keyboard(&five_partners);
    assert_eq!(capped_partner_kb.inline_keyboard.len(), 4); // 2 item rows + 1 manual row + 1 action row
    assert_eq!(capped_partner_kb.inline_keyboard[0].len(), 2);
    assert_eq!(capped_partner_kb.inline_keyboard[1].len(), 2);

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

#[test]
fn test_resolve_commit_sha_precedence_and_fallbacks() {
    // Nothing provided -> "unknown"
    assert_eq!(resolve_commit_sha(None, None, None), "unknown");
    assert_eq!(
        resolve_commit_sha(Some("unknown"), Some(""), Some("  ")),
        "unknown"
    );

    // Compile-time SHA used when runtime env is unset or placeholder
    assert_eq!(
        resolve_commit_sha(Some("abc1234"), None, None),
        "abc1234"
    );
    assert_eq!(
        resolve_commit_sha(Some("abc1234"), Some("unknown"), None),
        "abc1234"
    );

    // Runtime GIT_HASH overrides compile-time SHA
    assert_eq!(
        resolve_commit_sha(Some("abc1234"), Some("def5678"), None),
        "def5678"
    );

    // Runtime IMAGE_SHA takes precedence even if Dockerfile baked GIT_HASH=unknown or compile-time SHA
    assert_eq!(
        resolve_commit_sha(Some("unknown"), Some("unknown"), Some("img9999")),
        "img9999"
    );
    assert_eq!(
        resolve_commit_sha(Some("abc1234"), Some("def5678"), Some("img9999")),
        "img9999"
    );
}

#[test]
fn test_get_commit_sha_from_env() {
    let _guard = ENV_MUTEX.lock().unwrap();
    let prev_git_hash = std::env::var("GIT_HASH").ok();
    let prev_image_sha = std::env::var("IMAGE_SHA").ok();

    std::env::remove_var("GIT_HASH");
    std::env::remove_var("IMAGE_SHA");
    assert_eq!(get_commit_sha(None), "unknown");
    assert_eq!(get_commit_sha(Some("compiled-sha")), "compiled-sha");

    std::env::set_var("GIT_HASH", "runtime-git-sha");
    assert_eq!(get_commit_sha(Some("compiled-sha")), "runtime-git-sha");

    std::env::set_var("IMAGE_SHA", "runtime-image-sha");
    assert_eq!(get_commit_sha(Some("compiled-sha")), "runtime-image-sha");

    match prev_git_hash {
        Some(v) => std::env::set_var("GIT_HASH", v),
        None => std::env::remove_var("GIT_HASH"),
    }
    match prev_image_sha {
        Some(v) => std::env::set_var("IMAGE_SHA", v),
        None => std::env::remove_var("IMAGE_SHA"),
    }
}

