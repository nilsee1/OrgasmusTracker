//! Telegram Bot handlers, conversation state machine, and command dispatching.

use std::collections::BTreeSet;
use std::sync::Arc;

use chrono::{Datelike, TimeZone};
use teloxide::prelude::*;
use teloxide::types::{
    CallbackQuery, InlineKeyboardMarkup, InputFile, MessageId, ParseMode,
};
use teloxide::utils::command::BotCommands;

use crate::config::Config;
use crate::database::Database;
use crate::keyboards::*;
use crate::models::{
    DraftEntry, Entry, MAX_DESCRIPTION_LENGTH, MAX_LOCATION_LENGTH, MAX_PARTNER_NAME_LENGTH,
};
use crate::utils::{
    format_detailed_summary, format_entries_list_text, format_statistics_text,
    generate_csv_export, parse_german_datetime,
};

#[derive(BotCommands, Clone, Debug, PartialEq, Eq)]
#[command(rename_rule = "lowercase")]
pub enum Command {
    Start,
    Hilfe,
    Help,
    Orgasmus,
    Liste,
    Letzte,
    List,
    Statistik,
    Stats,
    Export,
    Abbrechen,
    Cancel,
}

#[derive(Clone, Default, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub enum State {
    #[default]
    Start,
    ChooseType {
        last_keyboard_msg_id: Option<i32>,
    },
    DateTime {
        draft: DraftEntry,
        last_keyboard_msg_id: Option<i32>,
    },
    Company {
        draft: DraftEntry,
        last_keyboard_msg_id: Option<i32>,
    },
    PartnerName {
        draft: DraftEntry,
        last_keyboard_msg_id: Option<i32>,
    },
    Location {
        draft: DraftEntry,
        last_keyboard_msg_id: Option<i32>,
    },
    Quality {
        draft: DraftEntry,
        last_keyboard_msg_id: Option<i32>,
    },
    Description {
        draft: DraftEntry,
        last_keyboard_msg_id: Option<i32>,
    },
}

impl State {
    pub fn last_keyboard_msg_id(&self) -> Option<i32> {
        match self {
            State::ChooseType { last_keyboard_msg_id }
            | State::DateTime { last_keyboard_msg_id, .. }
            | State::Company { last_keyboard_msg_id, .. }
            | State::PartnerName { last_keyboard_msg_id, .. }
            | State::Location { last_keyboard_msg_id, .. }
            | State::Quality { last_keyboard_msg_id, .. }
            | State::Description { last_keyboard_msg_id, .. } => *last_keyboard_msg_id,
            State::Start => None,
        }
    }
}

pub type MyDialogue = teloxide::dispatching::dialogue::Dialogue<
    State,
    teloxide::dispatching::dialogue::InMemStorage<State>,
>;
pub type HandlerResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

/// Helper to clear the previous inline keyboard from the chat if known.
pub async fn clear_previous_keyboard(
    bot: &Bot,
    chat_id: ChatId,
    msg_id: Option<i32>,
) {
    if let Some(mid) = msg_id {
        let _ = bot
            .edit_message_reply_markup(chat_id, MessageId(mid))
            .reply_markup(InlineKeyboardMarkup::default())
            .await;
    }
}

/// Handle top-level bot commands (/start, /orgasmus, /liste, /hilfe, /abbrechen).
pub async fn handle_command(
    bot: Bot,
    msg: Message,
    cmd: Command,
    dialogue: MyDialogue,
    config: Arc<Config>,
    db: Arc<Database>,
) -> HandlerResult {
    let user_id = msg.from.as_ref().map(|u| u.id.0 as i64).unwrap_or(0);
    let state = dialogue.get().await?.unwrap_or_default();
    clear_previous_keyboard(&bot, msg.chat.id, state.last_keyboard_msg_id()).await;

    match cmd {
        Command::Start => {
            dialogue.exit().await?;
            let first_name = msg
                .from
                .as_ref()
                .map(|u| u.first_name.as_str())
                .unwrap_or("Hallo");
            let text = format!(
                "👋 <b>Willkommen {}!</b>\n\n\
                Dies ist dein persönlicher, privater Orgasmus-Tracker.\n\n\
                <b>Verfügbare Befehle:</b>\n\
                • /orgasmus – Neuen Orgasmus erfassen (Einfach oder Ausführlich)\n\
                • /liste – Die letzten 10 Einträge anzeigen\n\
                • /statistik – Persönliche Auswertungen &amp; Kennzahlen anzeigen\n\
                • /export – Alle Einträge als CSV-Datei exportieren\n\
                • /hilfe – Befehlsübersicht &amp; Hilfe anzeigen\n\
                • /abbrechen – Laufenden Vorgang abbrechen",
                html_escape::encode_text(first_name)
            );
            bot.send_message(msg.chat.id, text)
                .parse_mode(ParseMode::Html)
                .await?;
        }
        Command::Hilfe | Command::Help => {
            let text = "ℹ️ <b>Orgasmus-Tracker Hilfe</b>\n\n\
                <b>Befehle:</b>\n\
                • <code>/orgasmus</code>: Startet die geführte Eingabe.\n\
                  - <b>Einfach:</b> Speichert sofort mit aktuellem Datum &amp; Uhrzeit.\n\
                  - <b>Ausführlich:</b> Schrittweise Erfassung von Zeit, Begleitung, Qualität (1-10) und Notizen.\n\
                  - In jedem Schritt kannst du mit <b>✅ Fertigstellen</b> vorzeitig speichern oder mit <b>❌ Abbruch</b> abbrechen.\n\n\
                • <code>/liste</code> (oder <code>/letzte</code>): Zeigt die letzten 10 protokollierten Einträge an.\n\
                • <code>/statistik</code> (oder <code>/stats</code>): Zeigt aggregierte Kennzahlen und Statistiken an.\n\
                • <code>/export</code>: Sendet alle deine Daten als CSV-Datei direkt in diesen Chat.\n\
                • <code>/abbrechen</code>: Bricht den aktuellen Dialog ab.\n\n\
                🔒 <i>Alle Daten werden lokal in deiner sicheren SQLite-Datenbank gespeichert.</i>";
            bot.send_message(msg.chat.id, text)
                .parse_mode(ParseMode::Html)
                .await?;
        }
        Command::Orgasmus => {
            let sent = bot
                .send_message(
                    msg.chat.id,
                    "Möchtest du einen einfachen oder ausführlichen Orgasmus eintragen?",
                )
                .reply_markup(get_choose_type_keyboard())
                .await?;
            dialogue
                .update(State::ChooseType {
                    last_keyboard_msg_id: Some(sent.id.0),
                })
                .await?;
        }
        Command::Liste | Command::Letzte | Command::List => {
            dialogue.exit().await?;
            let entries = db.get_last_entries(user_id, 10).await?;
            let text = format_entries_list_text(&entries);
            if entries.is_empty() {
                bot.send_message(msg.chat.id, text)
                    .parse_mode(ParseMode::Html)
                    .await?;
            } else {
                bot.send_message(msg.chat.id, text)
                    .parse_mode(ParseMode::Html)
                    .reply_markup(get_liste_main_keyboard())
                    .await?;
            }
        }
        Command::Statistik | Command::Stats => {
            dialogue.exit().await?;
            let stats = db.get_statistics(user_id).await?;
            let now = chrono::Utc::now().with_timezone(&config.timezone);
            let text = format_statistics_text(&stats, now.year());
            bot.send_message(msg.chat.id, text)
                .parse_mode(ParseMode::Html)
                .await?;
        }
        Command::Export => {
            dialogue.exit().await?;
            let entries = db.get_all_entries(user_id).await?;
            if entries.is_empty() {
                bot.send_message(
                    msg.chat.id,
                    "ℹ️ <b>Keine Daten vorhanden.</b>\n\nEs wurden noch keine Einträge zum Exportieren gefunden.",
                )
                .parse_mode(ParseMode::Html)
                .await?;
            } else {
                let csv_bytes = generate_csv_export(&entries);
                let now = chrono::Utc::now().with_timezone(&config.timezone);
                let filename = format!("orgasmus_tracker_export_{}.csv", now.format("%Y-%m-%d"));
                let caption = format!(
                    "📄 <b>Daten-Export</b>\n\nHier ist deine CSV-Datei mit allen {} erfassten Einträgen.",
                    entries.len()
                );
                bot.send_document(
                    msg.chat.id,
                    InputFile::memory(csv_bytes).file_name(filename),
                )
                .caption(caption)
                .parse_mode(ParseMode::Html)
                .await?;
            }
        }
        Command::Abbrechen | Command::Cancel => {
            dialogue.exit().await?;
            bot.send_message(msg.chat.id, "❌ Vorgang abgebrochen.")
                .await?;
        }
    }

    Ok(())
}

/// Helper to persist draft entry to database and format summary.
async fn persist_draft_and_send_summary(
    bot: &Bot,
    chat_id: ChatId,
    user_id: i64,
    draft: &DraftEntry,
    config: &Config,
    db: &Database,
) -> HandlerResult {
    let now = chrono::Utc::now().with_timezone(&config.timezone);
    let default_ts = now.format("%Y-%m-%d %H:%M").to_string();
    let timestamp = draft.timestamp.as_deref().unwrap_or(&default_ts);
    let created_at = now.to_rfc3339();

    db.add_entry(
        user_id,
        "detailed",
        timestamp,
        draft.company.as_deref(),
        draft.partner_name.as_deref(),
        draft.location.as_deref(),
        draft.quality,
        draft.description.as_deref(),
        Some(&created_at),
    )
    .await?;

    let summary = format_detailed_summary(draft);
    bot.send_message(chat_id, summary)
        .parse_mode(ParseMode::Html)
        .await?;

    Ok(())
}

/// Handle text message in State::DateTime.
pub async fn handle_datetime_message(
    bot: Bot,
    msg: Message,
    dialogue: MyDialogue,
    config: Arc<Config>,
    (draft, last_msg_id): (DraftEntry, Option<i32>),
) -> HandlerResult {
    clear_previous_keyboard(&bot, msg.chat.id, last_msg_id).await;
    let text = msg.text().unwrap_or("").trim();
    let parsed_dt = parse_german_datetime(text);
    let now = chrono::Utc::now().with_timezone(&config.timezone);

    if parsed_dt.is_none() {
        let sent = bot
            .send_message(
                msg.chat.id,
                format!(
                    "⚠️ <b>Ungültiges Datumsformat!</b>\n\n\
                    Bitte gib Datum und Uhrzeit im Format <code>TT.MM.JJJJ HH:MM</code> ein \
                    (z.B. <code>{}</code>) oder wähle 'Jetzt':",
                    now.format("%d.%m.%Y %H:%M")
                ),
            )
            .parse_mode(ParseMode::Html)
            .reply_markup(get_datetime_keyboard())
            .await?;

        dialogue
            .update(State::DateTime {
                draft,
                last_keyboard_msg_id: Some(sent.id.0),
            })
            .await?;
        return Ok(());
    }

    let dt = parsed_dt.unwrap();
    // Validate that the date is not in the future
    let dt_local = match config.timezone.from_local_datetime(&dt).single() {
        Some(local_dt) => local_dt,
        None => {
            // In case of daylight savings transition ambiguity, fallback to earliest
            config.timezone.from_local_datetime(&dt).earliest().unwrap_or(now)
        }
    };

    if dt_local > now {
        let sent = bot
            .send_message(
                msg.chat.id,
                "⚠️ <b>Ungültiges Datum!</b>\n\n\
                Das Datum darf nicht in der Zukunft liegen. Bitte gib ein vergangenes Datum ein oder wähle 'Jetzt':",
            )
            .parse_mode(ParseMode::Html)
            .reply_markup(get_datetime_keyboard())
            .await?;

        dialogue
            .update(State::DateTime {
                draft,
                last_keyboard_msg_id: Some(sent.id.0),
            })
            .await?;
        return Ok(());
    }

    let mut updated_draft = draft;
    updated_draft.timestamp = Some(dt.format("%Y-%m-%d %H:%M").to_string());

    let sent = bot
        .send_message(
            msg.chat.id,
            "👥 Warst du alleine oder mit einer anderen Person?",
        )
        .reply_markup(get_company_keyboard())
        .await?;

    dialogue
        .update(State::Company {
            draft: updated_draft,
            last_keyboard_msg_id: Some(sent.id.0),
        })
        .await?;

    Ok(())
}

/// Handle text message in State::PartnerName.
pub async fn handle_partner_name_message(
    bot: Bot,
    msg: Message,
    dialogue: MyDialogue,
    (draft, last_msg_id): (DraftEntry, Option<i32>),
) -> HandlerResult {
    clear_previous_keyboard(&bot, msg.chat.id, last_msg_id).await;
    let partner_name = msg.text().unwrap_or("").trim().to_string();

    if partner_name.chars().count() > MAX_PARTNER_NAME_LENGTH {
        let sent = bot
            .send_message(
                msg.chat.id,
                format!(
                    "⚠️ <b>Name zu lang!</b>\n\n\
                    Der Name darf maximal {} Zeichen lang sein (aktuell: {}). Bitte gib einen kürzeren Namen ein:",
                    MAX_PARTNER_NAME_LENGTH,
                    partner_name.chars().count()
                ),
            )
            .parse_mode(ParseMode::Html)
            .reply_markup(get_partner_name_keyboard())
            .await?;

        dialogue
            .update(State::PartnerName {
                draft,
                last_keyboard_msg_id: Some(sent.id.0),
            })
            .await?;
        return Ok(());
    }

    let mut updated_draft = draft;
    updated_draft.partner_name = Some(partner_name);

    let sent = bot
        .send_message(
            msg.chat.id,
            "📍 Wo fand es statt? Wähle einen Button oder gib den Ort als Text ein:",
        )
        .reply_markup(get_location_keyboard())
        .await?;

    dialogue
        .update(State::Location {
            draft: updated_draft,
            last_keyboard_msg_id: Some(sent.id.0),
        })
        .await?;

    Ok(())
}

/// Handle text message in State::Location.
pub async fn handle_location_message(
    bot: Bot,
    msg: Message,
    dialogue: MyDialogue,
    (draft, last_msg_id): (DraftEntry, Option<i32>),
) -> HandlerResult {
    clear_previous_keyboard(&bot, msg.chat.id, last_msg_id).await;
    let location_text = msg.text().unwrap_or("").trim().to_string();

    if location_text.chars().count() > MAX_LOCATION_LENGTH {
        let sent = bot
            .send_message(
                msg.chat.id,
                format!(
                    "⚠️ <b>Ort zu lang!</b>\n\n\
                    Der Ort darf maximal {} Zeichen lang sein (aktuell: {}). Bitte gib einen kürzeren Ort ein:",
                    MAX_LOCATION_LENGTH,
                    location_text.chars().count()
                ),
            )
            .parse_mode(ParseMode::Html)
            .reply_markup(get_location_keyboard())
            .await?;

        dialogue
            .update(State::Location {
                draft,
                last_keyboard_msg_id: Some(sent.id.0),
            })
            .await?;
        return Ok(());
    }

    let mut updated_draft = draft;
    if !location_text.is_empty() {
        updated_draft.location = Some(location_text);
    }

    let sent = bot
        .send_message(
            msg.chat.id,
            "⭐ Wie war die Qualität auf einer Skala von 1 bis 10 (10 = das Beste)?",
        )
        .reply_markup(get_quality_keyboard())
        .await?;

    dialogue
        .update(State::Quality {
            draft: updated_draft,
            last_keyboard_msg_id: Some(sent.id.0),
        })
        .await?;

    Ok(())
}

/// Handle text message in State::Description.
pub async fn handle_description_message(
    bot: Bot,
    msg: Message,
    dialogue: MyDialogue,
    config: Arc<Config>,
    db: Arc<Database>,
    (draft, last_msg_id): (DraftEntry, Option<i32>),
) -> HandlerResult {
    clear_previous_keyboard(&bot, msg.chat.id, last_msg_id).await;
    let description = msg.text().unwrap_or("").trim().to_string();

    if description.chars().count() > MAX_DESCRIPTION_LENGTH {
        let sent = bot
            .send_message(
                msg.chat.id,
                format!(
                    "⚠️ <b>Beschreibung zu lang!</b>\n\n\
                    Die Beschreibung darf maximal {} Zeichen lang sein (aktuell: {}). Bitte fasse dich etwas kürzer:",
                    MAX_DESCRIPTION_LENGTH,
                    description.chars().count()
                ),
            )
            .parse_mode(ParseMode::Html)
            .reply_markup(get_description_keyboard())
            .await?;

        dialogue
            .update(State::Description {
                draft,
                last_keyboard_msg_id: Some(sent.id.0),
            })
            .await?;
        return Ok(());
    }

    let mut updated_draft = draft;
    updated_draft.description = Some(description);

    let user_id = msg.from.as_ref().map(|u| u.id.0 as i64).unwrap_or(0);
    persist_draft_and_send_summary(&bot, msg.chat.id, user_id, &updated_draft, &config, &db).await?;
    dialogue.exit().await?;

    Ok(())
}

/// Handle all callback queries from inline buttons.
pub async fn handle_callback_query(
    bot: Bot,
    q: CallbackQuery,
    dialogue: MyDialogue,
    config: Arc<Config>,
    db: Arc<Database>,
) -> HandlerResult {
    let data = match q.data.as_deref() {
        Some(d) => d,
        None => return Ok(()),
    };

    let user_id = q.from.id.0 as i64;
    let reg_msg = match q.regular_message() {
        Some(m) => m,
        None => return Ok(()),
    };
    let chat_id = reg_msg.chat.id;
    let message_id = reg_msg.id;

    // 1. Navigation Cancel
    if data == ACTION_CANCEL {
        bot.answer_callback_query(q.id.clone()).await?;
        dialogue.exit().await?;
        bot.edit_message_text(chat_id, message_id, "❌ Eintrag abgebrochen.")
            .await?;
        return Ok(());
    }

    // 2. Navigation Finish
    if data == ACTION_FINISH {
        bot.answer_callback_query(q.id.clone()).await?;
        let current_state = dialogue.get().await?.unwrap_or_default();
        let draft = match current_state {
            State::DateTime { draft, .. }
            | State::Company { draft, .. }
            | State::PartnerName { draft, .. }
            | State::Location { draft, .. }
            | State::Quality { draft, .. }
            | State::Description { draft, .. } => draft,
            _ => DraftEntry {
                r#type: "detailed".to_string(),
                ..Default::default()
            },
        };

        let now = chrono::Utc::now().with_timezone(&config.timezone);
        let default_ts = now.format("%Y-%m-%d %H:%M").to_string();
        let timestamp = draft.timestamp.as_deref().unwrap_or(&default_ts);
        let created_at = now.to_rfc3339();

        db.add_entry(
            user_id,
            "detailed",
            timestamp,
            draft.company.as_deref(),
            draft.partner_name.as_deref(),
            draft.location.as_deref(),
            draft.quality,
            draft.description.as_deref(),
            Some(&created_at),
        )
        .await?;

        let summary = format_detailed_summary(&draft);
        dialogue.exit().await?;
        bot.edit_message_text(chat_id, message_id, summary)
            .parse_mode(ParseMode::Html)
            .await?;
        return Ok(());
    }

    // 3. Choose Type: Simple
    if data == TYPE_SIMPLE {
        bot.answer_callback_query(q.id.clone()).await?;
        let now = chrono::Utc::now().with_timezone(&config.timezone);
        let timestamp = now.format("%Y-%m-%d %H:%M").to_string();
        let created_at = now.to_rfc3339();

        db.add_entry(
            user_id,
            "simple",
            &timestamp,
            None,
            None,
            None,
            None,
            None,
            Some(&created_at),
        )
        .await?;

        dialogue.exit().await?;
        let text = format!(
            "✅ <b>Einfacher Orgasmus erfolgreich gespeichert!</b>\n\n\
            📅 <b>Datum &amp; Uhrzeit:</b> {}",
            html_escape::encode_text(&crate::utils::format_timestamp_display(&timestamp))
        );
        bot.edit_message_text(chat_id, message_id, text)
            .parse_mode(ParseMode::Html)
            .await?;
        return Ok(());
    }

    // 4. Choose Type: Detailed
    if data == TYPE_DETAILED {
        bot.answer_callback_query(q.id.clone()).await?;
        let now = chrono::Utc::now().with_timezone(&config.timezone);
        let draft = DraftEntry {
            r#type: "detailed".to_string(),
            timestamp: Some(now.format("%Y-%m-%d %H:%M").to_string()),
            ..Default::default()
        };

        let text = format!(
            "📅 Bitte gib Datum und Uhrzeit ein (Format: <code>TT.MM.JJJJ HH:MM</code>, \
            z.B. <code>{}</code>) oder wähle 'Jetzt':",
            now.format("%d.%m.%Y %H:%M")
        );

        bot.edit_message_text(chat_id, message_id, text)
            .parse_mode(ParseMode::Html)
            .reply_markup(get_datetime_keyboard())
            .await?;

        dialogue
            .update(State::DateTime {
                draft,
                last_keyboard_msg_id: Some(message_id.0),
            })
            .await?;
        return Ok(());
    }

    // 5. DateTime: Now
    if data == TIME_NOW {
        bot.answer_callback_query(q.id.clone()).await?;
        let current_state = dialogue.get().await?.unwrap_or_default();
        let mut draft = match current_state {
            State::DateTime { draft, .. } => draft,
            _ => DraftEntry {
                r#type: "detailed".to_string(),
                ..Default::default()
            },
        };

        let now = chrono::Utc::now().with_timezone(&config.timezone);
        draft.timestamp = Some(now.format("%Y-%m-%d %H:%M").to_string());

        bot.edit_message_text(
            chat_id,
            message_id,
            "👥 Warst du alleine oder mit einer anderen Person?",
        )
        .reply_markup(get_company_keyboard())
        .await?;

        dialogue
            .update(State::Company {
                draft,
                last_keyboard_msg_id: Some(message_id.0),
            })
            .await?;
        return Ok(());
    }

    // 6. Company: Alone
    if data == COMPANY_ALONE {
        bot.answer_callback_query(q.id.clone()).await?;
        let current_state = dialogue.get().await?.unwrap_or_default();
        let mut draft = match current_state {
            State::Company { draft, .. } => draft,
            _ => DraftEntry {
                r#type: "detailed".to_string(),
                ..Default::default()
            },
        };

        draft.company = Some("Alleine".to_string());
        draft.partner_name = None;

        bot.edit_message_text(
            chat_id,
            message_id,
            "📍 Wo fand es statt? Wähle einen Button oder gib den Ort als Text ein:",
        )
        .reply_markup(get_location_keyboard())
        .await?;

        dialogue
            .update(State::Location {
                draft,
                last_keyboard_msg_id: Some(message_id.0),
            })
            .await?;
        return Ok(());
    }

    // 7. Location: Nils or Mika
    if data == LOCATION_NILS || data == LOCATION_MIKA {
        bot.answer_callback_query(q.id.clone()).await?;
        let current_state = dialogue.get().await?.unwrap_or_default();
        let mut draft = match current_state {
            State::Location { draft, .. } => draft,
            _ => DraftEntry {
                r#type: "detailed".to_string(),
                ..Default::default()
            },
        };

        let loc_name = if data == LOCATION_NILS { "Nils" } else { "Mika" };
        draft.location = Some(loc_name.to_string());

        bot.edit_message_text(
            chat_id,
            message_id,
            "⭐ Wie war die Qualität auf einer Skala von 1 bis 10 (10 = das Beste)?",
        )
        .reply_markup(get_quality_keyboard())
        .await?;

        dialogue
            .update(State::Quality {
                draft,
                last_keyboard_msg_id: Some(message_id.0),
            })
            .await?;
        return Ok(());
    }

    // 8. Company: Partner
    if data == COMPANY_PARTNER {
        bot.answer_callback_query(q.id.clone()).await?;
        let current_state = dialogue.get().await?.unwrap_or_default();
        let mut draft = match current_state {
            State::Company { draft, .. } => draft,
            _ => DraftEntry {
                r#type: "detailed".to_string(),
                ..Default::default()
            },
        };

        draft.company = Some("Mit anderer Person".to_string());

        bot.edit_message_text(
            chat_id,
            message_id,
            "💬 Bitte gib den Namen der Person als Text ein:",
        )
        .reply_markup(get_partner_name_keyboard())
        .await?;

        dialogue
            .update(State::PartnerName {
                draft,
                last_keyboard_msg_id: Some(message_id.0),
            })
            .await?;
        return Ok(());
    }

    // 8. Quality Rating
    if data.starts_with(QUALITY_PREFIX) {
        bot.answer_callback_query(q.id.clone()).await?;
        if let Ok(quality_val) = data[QUALITY_PREFIX.len()..].parse::<i64>() {
            let current_state = dialogue.get().await?.unwrap_or_default();
            let mut draft = match current_state {
                State::Quality { draft, .. } => draft,
                _ => DraftEntry {
                    r#type: "detailed".to_string(),
                    ..Default::default()
                },
            };

            draft.quality = Some(quality_val);

            bot.edit_message_text(
                chat_id,
                message_id,
                "📝 Möchtest du noch eine Beschreibung hinzufügen? Schicke einfach eine Nachricht oder klicke auf Fertigstellen:",
            )
            .reply_markup(get_description_keyboard())
            .await?;

            dialogue
                .update(State::Description {
                    draft,
                    last_keyboard_msg_id: Some(message_id.0),
                })
                .await?;
        }
        return Ok(());
    }

    // =========================================================================
    // List & Deletion Handlers
    // =========================================================================
    if data == LIST_DELETE_MODE {
        bot.answer_callback_query(q.id.clone()).await?;
        let entries = db.get_last_entries(user_id, 10).await?;
        if entries.is_empty() {
            bot.edit_message_text(
                chat_id,
                message_id,
                format_entries_list_text(&[]),
            )
            .parse_mode(ParseMode::Html)
            .await?;
            return Ok(());
        }

        let selected = BTreeSet::new();
        let prompt = format!(
            "{}\n\n👇 <b>Wähle die Nummern der Einträge aus, die du löschen möchtest:</b>",
            format_entries_list_text(&entries)
        );

        bot.edit_message_text(chat_id, message_id, prompt)
            .parse_mode(ParseMode::Html)
            .reply_markup(get_liste_delete_select_keyboard(&entries, &selected))
            .await?;
        return Ok(());
    }

    if data.starts_with(LIST_TOGGLE_PREFIX) {
        let entry_id_str = &data[LIST_TOGGLE_PREFIX.len()..];
        if let Ok(entry_id) = entry_id_str.parse::<i64>() {
            bot.answer_callback_query(q.id.clone()).await?;
            let entries = db.get_last_entries(user_id, 10).await?;

            // Extract currently selected from the keyboard markup
            let mut selected = BTreeSet::new();
            if let Some(markup) = reg_msg.reply_markup() {
                for row in &markup.inline_keyboard {
                    for btn in row {
                        if btn.text.starts_with("☑️") {
                            if let teloxide::types::InlineKeyboardButtonKind::CallbackData(ref callback) = btn.kind {
                                if callback.starts_with(LIST_TOGGLE_PREFIX) {
                                    if let Ok(id) = callback[LIST_TOGGLE_PREFIX.len()..].parse::<i64>() {
                                        selected.insert(id);
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Toggle
            if selected.contains(&entry_id) {
                selected.remove(&entry_id);
            } else {
                selected.insert(entry_id);
            }

            // Keep only IDs that actually exist in entries
            let valid_ids: BTreeSet<i64> = entries.iter().map(|e| e.id).collect();
            selected = selected.intersection(&valid_ids).copied().collect();

            bot.edit_message_reply_markup(chat_id, message_id)
                .reply_markup(get_liste_delete_select_keyboard(&entries, &selected))
                .await?;
        }
        return Ok(());
    }

    if data == LIST_DELETE_CONFIRM {
        let entries = db.get_last_entries(user_id, 10).await?;
        let mut selected = BTreeSet::new();
        if let Some(markup) = reg_msg.reply_markup() {
            for row in &markup.inline_keyboard {
                for btn in row {
                    if btn.text.starts_with("☑️") {
                        if let teloxide::types::InlineKeyboardButtonKind::CallbackData(ref callback) = btn.kind {
                            if callback.starts_with(LIST_TOGGLE_PREFIX) {
                                if let Ok(id) = callback[LIST_TOGGLE_PREFIX.len()..].parse::<i64>() {
                                    selected.insert(id);
                                }
                            }
                        }
                    }
                }
            }
        }

        if selected.is_empty() {
            bot.answer_callback_query(q.id.clone())
                .text("Bitte wähle mindestens einen Eintrag aus.")
                .show_alert(true)
                .await?;
            return Ok(());
        }

        bot.answer_callback_query(q.id.clone()).await?;
        let to_delete: Vec<&Entry> = entries.iter().filter(|e| selected.contains(&e.id)).collect();

        if to_delete.is_empty() {
            bot.edit_message_text(
                chat_id,
                message_id,
                format_entries_list_text(&entries),
            )
            .parse_mode(ParseMode::Html)
            .reply_markup(get_liste_main_keyboard())
            .await?;
            return Ok(());
        }

        let mut lines = vec![
            "⚠️ <b>Sicherheitsabfrage</b>\n".to_string(),
            format!(
                "Möchtest du folgende(n) <b>{}</b> Eintrag/Einträge wirklich unwiderruflich löschen?\n",
                to_delete.len()
            ),
        ];

        for e in to_delete {
            let ts_fmt = crate::utils::format_timestamp_display(&e.timestamp);
            let etype = if e.r#type == "detailed" {
                "Ausführlich"
            } else {
                "Einfach"
            };
            let mut extra = String::new();
            if let Some(partner) = &e.partner_name {
                extra = format!(" (mit {})", html_escape::encode_text(partner));
            } else if let Some(company) = &e.company {
                extra = format!(" ({})", html_escape::encode_text(company));
            }
            lines.push(format!(
                "• 📅 <b>{}</b> – <i>{}</i>{}",
                html_escape::encode_text(&ts_fmt),
                etype,
                extra
            ));
        }

        bot.edit_message_text(chat_id, message_id, lines.join("\n"))
            .parse_mode(ParseMode::Html)
            .reply_markup(get_liste_delete_confirm_keyboard())
            .await?;
        return Ok(());
    }

    if data == LIST_DELETE_EXECUTE {
        bot.answer_callback_query(q.id.clone()).await?;

        let entries = db.get_last_entries(user_id, 10).await?;
        let mut ids_to_delete = Vec::new();
        if let Some(text) = reg_msg.text() {
            for entry in &entries {
                let ts_fmt = crate::utils::format_timestamp_display(&entry.timestamp);
                if text.contains(&ts_fmt) {
                    ids_to_delete.push(entry.id);
                }
            }
        }

        let deleted_count = db.delete_entries(&ids_to_delete, user_id).await?;

        let remaining_entries = db.get_last_entries(user_id, 10).await?;
        if remaining_entries.is_empty() {
            bot.edit_message_text(
                chat_id,
                message_id,
                format_entries_list_text(&[]),
            )
            .parse_mode(ParseMode::Html)
            .await?;
        } else {
            let new_text = format!(
                "✅ <b>{} Eintrag/Einträge erfolgreich gelöscht.</b>\n\n{}",
                deleted_count,
                format_entries_list_text(&remaining_entries)
            );
            bot.edit_message_text(chat_id, message_id, new_text)
                .parse_mode(ParseMode::Html)
                .reply_markup(get_liste_main_keyboard())
                .await?;
        }
        return Ok(());
    }

    if data == LIST_DELETE_CANCEL {
        bot.answer_callback_query(q.id.clone()).await?;
        let entries = db.get_last_entries(user_id, 10).await?;
        if entries.is_empty() {
            bot.edit_message_text(
                chat_id,
                message_id,
                format_entries_list_text(&[]),
            )
            .parse_mode(ParseMode::Html)
            .await?;
        } else {
            bot.edit_message_text(
                chat_id,
                message_id,
                format_entries_list_text(&entries),
            )
            .parse_mode(ParseMode::Html)
            .reply_markup(get_liste_main_keyboard())
            .await?;
        }
        return Ok(());
    }

    bot.answer_callback_query(q.id).await?;
    Ok(())
}

