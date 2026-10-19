"""Telegram Orgasmus-Tracker Bot.

Production-ready implementation using python-telegram-bot v21+ with stateful
conversation flow, SQLite persistence, German UI, and strict authorization filters.
"""

import html
import logging
import os
import re
import sys
from datetime import datetime
from functools import wraps
from pathlib import Path
from typing import Any

# Ensure project root is in sys.path
PROJECT_ROOT = Path(__file__).resolve().parent.parent
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

from telegram import (
    InlineKeyboardButton,
    InlineKeyboardMarkup,
    Update,
)
from telegram.constants import ParseMode
from telegram.ext import (
    Application,
    CallbackQueryHandler,
    CommandHandler,
    ContextTypes,
    ConversationHandler,
    MessageHandler,
    filters,
)

from src.config import (
    BOT_TZ,
    DB_PATH,
    TELEGRAM_BOT_TOKEN,
    is_user_allowed,
    validate_config,
)
from src.database import Database

# Configure structured logging
logging.basicConfig(
    format="%(asctime)s - %(name)s - %(levelname)s - %(message)s",
    level=logging.INFO,
)
logger = logging.getLogger(__name__)

# Initialize database instance
db = Database(db_path=DB_PATH)

# ==============================================================================
# Conversation States
# ==============================================================================
(
    STATE_CHOOSE_TYPE,
    STATE_DATETIME,
    STATE_COMPANY,
    STATE_PARTNER_NAME,
    STATE_QUALITY,
    STATE_DESCRIPTION,
) = range(1, 7)

# ==============================================================================
# Callback Data Constants
# ==============================================================================
TYPE_SIMPLE = "TYPE_SIMPLE"
TYPE_DETAILED = "TYPE_DETAILED"

TIME_NOW = "TIME_NOW"

COMPANY_ALONE = "COMPANY_ALONE"
COMPANY_PARTNER = "COMPANY_PARTNER"

QUALITY_PREFIX = "QUALITY_"

ACTION_FINISH = "ACTION_FINISH"
ACTION_CANCEL = "ACTION_CANCEL"


# ==============================================================================
# Security Decorator & Helpers
# ==============================================================================
def restricted(func):
    """Decorator to restrict handler execution strictly to ALLOWED_USER_IDS."""

    @wraps(func)
    async def wrapper(update: Update, context: ContextTypes.DEFAULT_TYPE, *args, **kwargs):
        user = update.effective_user
        if not user or not is_user_allowed(user.id):
            uid = user.id if user else "Unbekannt"
            logger.warning("Unberechtigter Zugriffsversuch von Telegram-User-ID: %s", uid)

            if update.callback_query:
                await update.callback_query.answer(
                    "⛔ Zugriff verweigert: Deine ID ist nicht autorisiert.",
                    show_alert=True,
                )
            elif update.effective_message:
                await update.effective_message.reply_text(
                    "⛔ <b>Zugriff verweigert</b>\n\n"
                    f"Deine Telegram-ID (<code>{html.escape(str(uid))}</code>) ist nicht autorisiert.",
                    parse_mode=ParseMode.HTML,
                )
            return ConversationHandler.END

        return await func(update, context, *args, **kwargs)

    return wrapper


def format_timestamp_display(ts_str: str) -> str:
    """Format an ISO or standard timestamp string into a readable German date & time."""
    if not ts_str:
        return ""

    formats = [
        "%Y-%m-%d %H:%M",
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%dT%H:%M:%S%z",
        "%d.%m.%Y %H:%M",
    ]

    for fmt in formats:
        try:
            # Handle potential ISO format with timezone or subseconds if present
            clean_ts = ts_str.split(".")[0] if "." in ts_str and "T" in ts_str else ts_str
            dt = datetime.strptime(clean_ts, fmt)
            return dt.strftime("%d.%m.%Y um %H:%M Uhr")
        except ValueError:
            continue

    return ts_str


def parse_german_datetime(input_text: str) -> datetime | None:
    """Parse text input matching 'DD.MM.YYYY HH:MM'."""
    text = input_text.strip()
    try:
        return datetime.strptime(text, "%d.%m.%Y %H:%M")
    except ValueError:
        pass

    # Secondary fallback for common German typos or seconds:
    try:
        return datetime.strptime(text, "%d.%m.%Y %H:%M:%S")
    except ValueError:
        return None


# ==============================================================================
# Inline Keyboards
# ==============================================================================
def get_choose_type_keyboard() -> InlineKeyboardMarkup:
    """Keyboard for selecting simple or detailed tracking."""
    return InlineKeyboardMarkup(
        [
            [InlineKeyboardButton("⚡ Einfacher Orgasmus", callback_data=TYPE_SIMPLE)],
            [InlineKeyboardButton("📝 Ausführlicher Orgasmus", callback_data=TYPE_DETAILED)],
            [InlineKeyboardButton("❌ Abbruch", callback_data=ACTION_CANCEL)],
        ]
    )


def get_datetime_keyboard() -> InlineKeyboardMarkup:
    """Keyboard for datetime step."""
    return InlineKeyboardMarkup(
        [
            [InlineKeyboardButton("🕒 Jetzt (Aktuelle Zeit)", callback_data=TIME_NOW)],
            [
                InlineKeyboardButton("✅ Fertigstellen", callback_data=ACTION_FINISH),
                InlineKeyboardButton("❌ Abbruch", callback_data=ACTION_CANCEL),
            ],
        ]
    )


def get_company_keyboard() -> InlineKeyboardMarkup:
    """Keyboard for company step."""
    return InlineKeyboardMarkup(
        [
            [
                InlineKeyboardButton("👤 Alleine", callback_data=COMPANY_ALONE),
                InlineKeyboardButton("👥 Mit anderer Person", callback_data=COMPANY_PARTNER),
            ],
            [
                InlineKeyboardButton("✅ Fertigstellen", callback_data=ACTION_FINISH),
                InlineKeyboardButton("❌ Abbruch", callback_data=ACTION_CANCEL),
            ],
        ]
    )


def get_partner_name_keyboard() -> InlineKeyboardMarkup:
    """Keyboard for partner name input step."""
    return InlineKeyboardMarkup(
        [
            [
                InlineKeyboardButton("✅ Fertigstellen", callback_data=ACTION_FINISH),
                InlineKeyboardButton("❌ Abbruch", callback_data=ACTION_CANCEL),
            ]
        ]
    )


def get_quality_keyboard() -> InlineKeyboardMarkup:
    """Keyboard with numbers 1 to 10 arranged in two rows, plus action buttons."""
    row_1 = [
        InlineKeyboardButton(f"{i}", callback_data=f"{QUALITY_PREFIX}{i}")
        for i in range(1, 6)
    ]
    row_2 = [
        InlineKeyboardButton(f"{i}", callback_data=f"{QUALITY_PREFIX}{i}")
        for i in range(6, 11)
    ]
    actions = [
        InlineKeyboardButton("✅ Fertigstellen", callback_data=ACTION_FINISH),
        InlineKeyboardButton("❌ Abbruch", callback_data=ACTION_CANCEL),
    ]
    return InlineKeyboardMarkup([row_1, row_2, actions])


def get_description_keyboard() -> InlineKeyboardMarkup:
    """Keyboard for final description step."""
    return InlineKeyboardMarkup(
        [
            [
                InlineKeyboardButton(
                    "✅ Fertigstellen (Ohne Beschreibung)",
                    callback_data=ACTION_FINISH,
                )
            ],
            [InlineKeyboardButton("❌ Abbruch", callback_data=ACTION_CANCEL)],
        ]
    )


# ==============================================================================
# Helper to Format and Save Draft
# ==============================================================================
def format_detailed_summary(
    timestamp: str,
    company: str | None,
    partner_name: str | None,
    quality: int | None,
    description: str | None,
) -> str:
    """Build a clean German HTML summary of a saved detailed entry."""
    formatted_ts = format_timestamp_display(timestamp)
    lines = [
        "✅ <b>Ausführlicher Orgasmus erfolgreich gespeichert!</b>",
        "",
        f"📅 <b>Datum &amp; Uhrzeit:</b> {html.escape(formatted_ts)}",
    ]

    if company == "Alleine":
        lines.append("👥 <b>Begleitung:</b> Alleine")
    elif company == "Mit anderer Person":
        if partner_name:
            lines.append(f"👥 <b>Begleitung:</b> Mit anderer Person ({html.escape(partner_name)})")
        else:
            lines.append("👥 <b>Begleitung:</b> Mit anderer Person")
    else:
        lines.append("👥 <b>Begleitung:</b> <i>Keine Angabe</i>")

    if quality is not None:
        lines.append(f"⭐ <b>Qualität:</b> {quality}/10")
    else:
        lines.append("⭐ <b>Qualität:</b> <i>Keine Angabe</i>")

    if description:
        lines.append(f"📝 <b>Beschreibung:</b> {html.escape(description)}")
    else:
        lines.append("📝 <b>Beschreibung:</b> <i>Keine</i>")

    return "\n".join(lines)


async def persist_draft_and_send_summary(
    update: Update,
    context: ContextTypes.DEFAULT_TYPE,
    is_callback: bool,
) -> None:
    """Persist the collected draft data into SQLite and send confirmation to user."""
    draft = context.user_data.get("draft", {})
    user = update.effective_user
    if not user:
        return

    now = datetime.now(BOT_TZ)
    timestamp = draft.get("timestamp") or now.strftime("%Y-%m-%d %H:%M")
    company = draft.get("company")
    partner_name = draft.get("partner_name")
    quality = draft.get("quality")
    description = draft.get("description")

    await db.async_add_entry(
        user_id=user.id,
        type="detailed",
        timestamp=timestamp,
        company=company,
        partner_name=partner_name,
        quality=quality,
        description=description,
        created_at=now.isoformat(),
    )

    summary = format_detailed_summary(
        timestamp=timestamp,
        company=company,
        partner_name=partner_name,
        quality=quality,
        description=description,
    )

    context.user_data.clear()

    if is_callback and update.callback_query:
        await update.callback_query.edit_message_text(
            summary,
            parse_mode=ParseMode.HTML,
        )
    elif update.effective_message:
        await update.effective_message.reply_text(
            summary,
            parse_mode=ParseMode.HTML,
        )


# ==============================================================================
# Conversation Handler Callbacks
# ==============================================================================
@restricted
async def start_orgasmus(update: Update, context: ContextTypes.DEFAULT_TYPE) -> int:
    """Trigger the tracking flow on /orgasmus."""
    context.user_data.clear()
    text = "Möchtest du einen einfachen oder ausführlichen Orgasmus eintragen?"

    if update.callback_query:
        await update.callback_query.answer()
        await update.callback_query.edit_message_text(
            text,
            reply_markup=get_choose_type_keyboard(),
        )
    elif update.effective_message:
        await update.effective_message.reply_text(
            text,
            reply_markup=get_choose_type_keyboard(),
        )
    return STATE_CHOOSE_TYPE


@restricted
async def handle_type_simple(update: Update, context: ContextTypes.DEFAULT_TYPE) -> int:
    """Handle choice: 'Einfacher Orgasmus'."""
    query = update.callback_query
    await query.answer()

    user = update.effective_user
    now = datetime.now(BOT_TZ)
    timestamp = now.strftime("%Y-%m-%d %H:%M")

    await db.async_add_entry(
        user_id=user.id,
        type="simple",
        timestamp=timestamp,
        created_at=now.isoformat(),
    )

    context.user_data.clear()
    formatted_ts = format_timestamp_display(timestamp)

    await query.edit_message_text(
        "✅ <b>Einfacher Orgasmus wurde mit aktuellem Datum &amp; Uhrzeit gespeichert!</b>\n\n"
        f"📅 <b>Zeitpunkt:</b> {html.escape(formatted_ts)}",
        parse_mode=ParseMode.HTML,
    )
    return ConversationHandler.END


@restricted
async def handle_type_detailed(update: Update, context: ContextTypes.DEFAULT_TYPE) -> int:
    """Handle choice: 'Ausführlicher Orgasmus'."""
    query = update.callback_query
    await query.answer()

    now = datetime.now(BOT_TZ)
    # Initialize draft with default timestamp so early 'Fertigstellen' has a valid date
    context.user_data["draft"] = {
        "type": "detailed",
        "timestamp": now.strftime("%Y-%m-%d %H:%M"),
        "company": None,
        "partner_name": None,
        "quality": None,
        "description": None,
    }

    await query.edit_message_text(
        "📅 Bitte gib Datum und Uhrzeit ein (Format: <code>TT.MM.JJJJ HH:MM</code>, "
        f"z.B. <code>{now.strftime('%d.%m.%Y %H:%M')}</code>) oder wähle 'Jetzt':",
        reply_markup=get_datetime_keyboard(),
        parse_mode=ParseMode.HTML,
    )
    return STATE_DATETIME


@restricted
async def handle_datetime_now(update: Update, context: ContextTypes.DEFAULT_TYPE) -> int:
    """Handle click on 'Jetzt (Aktuelle Zeit)'."""
    query = update.callback_query
    await query.answer()

    now = datetime.now(BOT_TZ)
    draft = context.user_data.setdefault("draft", {})
    draft["timestamp"] = now.strftime("%Y-%m-%d %H:%M")

    await query.edit_message_text(
        "👥 Warst du alleine oder mit einer anderen Person?",
        reply_markup=get_company_keyboard(),
    )
    return STATE_COMPANY


@restricted
async def handle_datetime_text(update: Update, context: ContextTypes.DEFAULT_TYPE) -> int:
    """Handle manual text input for date and time."""
    text = update.message.text.strip()
    dt = parse_german_datetime(text)

    if not dt:
        now = datetime.now(BOT_TZ)
        await update.message.reply_text(
            "⚠️ <b>Ungültiges Datumsformat!</b>\n\n"
            "Bitte gib Datum und Uhrzeit im Format <code>TT.MM.JJJJ HH:MM</code> ein "
            f"(z.B. <code>{now.strftime('%d.%m.%Y %H:%M')}</code>) oder wähle 'Jetzt':",
            reply_markup=get_datetime_keyboard(),
            parse_mode=ParseMode.HTML,
        )
        return STATE_DATETIME

    draft = context.user_data.setdefault("draft", {})
    draft["timestamp"] = dt.strftime("%Y-%m-%d %H:%M")

    await update.message.reply_text(
        "👥 Warst du alleine oder mit einer anderen Person?",
        reply_markup=get_company_keyboard(),
    )
    return STATE_COMPANY


@restricted
async def handle_company_alone(update: Update, context: ContextTypes.DEFAULT_TYPE) -> int:
    """Handle company selection: 'Alleine'."""
    query = update.callback_query
    await query.answer()

    draft = context.user_data.setdefault("draft", {})
    draft["company"] = "Alleine"
    draft["partner_name"] = None

    await query.edit_message_text(
        "⭐ Wie war die Qualität auf einer Skala von 1 bis 10 (10 = das Beste)?",
        reply_markup=get_quality_keyboard(),
    )
    return STATE_QUALITY


@restricted
async def handle_company_partner(update: Update, context: ContextTypes.DEFAULT_TYPE) -> int:
    """Handle company selection: 'Mit anderer Person'."""
    query = update.callback_query
    await query.answer()

    draft = context.user_data.setdefault("draft", {})
    draft["company"] = "Mit anderer Person"

    await query.edit_message_text(
        "💬 Bitte gib den Namen der Person als Text ein:",
        reply_markup=get_partner_name_keyboard(),
    )
    return STATE_PARTNER_NAME


@restricted
async def handle_partner_name_text(update: Update, context: ContextTypes.DEFAULT_TYPE) -> int:
    """Save partner name from text message."""
    partner_name = update.message.text.strip()
    draft = context.user_data.setdefault("draft", {})
    draft["partner_name"] = partner_name

    await update.message.reply_text(
        "⭐ Wie war die Qualität auf einer Skala von 1 bis 10 (10 = das Beste)?",
        reply_markup=get_quality_keyboard(),
    )
    return STATE_QUALITY


@restricted
async def handle_quality_choice(update: Update, context: ContextTypes.DEFAULT_TYPE) -> int:
    """Handle quality rating (1-10) click."""
    query = update.callback_query
    await query.answer()

    # Extract quality number from callback_data (e.g. QUALITY_8)
    match = re.search(r"^QUALITY_(\d+)$", query.data)
    if match:
        quality_val = int(match.group(1))
        draft = context.user_data.setdefault("draft", {})
        draft["quality"] = quality_val

    await query.edit_message_text(
        "📝 Möchtest du noch eine Beschreibung hinzufügen? Schicke einfach eine Nachricht oder klicke auf Fertigstellen:",
        reply_markup=get_description_keyboard(),
    )
    return STATE_DESCRIPTION


@restricted
async def handle_description_text(update: Update, context: ContextTypes.DEFAULT_TYPE) -> int:
    """Handle optional description input, persist entry, and end conversation."""
    description = update.message.text.strip()
    draft = context.user_data.setdefault("draft", {})
    draft["description"] = description

    await persist_draft_and_send_summary(update, context, is_callback=False)
    return ConversationHandler.END


# ==============================================================================
# Global Navigation Actions (Finish & Cancel)
# ==============================================================================
@restricted
async def handle_action_finish(update: Update, context: ContextTypes.DEFAULT_TYPE) -> int:
    """Global navigation: Finish immediately with collected draft data."""
    query = update.callback_query
    if query:
        await query.answer()
        await persist_draft_and_send_summary(update, context, is_callback=True)
    elif update.effective_message:
        await persist_draft_and_send_summary(update, context, is_callback=False)

    return ConversationHandler.END


@restricted
async def handle_action_cancel(update: Update, context: ContextTypes.DEFAULT_TYPE) -> int:
    """Global navigation: Discard draft and cancel conversation."""
    context.user_data.clear()
    query = update.callback_query

    if query:
        await query.answer()
        await query.edit_message_text("❌ Eintrag abgebrochen.")
    elif update.effective_message:
        await update.effective_message.reply_text("❌ Eintrag abgebrochen.")

    return ConversationHandler.END


@restricted
async def handle_cancel_command(update: Update, context: ContextTypes.DEFAULT_TYPE) -> int:
    """Handle /cancel or /abbrechen command."""
    context.user_data.clear()
    if update.effective_message:
        await update.effective_message.reply_text("❌ Vorgang abgebrochen.")
    return ConversationHandler.END


# ==============================================================================
# General & History Commands
# ==============================================================================
@restricted
async def handle_start(update: Update, context: ContextTypes.DEFAULT_TYPE) -> None:
    """Send welcome and quick-start message."""
    user = update.effective_user
    first_name = html.escape(user.first_name) if user and user.first_name else "Hallo"

    text = (
        f"👋 <b>Willkommen {first_name}!</b>\n\n"
        "Dies ist dein persönlicher, privater Orgasmus-Tracker.\n\n"
        "<b>Verfügbare Befehle:</b>\n"
        "• /orgasmus – Neuen Orgasmus erfassen (Einfach oder Ausführlich)\n"
        "• /liste – Die letzten 10 Einträge anzeigen\n"
        "• /hilfe – Befehlsübersicht & Hilfe anzeigen\n"
        "• /abbrechen – Laufenden Vorgang abbrechen"
    )
    await update.effective_message.reply_text(text, parse_mode=ParseMode.HTML)


@restricted
async def handle_help(update: Update, context: ContextTypes.DEFAULT_TYPE) -> None:
    """Send help message."""
    text = (
        "ℹ️ <b>Orgasmus-Tracker Hilfe</b>\n\n"
        "<b>Befehle:</b>\n"
        "• <code>/orgasmus</code>: Startet die geführte Eingabe.\n"
        "  - <b>Einfach:</b> Speichert sofort mit aktuellem Datum &amp; Uhrzeit.\n"
        "  - <b>Ausführlich:</b> Schrittweise Erfassung von Zeit, Begleitung, Qualität (1-10) und Notizen.\n"
        "  - In jedem Schritt kannst du mit <b>✅ Fertigstellen</b> vorzeitig speichern oder mit <b>❌ Abbruch</b> abbrechen.\n\n"
        "• <code>/liste</code> (oder <code>/letzte</code>): Zeigt die letzten 10 protokollierten Einträge an.\n"
        "• <code>/abbrechen</code>: Bricht den aktuellen Dialog ab.\n\n"
        "🔒 <i>Alle Daten werden lokal in deiner sicheren SQLite-Datenbank gespeichert.</i>"
    )
    await update.effective_message.reply_text(text, parse_mode=ParseMode.HTML)


@restricted
async def handle_liste(update: Update, context: ContextTypes.DEFAULT_TYPE) -> None:
    """Retrieve and display the last 10 entries for the requesting user."""
    user = update.effective_user
    if not user:
        return

    entries = await db.async_get_last_entries(user_id=user.id, limit=10)
    if not entries:
        await update.effective_message.reply_text(
            "ℹ️ <b>Bisher sind noch keine Einträge vorhanden.</b>\n\n"
            "Nutze /orgasmus, um deinen ersten Eintrag zu erstellen.",
            parse_mode=ParseMode.HTML,
        )
        return

    count = len(entries)
    text_blocks = [f"📊 <b>Deine letzten {count} Einträge:</b>\n"]

    for idx, entry in enumerate(entries, start=1):
        ts_formatted = format_timestamp_display(entry["timestamp"])
        entry_type = "Ausführlich" if entry["type"] == "detailed" else "Einfach"

        block_lines = [
            f"<b>{idx}.</b> 📅 <b>{html.escape(ts_formatted)}</b> (<i>{entry_type}</i>)"
        ]

        if entry["type"] == "detailed":
            if entry["company"]:
                if entry["company"] == "Mit anderer Person" and entry.get("partner_name"):
                    block_lines.append(
                        f"   👥 Begleitung: Mit {html.escape(entry['partner_name'])}"
                    )
                else:
                    block_lines.append(f"   👥 Begleitung: {html.escape(entry['company'])}")

            if entry.get("quality") is not None:
                block_lines.append(f"   ⭐ Qualität: {entry['quality']}/10")

            if entry.get("description"):
                block_lines.append(f"   📝 Notiz: {html.escape(entry['description'])}")

        text_blocks.append("\n".join(block_lines))

    await update.effective_message.reply_text(
        "\n\n".join(text_blocks),
        parse_mode=ParseMode.HTML,
    )


async def error_handler(update: object, context: ContextTypes.DEFAULT_TYPE) -> None:
    """Log errors caused by updates."""
    logger.error("Exception während der Update-Verarbeitung aufgetreten:", exc_info=context.error)


async def post_init(application: Application) -> None:
    """Set bot commands menu on startup and initialize database."""
    await db.async_init_db()

    commands = [
        ("orgasmus", "Neuen Orgasmus erfassen"),
        ("liste", "Letzte 10 Einträge anzeigen"),
        ("hilfe", "Hilfe und Befehle anzeigen"),
        ("abbrechen", "Laufenden Vorgang abbrechen"),
    ]
    try:
        await application.bot.set_my_commands(commands)
        logger.info("Bot-Befehle erfolgreich im Telegram-Menü registriert.")
    except Exception as e:
        logger.warning("Konnte Bot-Befehle nicht registrieren: %s", e)


# ==============================================================================
# Application Factory
# ==============================================================================
def create_application() -> Application:
    """Build and configure the Telegram application."""
    validate_config()

    application = (
        Application.builder()
        .token(TELEGRAM_BOT_TOKEN)
        .post_init(post_init)
        .build()
    )

    # Conversation handler for /orgasmus
    conv_handler = ConversationHandler(
        entry_points=[
            CommandHandler(["orgasmus", "Orgasmus", "ORGASMUS"], start_orgasmus),
        ],
        states={
            STATE_CHOOSE_TYPE: [
                CallbackQueryHandler(handle_type_simple, pattern=f"^{TYPE_SIMPLE}$"),
                CallbackQueryHandler(handle_type_detailed, pattern=f"^{TYPE_DETAILED}$"),
                CallbackQueryHandler(handle_action_cancel, pattern=f"^{ACTION_CANCEL}$"),
            ],
            STATE_DATETIME: [
                CallbackQueryHandler(handle_datetime_now, pattern=f"^{TIME_NOW}$"),
                CallbackQueryHandler(handle_action_finish, pattern=f"^{ACTION_FINISH}$"),
                CallbackQueryHandler(handle_action_cancel, pattern=f"^{ACTION_CANCEL}$"),
                MessageHandler(filters.TEXT & ~filters.COMMAND, handle_datetime_text),
            ],
            STATE_COMPANY: [
                CallbackQueryHandler(handle_company_alone, pattern=f"^{COMPANY_ALONE}$"),
                CallbackQueryHandler(handle_company_partner, pattern=f"^{COMPANY_PARTNER}$"),
                CallbackQueryHandler(handle_action_finish, pattern=f"^{ACTION_FINISH}$"),
                CallbackQueryHandler(handle_action_cancel, pattern=f"^{ACTION_CANCEL}$"),
            ],
            STATE_PARTNER_NAME: [
                CallbackQueryHandler(handle_action_finish, pattern=f"^{ACTION_FINISH}$"),
                CallbackQueryHandler(handle_action_cancel, pattern=f"^{ACTION_CANCEL}$"),
                MessageHandler(filters.TEXT & ~filters.COMMAND, handle_partner_name_text),
            ],
            STATE_QUALITY: [
                CallbackQueryHandler(
                    handle_quality_choice,
                    pattern=f"^{QUALITY_PREFIX}([1-9]|10)$",
                ),
                CallbackQueryHandler(handle_action_finish, pattern=f"^{ACTION_FINISH}$"),
                CallbackQueryHandler(handle_action_cancel, pattern=f"^{ACTION_CANCEL}$"),
            ],
            STATE_DESCRIPTION: [
                CallbackQueryHandler(handle_action_finish, pattern=f"^{ACTION_FINISH}$"),
                CallbackQueryHandler(handle_action_cancel, pattern=f"^{ACTION_CANCEL}$"),
                MessageHandler(filters.TEXT & ~filters.COMMAND, handle_description_text),
            ],
        },
        fallbacks=[
            CallbackQueryHandler(handle_action_finish, pattern=f"^{ACTION_FINISH}$"),
            CallbackQueryHandler(handle_action_cancel, pattern=f"^{ACTION_CANCEL}$"),
            CommandHandler(["cancel", "Cancel", "abbrechen", "Abbrechen"], handle_cancel_command),
        ],
        allow_reentry=True,
    )

    # Register handlers
    application.add_handler(conv_handler)
    application.add_handler(CommandHandler(["start", "Start"], handle_start))
    application.add_handler(CommandHandler(["help", "Help", "hilfe", "Hilfe"], handle_help))
    application.add_handler(
        CommandHandler(
            ["liste", "Liste", "letzte", "Letzte", "list", "List"],
            handle_liste,
        )
    )
    application.add_handler(
        CommandHandler(["cancel", "Cancel", "abbrechen", "Abbrechen"], handle_cancel_command)
    )

    # Add error handler
    application.add_error_handler(error_handler)

    return application


def main() -> None:
    """Start the bot in polling mode."""
    logger.info("Starte Orgasmus-Tracker Telegram Bot...")
    app = create_application()
    app.run_polling(drop_pending_updates=True)


if __name__ == "__main__":
    main()

