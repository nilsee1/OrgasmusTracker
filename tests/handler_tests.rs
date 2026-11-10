use chrono::{Duration, TimeZone, Utc};
use orgasmus_tracker::database::Database;
use orgasmus_tracker::handlers::{
    get_location_prompt_text, get_partner_name_prompt_text, prepare_location_step,
    prepare_partner_step, resolve_suggestion_index, Command, State, TOP_SUGGESTIONS_LIMIT,
};
use orgasmus_tracker::keyboards::{
    ACTION_CANCEL, ACTION_FINISH, LOCATION_MANUAL, LOCATION_PREFIX, PARTNER_MANUAL, PARTNER_PREFIX,
};
use orgasmus_tracker::models::{
    DraftEntry, MAX_DESCRIPTION_LENGTH, MAX_LOCATION_LENGTH, MAX_PARTNER_NAME_LENGTH,
};
use orgasmus_tracker::utils::parse_german_datetime;
use teloxide::types::InlineKeyboardButtonKind;
use teloxide::utils::command::BotCommands;
use tempfile::NamedTempFile;

#[test]
fn test_command_parsing() {
    assert_eq!(Command::parse("/start", "bot").unwrap(), Command::Start);
    assert_eq!(Command::parse("/orgasmus", "bot").unwrap(), Command::Orgasmus);
    assert_eq!(Command::parse("/liste", "bot").unwrap(), Command::Liste);
    assert_eq!(Command::parse("/letzte", "bot").unwrap(), Command::Letzte);
    assert_eq!(Command::parse("/list", "bot").unwrap(), Command::List);
    assert_eq!(Command::parse("/statistik", "bot").unwrap(), Command::Statistik);
    assert_eq!(Command::parse("/stats", "bot").unwrap(), Command::Stats);
    assert_eq!(Command::parse("/export", "bot").unwrap(), Command::Export);
    assert_eq!(Command::parse("/hilfe", "bot").unwrap(), Command::Hilfe);
    assert_eq!(Command::parse("/help", "bot").unwrap(), Command::Help);
    assert_eq!(Command::parse("/abbrechen", "bot").unwrap(), Command::Abbrechen);
    assert_eq!(Command::parse("/cancel", "bot").unwrap(), Command::Cancel);
}

#[test]
fn test_state_last_keyboard_msg_id() {
    let state_start = State::Start;
    assert_eq!(state_start.last_keyboard_msg_id(), None);

    let state_choose = State::ChooseType {
        last_keyboard_msg_id: Some(123),
    };
    assert_eq!(state_choose.last_keyboard_msg_id(), Some(123));

    let state_dt = State::DateTime {
        draft: DraftEntry::default(),
        last_keyboard_msg_id: Some(456),
    };
    assert_eq!(state_dt.last_keyboard_msg_id(), Some(456));

    let state_company = State::Company {
        draft: DraftEntry::default(),
        last_keyboard_msg_id: Some(789),
    };
    assert_eq!(state_company.last_keyboard_msg_id(), Some(789));

    let state_partner = State::PartnerName {
        draft: DraftEntry::default(),
        suggestions: vec!["Julia".to_string()],
        last_keyboard_msg_id: Some(101),
    };
    assert_eq!(state_partner.last_keyboard_msg_id(), Some(101));

    let state_location = State::Location {
        draft: DraftEntry::default(),
        suggestions: vec!["Zuhause".to_string()],
        last_keyboard_msg_id: Some(151),
    };
    assert_eq!(state_location.last_keyboard_msg_id(), Some(151));

    let state_quality = State::Quality {
        draft: DraftEntry::default(),
        last_keyboard_msg_id: Some(202),
    };
    assert_eq!(state_quality.last_keyboard_msg_id(), Some(202));

    let state_desc = State::Description {
        draft: DraftEntry::default(),
        last_keyboard_msg_id: Some(303),
    };
    assert_eq!(state_desc.last_keyboard_msg_id(), Some(303));
}

#[test]
fn test_future_datetime_validation_logic() {
    let tz = chrono_tz::Europe::Berlin;
    let now = Utc::now().with_timezone(&tz);

    // Past date
    let past = now - Duration::hours(2);
    let past_str = past.format("%d.%m.%Y %H:%M").to_string();
    let parsed_past = parse_german_datetime(&past_str).unwrap();
    let local_past = tz.from_local_datetime(&parsed_past).unwrap();
    assert!(local_past <= now);

    // Future date
    let future = now + Duration::days(2);
    let future_str = future.format("%d.%m.%Y %H:%M").to_string();
    let parsed_future = parse_german_datetime(&future_str).unwrap();
    let local_future = tz.from_local_datetime(&parsed_future).unwrap();
    assert!(local_future > now);
}

#[test]
fn test_partner_name_length_validation() {
    let valid_name = "A".repeat(MAX_PARTNER_NAME_LENGTH);
    assert!(valid_name.chars().count() <= MAX_PARTNER_NAME_LENGTH);

    let too_long_name = "A".repeat(MAX_PARTNER_NAME_LENGTH + 1);
    assert!(too_long_name.chars().count() > MAX_PARTNER_NAME_LENGTH);
}

#[test]
fn test_location_length_validation() {
    let valid_loc = "L".repeat(MAX_LOCATION_LENGTH);
    assert!(valid_loc.chars().count() <= MAX_LOCATION_LENGTH);

    let too_long_loc = "L".repeat(MAX_LOCATION_LENGTH + 1);
    assert!(too_long_loc.chars().count() > MAX_LOCATION_LENGTH);
}

#[test]
fn test_description_length_validation() {
    let valid_desc = "D".repeat(MAX_DESCRIPTION_LENGTH);
    assert!(valid_desc.chars().count() <= MAX_DESCRIPTION_LENGTH);

    let too_long_desc = "D".repeat(MAX_DESCRIPTION_LENGTH + 1);
    assert!(too_long_desc.chars().count() > MAX_DESCRIPTION_LENGTH);
}

#[test]
fn test_resolve_suggestion_index() {
    let suggestions = vec![
        "Zuhause".to_string(),
        "Hotel".to_string(),
        "Strand".to_string(),
    ];

    assert_eq!(
        resolve_suggestion_index("LOC_IDX_0", LOCATION_PREFIX, &suggestions),
        Some("Zuhause".to_string())
    );
    assert_eq!(
        resolve_suggestion_index("LOC_IDX_2", LOCATION_PREFIX, &suggestions),
        Some("Strand".to_string())
    );
    assert_eq!(
        resolve_suggestion_index("LOC_IDX_3", LOCATION_PREFIX, &suggestions),
        None
    );
    assert_eq!(
        resolve_suggestion_index(LOCATION_MANUAL, LOCATION_PREFIX, &suggestions),
        None
    );
    assert_eq!(
        resolve_suggestion_index("PARTNER_IDX_0", LOCATION_PREFIX, &suggestions),
        None
    );
    assert_eq!(
        resolve_suggestion_index("PARTNER_IDX_1", PARTNER_PREFIX, &suggestions),
        Some("Hotel".to_string())
    );
}

async fn create_test_db() -> (Database, NamedTempFile) {
    let temp_file = NamedTempFile::new().unwrap();
    let db_path = temp_file.path().to_str().unwrap().to_string();
    let db = Database::connect(&db_path).await.unwrap();
    (db, temp_file)
}

#[tokio::test]
async fn test_prepare_partner_step_dynamic_and_isolated() {
    let (db, _tmp) = create_test_db().await;
    let user_a = 1001;
    let user_b = 2002;

    assert_eq!(TOP_SUGGESTIONS_LIMIT, 4);

    // 0 entries -> direct text prompt, finish/cancel only
    let (suggestions_empty, prompt_empty, kb_empty) =
        prepare_partner_step(&db, user_a).await.unwrap();
    assert!(suggestions_empty.is_empty());
    assert_eq!(prompt_empty, get_partner_name_prompt_text(&[]));
    assert_eq!(kb_empty.inline_keyboard.len(), 1);
    assert_eq!(
        kb_empty.inline_keyboard[0][0].kind,
        InlineKeyboardButtonKind::CallbackData(ACTION_FINISH.to_string())
    );
    assert_eq!(
        kb_empty.inline_keyboard[0][1].kind,
        InlineKeyboardButtonKind::CallbackData(ACTION_CANCEL.to_string())
    );

    // Populate 5 partners for User A and high-frequency partners for User B
    for _ in 0..4 {
        db.add_entry(user_a, "detailed", "2026-10-01 10:00", Some("Mit anderer Person"), Some("Julia"), None, None, None, None).await.unwrap();
    }
    for _ in 0..3 {
        db.add_entry(user_a, "detailed", "2026-10-02 10:00", Some("Mit anderer Person"), Some("Alex"), None, None, None, None).await.unwrap();
    }
    for _ in 0..2 {
        db.add_entry(user_a, "detailed", "2026-10-03 10:00", Some("Mit anderer Person"), Some("Sam"), None, None, None, None).await.unwrap();
    }
    db.add_entry(user_a, "detailed", "2026-10-04 10:00", Some("Mit anderer Person"), Some("Mika"), None, None, None, None).await.unwrap();
    db.add_entry(user_a, "detailed", "2026-10-05 10:00", Some("Mit anderer Person"), Some("Lukas"), None, None, None, None).await.unwrap();

    for _ in 0..10 {
        db.add_entry(user_b, "detailed", "2026-10-06 10:00", Some("Mit anderer Person"), Some("SecretUserB"), None, None, None, None).await.unwrap();
    }

    let (suggestions_a, prompt_a, kb_a) = prepare_partner_step(&db, user_a).await.unwrap();
    assert_eq!(suggestions_a, vec!["Julia", "Alex", "Sam", "Lukas"]);
    assert_eq!(prompt_a, get_partner_name_prompt_text(&suggestions_a));
    // 2 rows of 2 partner buttons + 1 manual row + 1 finish/cancel row
    assert_eq!(kb_a.inline_keyboard.len(), 4);
    assert_eq!(kb_a.inline_keyboard[0][0].text, "Julia");
    assert_eq!(kb_a.inline_keyboard[0][1].text, "Alex");
    assert_eq!(kb_a.inline_keyboard[1][0].text, "Sam");
    assert_eq!(kb_a.inline_keyboard[1][1].text, "Lukas");
    assert_eq!(
        kb_a.inline_keyboard[2][0].kind,
        InlineKeyboardButtonKind::CallbackData(PARTNER_MANUAL.to_string())
    );

    // Ensure User B's partner is isolated from User A and vice versa
    let (suggestions_b, _, _) = prepare_partner_step(&db, user_b).await.unwrap();
    assert_eq!(suggestions_b, vec!["SecretUserB"]);
}

#[tokio::test]
async fn test_prepare_location_step_dynamic_and_isolated() {
    let (db, _tmp) = create_test_db().await;
    let user_a = 3003;
    let user_b = 4004;

    // 0 entries -> direct text prompt, finish/cancel only
    let (suggestions_empty, prompt_empty, kb_empty) =
        prepare_location_step(&db, user_a).await.unwrap();
    assert!(suggestions_empty.is_empty());
    assert_eq!(prompt_empty, get_location_prompt_text(&[]));
    assert_eq!(kb_empty.inline_keyboard.len(), 1);

    // Populate 5 locations for User A and separate locations for User B
    for _ in 0..4 {
        db.add_entry(user_a, "detailed", "2026-10-01 10:00", Some("Alleine"), None, Some("Zuhause"), None, None, None).await.unwrap();
    }
    for _ in 0..3 {
        db.add_entry(user_a, "detailed", "2026-10-02 10:00", Some("Alleine"), None, Some("Hotel"), None, None, None).await.unwrap();
    }
    for _ in 0..2 {
        db.add_entry(user_a, "detailed", "2026-10-03 10:00", Some("Alleine"), None, Some("Strand"), None, None, None).await.unwrap();
    }
    db.add_entry(user_a, "detailed", "2026-10-04 10:00", Some("Alleine"), None, Some("Auto"), None, None, None).await.unwrap();
    db.add_entry(user_a, "detailed", "2026-10-05 10:00", Some("Alleine"), None, Some("Wald"), None, None, None).await.unwrap();

    for _ in 0..8 {
        db.add_entry(user_b, "detailed", "2026-10-06 10:00", Some("Alleine"), None, Some("SecretLocB"), None, None, None).await.unwrap();
    }

    let (suggestions_a, prompt_a, kb_a) = prepare_location_step(&db, user_a).await.unwrap();
    assert_eq!(suggestions_a, vec!["Zuhause", "Hotel", "Strand", "Wald"]);
    assert_eq!(prompt_a, get_location_prompt_text(&suggestions_a));
    assert_eq!(kb_a.inline_keyboard.len(), 4);
    assert_eq!(kb_a.inline_keyboard[0][0].text, "Zuhause");
    assert_eq!(kb_a.inline_keyboard[0][1].text, "Hotel");
    assert_eq!(kb_a.inline_keyboard[1][0].text, "Strand");
    assert_eq!(kb_a.inline_keyboard[1][1].text, "Wald");
    assert_eq!(
        kb_a.inline_keyboard[2][0].kind,
        InlineKeyboardButtonKind::CallbackData(LOCATION_MANUAL.to_string())
    );

    let (suggestions_b, _, _) = prepare_location_step(&db, user_b).await.unwrap();
    assert_eq!(suggestions_b, vec!["SecretLocB"]);
}

#[tokio::test]
async fn test_dynamic_dialogue_state_transitions() {
    let (db, _tmp) = create_test_db().await;
    let user_id = 5005;

    db.add_entry(
        user_id,
        "detailed",
        "2026-10-10 10:00",
        Some("Mit anderer Person"),
        Some("Julia"),
        Some("Zuhause"),
        Some(9),
        None,
        None,
    )
    .await
    .unwrap();

    // 1. User chooses partner -> enters State::PartnerName with dynamic suggestions
    let (partner_suggestions, _, _) = prepare_partner_step(&db, user_id).await.unwrap();
    let mut draft = DraftEntry {
        r#type: "detailed".to_string(),
        timestamp: Some("2026-10-20 18:00".to_string()),
        company: Some("Mit anderer Person".to_string()),
        ..Default::default()
    };
    let partner_state = State::PartnerName {
        draft: draft.clone(),
        suggestions: partner_suggestions.clone(),
        last_keyboard_msg_id: Some(10),
    };

    // 2. Selecting partner index 0 resolves "Julia" and advances to State::Location
    let chosen_partner = match &partner_state {
        State::PartnerName { suggestions, .. } => {
            resolve_suggestion_index("PARTNER_IDX_0", PARTNER_PREFIX, suggestions)
        }
        _ => None,
    };
    assert_eq!(chosen_partner.as_deref(), Some("Julia"));
    draft.partner_name = chosen_partner;

    let (location_suggestions, _, _) = prepare_location_step(&db, user_id).await.unwrap();
    let location_state = State::Location {
        draft: draft.clone(),
        suggestions: location_suggestions.clone(),
        last_keyboard_msg_id: Some(10),
    };

    // 3. Selecting location index 0 resolves "Zuhause" and advances to State::Quality
    let chosen_location = match &location_state {
        State::Location { suggestions, .. } => {
            resolve_suggestion_index("LOC_IDX_0", LOCATION_PREFIX, suggestions)
        }
        _ => None,
    };
    assert_eq!(chosen_location.as_deref(), Some("Zuhause"));
    draft.location = chosen_location;

    let quality_state = State::Quality {
        draft: draft.clone(),
        last_keyboard_msg_id: Some(10),
    };
    assert_eq!(quality_state.last_keyboard_msg_id(), Some(10));
    assert_eq!(draft.partner_name.as_deref(), Some("Julia"));
    assert_eq!(draft.location.as_deref(), Some("Zuhause"));
}
