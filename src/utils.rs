//! Helper utilities for date parsing, formatting, and HTML message construction.

use chrono::NaiveDateTime;
use crate::models::{DraftEntry, Entry, StatsSummary, MAX_DESCRIPTION_LENGTH};

/// Parse a German datetime string such as "19.10.2026 18:30" or ISO formats into NaiveDateTime.
pub fn parse_german_datetime(input: &str) -> Option<NaiveDateTime> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    let formats = [
        "%d.%m.%Y %H:%M",
        "%d.%m.%Y %H:%M:%S",
        "%Y-%m-%d %H:%M",
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%dT%H:%M:%S",
    ];

    for fmt in formats {
        if let Ok(dt) = NaiveDateTime::parse_from_str(trimmed, fmt) {
            return Some(dt);
        }
    }

    None
}

/// Format raw timestamp string into friendly German display format "TT.MM.JJJJ um HH:MM Uhr".
pub fn format_timestamp_display(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    if let Some(dt) = parse_german_datetime(trimmed) {
        return dt.format("%d.%m.%Y um %H:%M Uhr").to_string();
    }

    // Try parsing ISO RFC3339
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(trimmed) {
        return dt.format("%d.%m.%Y um %H:%M Uhr").to_string();
    }

    trimmed.to_string()
}

/// Format a German HTML confirmation summary of a saved detailed entry.
pub fn format_detailed_summary(draft: &DraftEntry) -> String {
    let ts = draft.timestamp.as_deref().unwrap_or("");
    let formatted_ts = format_timestamp_display(ts);

    let mut lines = vec![
        "✅ <b>Ausführlicher Orgasmus erfolgreich gespeichert!</b>".to_string(),
        "".to_string(),
        format!(
            "📅 <b>Datum &amp; Uhrzeit:</b> {}",
            html_escape::encode_text(&formatted_ts)
        ),
    ];

    match draft.company.as_deref() {
        Some("Alleine") => lines.push("👥 <b>Begleitung:</b> Alleine".to_string()),
        Some("Mit anderer Person") => {
            if let Some(partner) = &draft.partner_name {
                lines.push(format!(
                    "👥 <b>Begleitung:</b> Mit anderer Person ({})",
                    html_escape::encode_text(partner)
                ));
            } else {
                lines.push("👥 <b>Begleitung:</b> Mit anderer Person".to_string());
            }
        }
        _ => lines.push("👥 <b>Begleitung:</b> <i>Keine Angabe</i>".to_string()),
    }

    if let Some(loc) = &draft.location {
        lines.push(format!("📍 <b>Ort:</b> {}", html_escape::encode_text(loc)));
    }

    if let Some(q) = draft.quality {
        lines.push(format!("⭐ <b>Qualität:</b> {}/10", q));
    } else {
        lines.push("⭐ <b>Qualität:</b> <i>Keine Angabe</i>".to_string());
    }

    if let Some(desc) = &draft.description {
        lines.push(format!(
            "📝 <b>Beschreibung:</b> {}",
            html_escape::encode_text(desc)
        ));
    } else {
        lines.push("📝 <b>Beschreibung:</b> <i>Keine</i>".to_string());
    }

    lines.join("\n")
}

/// Format the list of entries into HTML for the /liste command.
pub fn format_entries_list_text(entries: &[Entry]) -> String {
    if entries.is_empty() {
        return "ℹ️ <b>Bisher sind noch keine Einträge vorhanden.</b>\n\nNutze /orgasmus, um deinen ersten Eintrag zu erstellen.".to_string();
    }

    let count = entries.len();
    let mut blocks = vec![format!("📊 <b>Deine letzten {} Einträge:</b>\n", count)];

    for (idx, entry) in entries.iter().enumerate() {
        let ts_formatted = format_timestamp_display(&entry.timestamp);
        let entry_type = if entry.r#type == "detailed" {
            "Ausführlich"
        } else {
            "Einfach"
        };

        let mut block_lines = vec![format!(
            "<b>{}.</b> 📅 <b>{}</b> (<i>{}</i>)",
            idx + 1,
            html_escape::encode_text(&ts_formatted),
            entry_type
        )];

        if entry.r#type == "detailed" {
            if let Some(company) = &entry.company {
                if company == "Mit anderer Person" {
                    if let Some(partner) = &entry.partner_name {
                        block_lines.push(format!(
                            "   👥 Begleitung: Mit {}",
                            html_escape::encode_text(partner)
                        ));
                    } else {
                        block_lines.push("   👥 Begleitung: Mit anderer Person".to_string());
                    }
                } else {
                    block_lines.push(format!(
                        "   👥 Begleitung: {}",
                        html_escape::encode_text(company)
                    ));
                }
            }

            if let Some(loc) = &entry.location {
                block_lines.push(format!(
                    "   📍 Ort: {}",
                    html_escape::encode_text(loc)
                ));
            }

            if let Some(quality) = entry.quality {
                block_lines.push(format!("   ⭐ Qualität: {}/10", quality));
            }

            if let Some(desc) = &entry.description {
                let formatted_desc = if desc.chars().count() > MAX_DESCRIPTION_LENGTH {
                    let prefix: String = desc.chars().take(MAX_DESCRIPTION_LENGTH).collect();
                    format!("{}…", prefix)
                } else {
                    desc.clone()
                };
                block_lines.push(format!(
                    "   📝 Notiz: {}",
                    html_escape::encode_text(&formatted_desc)
                ));
            }
        }

        blocks.push(block_lines.join("\n"));
    }

    blocks.join("\n\n")
}

/// Format the statistics summary into clean, user-friendly HTML.
pub fn format_statistics_text(stats: &StatsSummary, current_year: i32) -> String {
    if stats.total_count == 0 {
        return "ℹ️ <b>Bisher sind noch keine Einträge vorhanden.</b>\n\n\
            Nutze /orgasmus, um deinen ersten Eintrag zu erstellen und Statistiken freizuschalten."
            .to_string();
    }

    let quality_str = match stats.avg_quality {
        Some(q) => format!("{:.1} / 10", q),
        None => "<i>Keine Angabe</i>".to_string(),
    };

    let prev_year = current_year - 1;
    let prev_year_line = if stats.count_previous_year > 0 {
        format!("\n• Vorjahr ({}): <b>{}</b>", prev_year, stats.count_previous_year)
    } else {
        String::new()
    };

    let mut result = format!(
        "📊 <b>Persönliche Statistiken</b>\n\n\
        📈 <b>Einträge nach Zeitraum:</b>\n\
        • Gesamt: <b>{}</b>\n\
        • Letzte 7 Tage: <b>{}</b>\n\
        • Letzte 30 Tage: <b>{}</b>\n\
        • Laufendes Jahr ({}): <b>{}</b>\n\
        • Letzte 365 Tage: <b>{}</b>{}\n\n\
        👥 <b>Begleitung (Ausführlich):</b>\n\
        • 👤 Alleine: <b>{}</b>\n\
        • 👥 Mit anderer Person: <b>{}</b>\n\n\
        ⭐ <b>Qualitäts-Durchschnitt:</b>\n\
        • ⭐ <b>{}</b>",
        stats.total_count,
        stats.count_last_7_days,
        stats.count_last_30_days,
        current_year,
        stats.count_current_year,
        stats.count_last_365_days,
        prev_year_line,
        stats.count_alone,
        stats.count_partner,
        quality_str
    );

    let mut streak_lines = vec![
        "🔥 <b>Serien &amp; Abstände:</b>".to_string(),
        format!("• Aktuelle Serie: <b>{} Tag(e)</b>", stats.current_streak_days),
        format!("• Längste Serie: <b>{} Tag(e)</b>", stats.longest_streak_days),
    ];

    if let Some(d) = stats.days_since_last {
        let last_str = match d {
            0 => "Heute".to_string(),
            1 => "Gestern".to_string(),
            n => format!("vor {} Tagen", n),
        };
        streak_lines.push(format!("• Letzter Eintrag: <b>{}</b>", last_str));
    }

    if let Some(avg) = stats.avg_days_between {
        streak_lines.push(format!("• Ø Abstand: <b>{:.1} Tage</b>", avg));
    }

    if let Some(gap) = stats.longest_gap_days {
        streak_lines.push(format!("• Längste Pause: <b>{} Tag(e)</b>", gap));
    }

    result.push_str("\n\n");
    result.push_str(&streak_lines.join("\n"));

    let mut pattern_lines = vec!["🕒 <b>Zeit-Muster:</b>".to_string()];

    if let Some((day, count)) = &stats.most_active_weekday {
        pattern_lines.push(format!("• Beliebtester Tag: <b>{}</b> ({}x)", day, count));
    }

    pattern_lines.push(format!(
        "• Tageszeit: 🌅 Morgens: <b>{}</b> | ☀️ Mittags: <b>{}</b> | 🌆 Abends: <b>{}</b> | 🌙 Nachts: <b>{}</b>",
        stats.time_of_day.morning,
        stats.time_of_day.afternoon,
        stats.time_of_day.evening,
        stats.time_of_day.night
    ));

    result.push_str("\n\n");
    result.push_str(&pattern_lines.join("\n"));

    if !stats.top_locations.is_empty() {
        let mut loc_lines = vec!["📍 <b>Top Orte:</b>".to_string()];
        for (i, item) in stats.top_locations.iter().enumerate() {
            let escaped_name = html_escape::encode_text(&item.name);
            let quality_str = match item.avg_quality {
                Some(q) => format!(" (⭐ {:.1})", q),
                None => String::new(),
            };
            loc_lines.push(format!(
                "• {}. <b>{}</b>: {}x{}",
                i + 1,
                escaped_name,
                item.count,
                quality_str
            ));
        }
        result.push_str("\n\n");
        result.push_str(&loc_lines.join("\n"));
    }

    if !stats.top_partners.is_empty() {
        let mut partner_lines = vec!["💖 <b>Top Partner:</b>".to_string()];
        for (i, item) in stats.top_partners.iter().enumerate() {
            let escaped_name = html_escape::encode_text(&item.name);
            let quality_str = match item.avg_quality {
                Some(q) => format!(" (⭐ {:.1})", q),
                None => String::new(),
            };
            partner_lines.push(format!(
                "• {}. <b>{}</b>: {}x{}",
                i + 1,
                escaped_name,
                item.count,
                quality_str
            ));
        }
        result.push_str("\n\n");
        result.push_str(&partner_lines.join("\n"));
    }

    result
}

/// Escape a single CSV field value according to RFC 4180 rules.
pub fn escape_csv_field(val: &str) -> String {
    if val.contains(';') || val.contains('"') || val.contains('\n') || val.contains('\r') {
        let escaped = val.replace('"', "\"\"");
        format!("\"{}\"", escaped)
    } else {
        val.to_string()
    }
}

/// Generate a UTF-8 CSV file with semicolon delimiters and BOM (\u{FEFF}) for Excel.
pub fn generate_csv_export(entries: &[Entry]) -> Vec<u8> {
    let mut buffer = Vec::new();
    // Prepend UTF-8 BOM (\u{FEFF}) for German Excel
    buffer.extend_from_slice("\u{FEFF}".as_bytes());

    let header = "ID;Typ;Datum_Uhrzeit;Begleitung;Partner;Ort;Qualitaet;Notiz;Erstellt_Am\r\n";
    buffer.extend_from_slice(header.as_bytes());

    for entry in entries {
        let quality_str = entry.quality.map(|q| q.to_string()).unwrap_or_default();
        let line = format!(
            "{};{};{};{};{};{};{};{};{}\r\n",
            entry.id,
            escape_csv_field(&entry.r#type),
            escape_csv_field(&entry.timestamp),
            escape_csv_field(entry.company.as_deref().unwrap_or("")),
            escape_csv_field(entry.partner_name.as_deref().unwrap_or("")),
            escape_csv_field(entry.location.as_deref().unwrap_or("")),
            escape_csv_field(&quality_str),
            escape_csv_field(entry.description.as_deref().unwrap_or("")),
            escape_csv_field(&entry.created_at),
        );
        buffer.extend_from_slice(line.as_bytes());
    }

    buffer
}


