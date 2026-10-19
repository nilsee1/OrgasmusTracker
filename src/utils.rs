//! Helper utilities for date parsing, formatting, and HTML message construction.

use chrono::NaiveDateTime;
use crate::models::{DraftEntry, Entry, MAX_DESCRIPTION_LENGTH};

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

