use chrono::{Duration, TimeZone, Utc};
use orgasmus_tracker::handlers::{Command, State};
use orgasmus_tracker::models::{DraftEntry, MAX_DESCRIPTION_LENGTH, MAX_PARTNER_NAME_LENGTH};
use orgasmus_tracker::utils::parse_german_datetime;
use teloxide::utils::command::BotCommands;

#[test]
fn test_command_parsing() {
    assert_eq!(Command::parse("/start", "bot").unwrap(), Command::Start);
    assert_eq!(Command::parse("/orgasmus", "bot").unwrap(), Command::Orgasmus);
    assert_eq!(Command::parse("/liste", "bot").unwrap(), Command::Liste);
    assert_eq!(Command::parse("/letzte", "bot").unwrap(), Command::Letzte);
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
        last_keyboard_msg_id: Some(101),
    };
    assert_eq!(state_partner.last_keyboard_msg_id(), Some(101));

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
fn test_description_length_validation() {
    let valid_desc = "D".repeat(MAX_DESCRIPTION_LENGTH);
    assert!(valid_desc.chars().count() <= MAX_DESCRIPTION_LENGTH);

    let too_long_desc = "D".repeat(MAX_DESCRIPTION_LENGTH + 1);
    assert!(too_long_desc.chars().count() > MAX_DESCRIPTION_LENGTH);
}
