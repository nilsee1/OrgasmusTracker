use orgasmus_tracker::models::{DraftEntry, Entry, MAX_DESCRIPTION_LENGTH};
use orgasmus_tracker::utils::{
    format_detailed_summary, format_entries_list_text, format_timestamp_display,
    parse_german_datetime,
};

#[test]
fn test_parse_german_datetime_valid() {
    let dt = parse_german_datetime("19.10.2026 18:30").unwrap();
    assert_eq!(dt.format("%d.%m.%Y %H:%M").to_string(), "19.10.2026 18:30");

    let dt_sec = parse_german_datetime("19.10.2026 18:30:45").unwrap();
    assert_eq!(dt_sec.format("%H:%M:%S").to_string(), "18:30:45");

    let dt_iso = parse_german_datetime("2026-10-19 18:30").unwrap();
    assert_eq!(dt_iso.format("%d.%m.%Y").to_string(), "19.10.2026");
}

#[test]
fn test_parse_german_datetime_invalid() {
    assert!(parse_german_datetime("invalid-date").is_none());
    assert!(parse_german_datetime("32.10.2026 18:30").is_none());
    assert!(parse_german_datetime("19.13.2026 18:30").is_none());
    assert!(parse_german_datetime("").is_none());
}

#[test]
fn test_format_timestamp_display() {
    assert_eq!(
        format_timestamp_display("2026-10-19 18:30"),
        "19.10.2026 um 18:30 Uhr"
    );
    assert_eq!(
        format_timestamp_display("19.10.2026 18:30"),
        "19.10.2026 um 18:30 Uhr"
    );
    assert_eq!(format_timestamp_display(""), "");
}

#[test]
fn test_format_detailed_summary() {
    let draft = DraftEntry {
        r#type: "detailed".to_string(),
        timestamp: Some("2026-10-19 18:30".to_string()),
        company: Some("Mit anderer Person".to_string()),
        partner_name: Some("Julia".to_string()),
        quality: Some(10),
        description: Some("Wunderbar!".to_string()),
    };

    let summary = format_detailed_summary(&draft);
    assert!(summary.contains("Ausführlicher Orgasmus erfolgreich gespeichert!"));
    assert!(summary.contains("19.10.2026 um 18:30 Uhr"));
    assert!(summary.contains("Mit anderer Person (Julia)"));
    assert!(summary.contains("10/10"));
    assert!(summary.contains("Wunderbar!"));
}

#[test]
fn test_format_detailed_summary_alone() {
    let draft = DraftEntry {
        r#type: "detailed".to_string(),
        timestamp: Some("2026-10-19 18:30".to_string()),
        company: Some("Alleine".to_string()),
        partner_name: None,
        quality: Some(8),
        description: None,
    };

    let summary = format_detailed_summary(&draft);
    assert!(summary.contains("Begleitung:</b> Alleine"));
    assert!(summary.contains("8/10"));
    assert!(summary.contains("<i>Keine</i>"));
}

#[test]
fn test_format_entries_list_text_empty() {
    let text = format_entries_list_text(&[]);
    assert!(text.contains("Bisher sind noch keine Einträge vorhanden"));
}

#[test]
fn test_format_entries_list_text_truncation() {
    let long_desc = "X".repeat(MAX_DESCRIPTION_LENGTH + 50);
    let entry = Entry {
        id: 1,
        user_id: 123,
        r#type: "detailed".to_string(),
        timestamp: "2026-10-19 18:30".to_string(),
        company: Some("Alleine".to_string()),
        partner_name: None,
        quality: Some(9),
        description: Some(long_desc.clone()),
        created_at: "2026-10-19T18:30:00Z".to_string(),
    };

    let text = format_entries_list_text(&[entry]);
    let expected_truncated = format!("{}…", "X".repeat(MAX_DESCRIPTION_LENGTH));
    assert!(text.contains(&expected_truncated));
    assert!(!text.contains(&long_desc));
}

#[test]
fn test_format_entries_list_text_escaping() {
    let entry = Entry {
        id: 2,
        user_id: 123,
        r#type: "detailed".to_string(),
        timestamp: "2026-10-19 18:30".to_string(),
        company: Some("Mit anderer Person".to_string()),
        partner_name: Some("<script>bad()</script>".to_string()),
        quality: Some(5),
        description: Some("<b>Note & \"Quotes\"</b>".to_string()),
        created_at: "2026-10-19T18:30:00Z".to_string(),
    };

    let text = format_entries_list_text(&[entry]);
    assert!(!text.contains("<script>"));
    assert!(text.contains("&lt;script&gt;bad()&lt;/script&gt;"));
}

