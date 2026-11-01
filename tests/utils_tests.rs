use orgasmus_tracker::models::{
    DraftEntry, Entry, RankedStatItem, StatsSummary, TimeOfDayDistribution, MAX_DESCRIPTION_LENGTH,
};
use orgasmus_tracker::utils::{
    escape_csv_field, format_detailed_summary, format_entries_list_text,
    format_statistics_text, format_timestamp_display, generate_csv_export,
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
        location: Some("Nils".to_string()),
        quality: Some(10),
        description: Some("Wunderbar!".to_string()),
    };

    let summary = format_detailed_summary(&draft);
    assert!(summary.contains("Ausführlicher Orgasmus erfolgreich gespeichert!"));
    assert!(summary.contains("19.10.2026 um 18:30 Uhr"));
    assert!(summary.contains("Mit anderer Person (Julia)"));
    assert!(summary.contains("Ort:</b> Nils"));
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
        location: None,
        quality: Some(8),
        description: None,
    };

    let summary = format_detailed_summary(&draft);
    assert!(summary.contains("Begleitung:</b> Alleine"));
    assert!(!summary.contains("Ort:"));
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
        location: None,
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
        location: Some("<b>Home</b>".to_string()),
        quality: Some(5),
        description: Some("<b>Note & \"Quotes\"</b>".to_string()),
        created_at: "2026-10-19T18:30:00Z".to_string(),
    };

    let text = format_entries_list_text(&[entry]);
    assert!(!text.contains("<script>"));
    assert!(text.contains("&lt;script&gt;bad()&lt;/script&gt;"));
    assert!(!text.contains("<b>Home</b>"));
    assert!(text.contains("&lt;b&gt;Home&lt;/b&gt;"));
}

#[test]
fn test_escape_csv_field() {
    assert_eq!(escape_csv_field("Hello World"), "Hello World");
    assert_eq!(escape_csv_field("Hello;World"), "\"Hello;World\"");
    assert_eq!(escape_csv_field("Hello \"World\""), "\"Hello \"\"World\"\"\"");
    assert_eq!(escape_csv_field("Line1\nLine2"), "\"Line1\nLine2\"");
    assert_eq!(escape_csv_field("Line1\r\nLine2"), "\"Line1\r\nLine2\"");
    assert_eq!(escape_csv_field("A;\"B\"\nC"), "\"A;\"\"B\"\"\nC\"");
}

#[test]
fn test_generate_csv_export_empty() {
    let bytes = generate_csv_export(&[]);
    let content = String::from_utf8(bytes).unwrap();
    // Verify UTF-8 BOM
    assert!(content.starts_with('\u{FEFF}'));
    // Verify header line
    assert!(content.contains("ID;Typ;Datum_Uhrzeit;Begleitung;Partner;Ort;Qualitaet;Notiz;Erstellt_Am\r\n"));
    let lines: Vec<&str> = content.split("\r\n").filter(|s| !s.is_empty()).collect();
    assert_eq!(lines.len(), 1); // Header only
}

#[test]
fn test_generate_csv_export_records() {
    let entries = vec![
        Entry {
            id: 1,
            user_id: 100,
            r#type: "simple".to_string(),
            timestamp: "2026-10-19 12:00".to_string(),
            company: None,
            partner_name: None,
            location: None,
            quality: None,
            description: None,
            created_at: "2026-10-19T12:00:00Z".to_string(),
        },
        Entry {
            id: 2,
            user_id: 100,
            r#type: "detailed".to_string(),
            timestamp: "2026-10-20 18:30".to_string(),
            company: Some("Mit anderer Person".to_string()),
            partner_name: Some("Julia; Sarah".to_string()),
            location: Some("Mika; Nils".to_string()),
            quality: Some(9),
            description: Some("Toll & \"wundervoll\"\nZweite Zeile".to_string()),
            created_at: "2026-10-20T18:35:00Z".to_string(),
        },
    ];

    let bytes = generate_csv_export(&entries);
    let content = String::from_utf8(bytes).unwrap();

    assert!(content.starts_with('\u{FEFF}'));
    assert!(content.contains("1;simple;2026-10-19 12:00;;;;;;2026-10-19T12:00:00Z\r\n"));
    assert!(content.contains("2;detailed;2026-10-20 18:30;Mit anderer Person;\"Julia; Sarah\";\"Mika; Nils\";9;\"Toll & \"\"wundervoll\"\"\nZweite Zeile\";2026-10-20T18:35:00Z\r\n"));
}

#[test]
fn test_format_statistics_text_empty() {
    let stats = StatsSummary::default();
    let text = format_statistics_text(&stats, 2026);
    assert!(text.contains("Bisher sind noch keine Einträge vorhanden"));
}

#[test]
fn test_format_statistics_text_populated() {
    let stats = StatsSummary {
        total_count: 42,
        count_last_7_days: 5,
        count_last_30_days: 18,
        count_current_year: 35,
        count_last_365_days: 40,
        count_previous_year: 7,
        count_alone: 25,
        count_partner: 17,
        avg_quality: Some(8.7),
        ..Default::default()
    };

    let text = format_statistics_text(&stats, 2026);
    assert!(text.contains("📊 <b>Persönliche Statistiken</b>"));
    assert!(text.contains("Gesamt: <b>42</b>"));
    assert!(text.contains("Letzte 7 Tage: <b>5</b>"));
    assert!(text.contains("Letzte 30 Tage: <b>18</b>"));
    assert!(text.contains("Laufendes Jahr (2026): <b>35</b>"));
    assert!(text.contains("Letzte 365 Tage: <b>40</b>"));
    assert!(text.contains("Vorjahr (2025): <b>7</b>"));
    assert!(text.contains("👤 Alleine: <b>25</b>"));
    assert!(text.contains("👥 Mit anderer Person: <b>17</b>"));
    assert!(text.contains("⭐ <b>8.7 / 10</b>"));
}

#[test]
fn test_format_statistics_text_no_quality_and_no_prev_year() {
    let stats = StatsSummary {
        total_count: 1,
        count_last_7_days: 1,
        count_last_30_days: 1,
        count_current_year: 1,
        count_last_365_days: 1,
        count_previous_year: 0,
        count_alone: 0,
        count_partner: 0,
        avg_quality: None,
        ..Default::default()
    };

    let text = format_statistics_text(&stats, 2026);
    assert!(!text.contains("Vorjahr"));
    assert!(text.contains("<i>Keine Angabe</i>"));
}

#[test]
fn test_format_statistics_text_extended_sections() {
    let stats = StatsSummary {
        total_count: 50,
        count_last_7_days: 7,
        count_last_30_days: 20,
        count_current_year: 45,
        count_last_365_days: 48,
        count_previous_year: 10,
        count_alone: 30,
        count_partner: 20,
        avg_quality: Some(8.5),
        top_locations: vec![
            RankedStatItem {
                name: "Home & Garden <3".to_string(),
                count: 15,
                avg_quality: Some(9.2),
            },
            RankedStatItem {
                name: "Office".to_string(),
                count: 5,
                avg_quality: None,
            },
        ],
        top_partners: vec![
            RankedStatItem {
                name: "Julia & Sarah".to_string(),
                count: 10,
                avg_quality: Some(8.9),
            },
        ],
        current_streak_days: 3,
        longest_streak_days: 7,
        days_since_last: Some(0), // Heute
        longest_gap_days: Some(5),
        avg_days_between: Some(2.1),
        most_active_weekday: Some(("Samstag".to_string(), 12)),
        time_of_day: TimeOfDayDistribution {
            morning: 10,
            afternoon: 15,
            evening: 20,
            night: 5,
        },
    };

    let text = format_statistics_text(&stats, 2026);

    // Verify Serien & Abstände
    assert!(text.contains("🔥 <b>Serien &amp; Abstände:</b>"));
    assert!(text.contains("• Aktuelle Serie: <b>3 Tag(e)</b>"));
    assert!(text.contains("• Längste Serie: <b>7 Tag(e)</b>"));
    assert!(text.contains("• Letzter Eintrag: <b>Heute</b>"));
    assert!(text.contains("• Ø Abstand: <b>2.1 Tage</b>"));
    assert!(text.contains("• Längste Pause: <b>5 Tag(e)</b>"));

    // Verify Zeit-Muster
    assert!(text.contains("🕒 <b>Zeit-Muster:</b>"));
    assert!(text.contains("• Beliebtester Tag: <b>Samstag</b> (12x)"));
    assert!(text.contains("• Tageszeit: 🌅 Morgens: <b>10</b> | ☀️ Mittags: <b>15</b> | 🌆 Abends: <b>20</b> | 🌙 Nachts: <b>5</b>"));

    // Verify Top Orte with HTML escaping and quality
    assert!(text.contains("📍 <b>Top Orte:</b>"));
    assert!(text.contains("• 1. <b>Home &amp; Garden &lt;3</b>: 15x (⭐ 9.2)"));
    assert!(text.contains("• 2. <b>Office</b>: 5x"));
    assert!(!text.contains("• 2. <b>Office</b>: 5x (⭐"));

    // Verify Top Partner with HTML escaping
    assert!(text.contains("💖 <b>Top Partner:</b>"));
    assert!(text.contains("• 1. <b>Julia &amp; Sarah</b>: 10x (⭐ 8.9)"));
}

#[test]
fn test_format_statistics_text_days_since_last_variations() {
    let mut stats = StatsSummary {
        total_count: 5,
        days_since_last: Some(1), // Gestern
        ..Default::default()
    };
    let text_gestern = format_statistics_text(&stats, 2026);
    assert!(text_gestern.contains("• Letzter Eintrag: <b>Gestern</b>"));

    stats.days_since_last = Some(4); // vor 4 Tagen
    let text_vor_4 = format_statistics_text(&stats, 2026);
    assert!(text_vor_4.contains("• Letzter Eintrag: <b>vor 4 Tagen</b>"));
}

#[test]
fn test_format_statistics_text_omits_empty_top_sections() {
    let stats = StatsSummary {
        total_count: 5,
        top_locations: vec![],
        top_partners: vec![],
        ..Default::default()
    };
    let text = format_statistics_text(&stats, 2026);
    assert!(!text.contains("📍 <b>Top Orte:</b>"));
    assert!(!text.contains("💖 <b>Top Partner:</b>"));
}

