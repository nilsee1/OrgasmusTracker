//! Main entrypoint for the Orgasmus-Tracker Bot.

use std::sync::Arc;
use teloxide::dispatching::dialogue::InMemStorage;
use teloxide::dispatching::UpdateFilterExt;
use teloxide::prelude::*;
use teloxide::types::{BotCommand, ParseMode};

use orgasmus_tracker::config::Config;
use orgasmus_tracker::database::Database;
use orgasmus_tracker::handlers::{
    handle_callback_query, handle_command, handle_datetime_message,
    handle_description_message, handle_partner_name_message, Command, State,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));
    log::info!("Starte Orgasmus-Tracker Telegram Bot in Rust...");

    let config = Arc::new(Config::from_env()?);
    let db = Arc::new(Database::connect(&config.db_path).await?);

    let bot = Bot::new(&config.bot_token);

    // Register bot commands in Telegram menu
    let bot_commands = vec![
        BotCommand::new("orgasmus", "Neuen Orgasmus erfassen"),
        BotCommand::new("liste", "Letzte 10 Einträge anzeigen"),
        BotCommand::new("hilfe", "Hilfe und Befehle anzeigen"),
        BotCommand::new("abbrechen", "Laufenden Vorgang abbrechen"),
    ];

    if let Err(e) = bot.set_my_commands(bot_commands).await {
        log::warn!("Konnte Bot-Befehle nicht registrieren: {}", e);
    } else {
        log::info!("Bot-Befehle erfolgreich im Telegram-Menü registriert.");
    }

    // Message handler tree
    let message_handler = Update::filter_message()
        // Authorization filter
        .filter_async({
            let config = Arc::clone(&config);
            move |msg: Message, bot: Bot| {
                let config = Arc::clone(&config);
                async move {
                    let user_id = msg.from.as_ref().map(|u| u.id.0 as i64);
                    if let Some(uid) = user_id {
                        if config.is_user_allowed(uid) {
                            return true;
                        }
                    }
                    let uid_str = user_id
                        .map(|id| id.to_string())
                        .unwrap_or_else(|| "Unbekannt".to_string());
                    let text = format!(
                        "⛔ <b>Zugriff verweigert</b>\n\nDeine Telegram-ID (<code>{}</code>) ist nicht autorisiert.",
                        html_escape::encode_text(&uid_str)
                    );
                    let _ = bot
                        .send_message(msg.chat.id, text)
                        .parse_mode(ParseMode::Html)
                        .await;
                    false
                }
            }
        })
        .branch(
            dptree::entry()
                .filter_command::<Command>()
                .endpoint(handle_command),
        )
        .branch(
            dptree::case![State::DateTime { draft, last_keyboard_msg_id }]
                .endpoint(handle_datetime_message),
        )
        .branch(
            dptree::case![State::PartnerName { draft, last_keyboard_msg_id }]
                .endpoint(handle_partner_name_message),
        )
        .branch(
            dptree::case![State::Description { draft, last_keyboard_msg_id }]
                .endpoint(handle_description_message),
        );

    // Callback query handler tree
    let callback_handler = Update::filter_callback_query()
        .filter_async({
            let config = Arc::clone(&config);
            move |q: teloxide::types::CallbackQuery, bot: Bot| {
                let config = Arc::clone(&config);
                async move {
                    let uid = q.from.id.0 as i64;
                    if config.is_user_allowed(uid) {
                        return true;
                    }
                    let _ = bot
                        .answer_callback_query(q.id)
                        .text("⛔ Zugriff verweigert: Deine ID ist nicht autorisiert.")
                        .show_alert(true)
                        .await;
                    false
                }
            }
        })
        .endpoint(handle_callback_query);

    let handler = dptree::entry()
        .enter_dialogue::<Update, InMemStorage<State>, State>()
        .branch(message_handler)
        .branch(callback_handler);

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![
            InMemStorage::<State>::new(),
            config,
            db
        ])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;

    Ok(())
}
