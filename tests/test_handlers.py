"""Unit tests for bot handlers, security filters, and conversation states."""

import os
import tempfile
import pytest
from unittest.mock import AsyncMock, MagicMock, patch

from telegram import Update, User, Message, Chat, CallbackQuery
from telegram.error import BadRequest, TelegramError
from telegram.ext import ConversationHandler

from src.database import Database
import src.bot as bot
import src.config as config


@pytest.fixture
def mock_db():
    with tempfile.NamedTemporaryFile(suffix=".db", delete=False) as f:
        temp_path = f.name

    db = Database(db_path=temp_path)
    db.init_db()

    with patch.object(bot, "db", db):
        yield db

    if os.path.exists(temp_path):
        os.remove(temp_path)


def create_mock_update(user_id: int, text: str = None, callback_data: str = None, response_msg_id: int = 777):
    """Helper to create a realistic Telegram Update object."""
    update = MagicMock(spec=Update)
    user = User(id=user_id, is_bot=False, first_name="TestUser", username="testuser")
    chat = Chat(id=user_id, type="private")

    update.effective_user = user
    update.effective_chat = chat

    sent_bot_msg = MagicMock(spec=Message)
    sent_bot_msg.message_id = response_msg_id

    if callback_data is not None:
        query = MagicMock(spec=CallbackQuery)
        query.data = callback_data
        query.from_user = user
        query.answer = AsyncMock()
        query.edit_message_text = AsyncMock(return_value=sent_bot_msg)
        update.callback_query = query
        update.effective_message = None
    else:
        message = MagicMock(spec=Message)
        message.text = text
        message.from_user = user
        message.chat = chat
        message.reply_text = AsyncMock(return_value=sent_bot_msg)
        update.message = message
        update.effective_message = message
        update.callback_query = None

    return update


@pytest.mark.asyncio
async def test_restricted_decorator_blocks_unauthorized_user(mock_db, monkeypatch):
    """Test that unauthorized users are rejected and informed."""
    monkeypatch.setattr(config, "ALLOWED_USER_IDS", {12345})

    # Unauthorized user
    update = create_mock_update(user_id=99999, text="/orgasmus")
    context = MagicMock()

    result = await bot.start_orgasmus(update, context)

    assert result == ConversationHandler.END
    update.effective_message.reply_text.assert_called_once()
    assert "Zugriff verweigert" in update.effective_message.reply_text.call_args[0][0]
    assert "99999" in update.effective_message.reply_text.call_args[0][0]


@pytest.mark.asyncio
async def test_restricted_decorator_blocks_unauthorized_callback(mock_db, monkeypatch):
    """Test that unauthorized callback queries receive an alert."""
    monkeypatch.setattr(config, "ALLOWED_USER_IDS", {12345})

    update = create_mock_update(user_id=99999, callback_data="ACTION_CANCEL")
    context = MagicMock()

    result = await bot.handle_action_cancel(update, context)

    assert result == ConversationHandler.END
    update.callback_query.answer.assert_called_once()
    assert "Zugriff verweigert" in update.callback_query.answer.call_args[0][0]


@pytest.mark.asyncio
async def test_start_orgasmus_authorized(mock_db, monkeypatch):
    """Test /orgasmus prompts for simple vs detailed choice."""
    monkeypatch.setattr(config, "ALLOWED_USER_IDS", {12345})

    update = create_mock_update(user_id=12345, text="/orgasmus")
    context = MagicMock()
    context.user_data = {}

    state = await bot.start_orgasmus(update, context)

    assert state == bot.STATE_CHOOSE_TYPE
    update.effective_message.reply_text.assert_called_once()
    prompt = update.effective_message.reply_text.call_args[0][0]
    assert "einfachen oder ausführlichen Orgasmus" in prompt


@pytest.mark.asyncio
async def test_handle_type_simple(mock_db, monkeypatch):
    """Test selecting simple orgasm immediately persists and ends conversation."""
    monkeypatch.setattr(config, "ALLOWED_USER_IDS", {12345})

    update = create_mock_update(user_id=12345, callback_data=bot.TYPE_SIMPLE)
    context = MagicMock()
    context.user_data = {}

    state = await bot.handle_type_simple(update, context)

    assert state == ConversationHandler.END
    update.callback_query.edit_message_text.assert_called_once()
    msg = update.callback_query.edit_message_text.call_args[0][0]
    assert "Einfacher Orgasmus wurde mit aktuellem Datum" in msg
    assert "gespeichert!" in msg

    # Verify entry in DB
    entries = mock_db.get_last_entries(user_id=12345)
    assert len(entries) == 1
    assert entries[0]["type"] == "simple"


@pytest.mark.asyncio
async def test_handle_type_detailed_and_finish_immediately(mock_db, monkeypatch):
    """Test selecting detailed orgasm and then immediately finishing."""
    monkeypatch.setattr(config, "ALLOWED_USER_IDS", {12345})

    # Step 1: Click Ausführlicher Orgasmus
    update_type = create_mock_update(user_id=12345, callback_data=bot.TYPE_DETAILED)
    context = MagicMock()
    context.user_data = {}

    state = await bot.handle_type_detailed(update_type, context)
    assert state == bot.STATE_DATETIME
    assert "draft" in context.user_data
    assert context.user_data["draft"]["type"] == "detailed"
    assert context.user_data["draft"]["timestamp"] is not None

    # Step 2: Click Fertigstellen immediately
    update_finish = create_mock_update(user_id=12345, callback_data=bot.ACTION_FINISH)
    state = await bot.handle_action_finish(update_finish, context)

    assert state == ConversationHandler.END
    update_finish.callback_query.edit_message_text.assert_called_once()
    msg = update_finish.callback_query.edit_message_text.call_args[0][0]
    assert "Ausführlicher Orgasmus erfolgreich gespeichert!" in msg

    # Verify persisted in database
    entries = mock_db.get_last_entries(user_id=12345)
    assert len(entries) == 1
    assert entries[0]["type"] == "detailed"
    assert entries[0]["company"] is None
    assert entries[0]["quality"] is None


@pytest.mark.asyncio
async def test_detailed_flow_complete(mock_db, monkeypatch):
    """Test full step-by-step detailed flow with all questions answered."""
    monkeypatch.setattr(config, "ALLOWED_USER_IDS", {12345})
    context = MagicMock()
    context.user_data = {}

    # 1. Start detailed
    update = create_mock_update(user_id=12345, callback_data=bot.TYPE_DETAILED)
    state = await bot.handle_type_detailed(update, context)
    assert state == bot.STATE_DATETIME

    # 2. DateTime manual input
    update_dt = create_mock_update(user_id=12345, text="19.10.2026 18:30")
    state = await bot.handle_datetime_text(update_dt, context)
    assert state == bot.STATE_COMPANY
    assert context.user_data["draft"]["timestamp"] == "2026-10-19 18:30"

    # 3. Company choice: Mit anderer Person
    update_comp = create_mock_update(user_id=12345, callback_data=bot.COMPANY_PARTNER)
    state = await bot.handle_company_partner(update_comp, context)
    assert state == bot.STATE_PARTNER_NAME
    assert context.user_data["draft"]["company"] == "Mit anderer Person"

    # 4. Partner name text
    update_partner = create_mock_update(user_id=12345, text="Laura")
    state = await bot.handle_partner_name_text(update_partner, context)
    assert state == bot.STATE_QUALITY
    assert context.user_data["draft"]["partner_name"] == "Laura"

    # 5. Quality rating: 10
    update_qual = create_mock_update(user_id=12345, callback_data="QUALITY_10")
    state = await bot.handle_quality_choice(update_qual, context)
    assert state == bot.STATE_DESCRIPTION
    assert context.user_data["draft"]["quality"] == 10

    # 6. Description text
    update_desc = create_mock_update(user_id=12345, text="Traumhaft schön")
    state = await bot.handle_description_text(update_desc, context)
    assert state == ConversationHandler.END

    # Verify entry in DB
    entries = mock_db.get_last_entries(user_id=12345)
    assert len(entries) == 1
    e = entries[0]
    assert e["type"] == "detailed"
    assert e["timestamp"] == "2026-10-19 18:30"
    assert e["company"] == "Mit anderer Person"
    assert e["partner_name"] == "Laura"
    assert e["quality"] == 10
    assert e["description"] == "Traumhaft schön"


@pytest.mark.asyncio
async def test_detailed_flow_alone_skips_partner(mock_db, monkeypatch):
    """Test that selecting 'Alleine' skips partner name directly to quality."""
    monkeypatch.setattr(config, "ALLOWED_USER_IDS", {12345})
    context = MagicMock()
    context.user_data = {
        "draft": {
            "type": "detailed",
            "timestamp": "2026-10-19 18:30",
        }
    }

    update = create_mock_update(user_id=12345, callback_data=bot.COMPANY_ALONE)
    state = await bot.handle_company_alone(update, context)

    assert state == bot.STATE_QUALITY
    assert context.user_data["draft"]["company"] == "Alleine"
    assert context.user_data["draft"]["partner_name"] is None


@pytest.mark.asyncio
async def test_cancel_action_discards_draft(mock_db, monkeypatch):
    """Test clicking Abbruch clears draft and cancels."""
    monkeypatch.setattr(config, "ALLOWED_USER_IDS", {12345})
    context = MagicMock()
    context.user_data = {"draft": {"type": "detailed"}}

    update = create_mock_update(user_id=12345, callback_data=bot.ACTION_CANCEL)
    state = await bot.handle_action_cancel(update, context)

    assert state == ConversationHandler.END
    assert len(context.user_data) == 0
    update.callback_query.edit_message_text.assert_called_once_with("❌ Eintrag abgebrochen.")

    # DB should be empty
    entries = mock_db.get_last_entries(user_id=12345)
    assert len(entries) == 0


@pytest.mark.asyncio
async def test_liste_command(mock_db, monkeypatch):
    """Test /liste command when empty and with records."""
    monkeypatch.setattr(config, "ALLOWED_USER_IDS", {12345})
    context = MagicMock()

    # Case 1: Empty
    update_empty = create_mock_update(user_id=12345, text="/liste")
    await bot.handle_liste(update_empty, context)
    update_empty.effective_message.reply_text.assert_called_once()
    assert "Bisher sind noch keine Einträge vorhanden" in update_empty.effective_message.reply_text.call_args[0][0]

    # Insert test records
    mock_db.add_entry(user_id=12345, type="simple", timestamp="2026-10-18 10:00")
    mock_db.add_entry(
        user_id=12345,
        type="detailed",
        timestamp="2026-10-19 14:00",
        company="Mit anderer Person",
        partner_name="Mia",
        quality=9,
        description="Sehr gut",
    )

    # Case 2: Populated
    update_pop = create_mock_update(user_id=12345, text="/liste")
    await bot.handle_liste(update_pop, context)
    update_pop.effective_message.reply_text.assert_called_once()
    reply = update_pop.effective_message.reply_text.call_args[0][0]
    assert "Deine letzten 2 Einträge" in reply
    assert "Ausführlich" in reply
    assert "Einfach" in reply
    assert "Mia" in reply
    assert "9/10" in reply


def test_remember_keyboard_message_helper():
    """Verify storing message ID in user_data."""
    context = MagicMock()
    context.user_data = {}

    msg = MagicMock(spec=Message)
    msg.message_id = 123
    bot.remember_keyboard_message(context, msg)
    assert context.user_data["last_keyboard_msg_id"] == 123

    # None or bool should not break or overwrite
    bot.remember_keyboard_message(context, None)
    assert context.user_data["last_keyboard_msg_id"] == 123
    bot.remember_keyboard_message(context, True)
    assert context.user_data["last_keyboard_msg_id"] == 123


@pytest.mark.asyncio
async def test_clear_previous_keyboard_helper():
    """Verify clear_previous_keyboard calls edit_message_reply_markup with None."""
    context = MagicMock()
    context.user_data = {"last_keyboard_msg_id": 456}
    context.bot.edit_message_reply_markup = AsyncMock()

    update = create_mock_update(user_id=12345, text="test")
    await bot.clear_previous_keyboard(update, context)

    context.bot.edit_message_reply_markup.assert_called_once_with(
        chat_id=12345,
        message_id=456,
        reply_markup=None,
    )
    assert "last_keyboard_msg_id" not in context.user_data

    # Second call does nothing
    context.bot.edit_message_reply_markup.reset_mock()
    await bot.clear_previous_keyboard(update, context)
    context.bot.edit_message_reply_markup.assert_not_called()


@pytest.mark.asyncio
async def test_clear_previous_keyboard_suppresses_telegram_error():
    """Verify clear_previous_keyboard handles TelegramError gracefully."""
    context = MagicMock()
    context.user_data = {"last_keyboard_msg_id": 456}
    context.bot.edit_message_reply_markup = AsyncMock(
        side_effect=BadRequest("Message is not modified")
    )

    update = create_mock_update(user_id=12345, text="test")
    await bot.clear_previous_keyboard(update, context)
    assert "last_keyboard_msg_id" not in context.user_data


@pytest.mark.asyncio
async def test_clear_keyboard_on_datetime_text(mock_db, monkeypatch):
    """Verify that datetime text input clears the previous keyboard."""
    monkeypatch.setattr(config, "ALLOWED_USER_IDS", {12345})
    context = MagicMock()
    context.user_data = {"last_keyboard_msg_id": 789}
    context.bot.edit_message_reply_markup = AsyncMock()

    update = create_mock_update(user_id=12345, text="19.10.2026 18:30", response_msg_id=888)
    state = await bot.handle_datetime_text(update, context)

    assert state == bot.STATE_COMPANY
    context.bot.edit_message_reply_markup.assert_called_once_with(
        chat_id=12345,
        message_id=789,
        reply_markup=None,
    )
    # The new message sent by the bot should now be tracked
    assert context.user_data["last_keyboard_msg_id"] == 888


@pytest.mark.asyncio
async def test_clear_keyboard_on_partner_name_text(mock_db, monkeypatch):
    """Verify that partner name text input clears the previous keyboard."""
    monkeypatch.setattr(config, "ALLOWED_USER_IDS", {12345})
    context = MagicMock()
    context.user_data = {"last_keyboard_msg_id": 333, "draft": {}}
    context.bot.edit_message_reply_markup = AsyncMock()

    update = create_mock_update(user_id=12345, text="Sam", response_msg_id=444)
    state = await bot.handle_partner_name_text(update, context)

    assert state == bot.STATE_QUALITY
    context.bot.edit_message_reply_markup.assert_called_once_with(
        chat_id=12345,
        message_id=333,
        reply_markup=None,
    )
    assert context.user_data["last_keyboard_msg_id"] == 444


@pytest.mark.asyncio
async def test_clear_keyboard_on_description_text(mock_db, monkeypatch):
    """Verify that description text input clears the previous keyboard."""
    monkeypatch.setattr(config, "ALLOWED_USER_IDS", {12345})
    context = MagicMock()
    context.user_data = {
        "last_keyboard_msg_id": 555,
        "draft": {
            "type": "detailed",
            "timestamp": "2026-10-19 18:30",
        },
    }
    context.bot.edit_message_reply_markup = AsyncMock()

    update = create_mock_update(user_id=12345, text="Super Erfahrung")
    state = await bot.handle_description_text(update, context)

    assert state == ConversationHandler.END
    context.bot.edit_message_reply_markup.assert_called_once_with(
        chat_id=12345,
        message_id=555,
        reply_markup=None,
    )


@pytest.mark.asyncio
async def test_clear_keyboard_on_cancel_command(mock_db, monkeypatch):
    """Verify that /cancel command clears the previous keyboard."""
    monkeypatch.setattr(config, "ALLOWED_USER_IDS", {12345})
    context = MagicMock()
    context.user_data = {"last_keyboard_msg_id": 666}
    context.bot.edit_message_reply_markup = AsyncMock()

    update = create_mock_update(user_id=12345, text="/cancel")
    state = await bot.handle_cancel_command(update, context)

    assert state == ConversationHandler.END
    context.bot.edit_message_reply_markup.assert_called_once_with(
        chat_id=12345,
        message_id=666,
        reply_markup=None,
    )


@pytest.mark.asyncio
async def test_clear_keyboard_on_reentry_start_orgasmus(mock_db, monkeypatch):
    """Verify that re-entering via /orgasmus command clears the previous keyboard."""
    monkeypatch.setattr(config, "ALLOWED_USER_IDS", {12345})
    context = MagicMock()
    context.user_data = {"last_keyboard_msg_id": 111}
    context.bot.edit_message_reply_markup = AsyncMock()

    update = create_mock_update(user_id=12345, text="/orgasmus", response_msg_id=222)
    state = await bot.start_orgasmus(update, context)

    assert state == bot.STATE_CHOOSE_TYPE
    context.bot.edit_message_reply_markup.assert_called_once_with(
        chat_id=12345,
        message_id=111,
        reply_markup=None,
    )
    assert context.user_data["last_keyboard_msg_id"] == 222


@pytest.mark.asyncio
async def test_liste_shows_delete_button(mock_db, monkeypatch):
    """Verify /liste attaches the main delete keyboard if entries exist."""
    monkeypatch.setattr(config, "ALLOWED_USER_IDS", {12345})
    mock_db.add_entry(user_id=12345, type="simple", timestamp="2026-10-19 12:00")

    context = MagicMock()
    update = create_mock_update(user_id=12345, text="/liste")

    await bot.handle_liste(update, context)

    update.effective_message.reply_text.assert_called_once()
    kwargs = update.effective_message.reply_text.call_args[1]
    assert kwargs.get("reply_markup") is not None
    button = kwargs["reply_markup"].inline_keyboard[0][0]
    assert button.callback_data == bot.LIST_DELETE_MODE
    assert "🗑️ Einträge löschen" in button.text


@pytest.mark.asyncio
async def test_list_delete_mode_switch(mock_db, monkeypatch):
    """Verify switching into delete selection mode."""
    monkeypatch.setattr(config, "ALLOWED_USER_IDS", {12345})
    id1 = mock_db.add_entry(user_id=12345, type="simple", timestamp="2026-10-18 10:00")
    id2 = mock_db.add_entry(user_id=12345, type="simple", timestamp="2026-10-19 10:00")

    context = MagicMock()
    context.user_data = {}
    update = create_mock_update(user_id=12345, callback_data=bot.LIST_DELETE_MODE)

    await bot.handle_list_delete_mode(update, context)

    update.callback_query.edit_message_text.assert_called_once()
    kwargs = update.callback_query.edit_message_text.call_args[1]
    prompt = update.callback_query.edit_message_text.call_args[0][0]
    assert "Wähle die Nummern der Einträge aus" in prompt

    kb = kwargs["reply_markup"]
    # Buttons should show ⬜ 1 and ⬜ 2
    button_callbacks = [b.callback_data for row in kb.inline_keyboard for b in row]
    assert f"{bot.LIST_TOGGLE_PREFIX}{id2}" in button_callbacks
    assert f"{bot.LIST_TOGGLE_PREFIX}{id1}" in button_callbacks
    assert bot.LIST_DELETE_CANCEL in button_callbacks


@pytest.mark.asyncio
async def test_list_toggle_entry(mock_db, monkeypatch):
    """Verify toggling entries updates selected_ids and keyboard."""
    monkeypatch.setattr(config, "ALLOWED_USER_IDS", {12345})
    id1 = mock_db.add_entry(user_id=12345, type="simple", timestamp="2026-10-19 10:00")

    context = MagicMock()
    context.user_data = {}

    # 1. Toggle ON
    update1 = create_mock_update(user_id=12345, callback_data=f"{bot.LIST_TOGGLE_PREFIX}{id1}")
    await bot.handle_list_toggle_entry(update1, context)

    assert id1 in context.user_data["delete_selected_ids"]
    update1.callback_query.edit_message_reply_markup.assert_called_once()
    kb1 = update1.callback_query.edit_message_reply_markup.call_args[1]["reply_markup"]
    texts1 = [b.text for row in kb1.inline_keyboard for b in row]
    assert "☑️ 1" in texts1
    assert any("🗑️ Auswahl löschen (1)" in t for t in texts1)

    # 2. Toggle OFF
    update2 = create_mock_update(user_id=12345, callback_data=f"{bot.LIST_TOGGLE_PREFIX}{id1}")
    await bot.handle_list_toggle_entry(update2, context)

    assert id1 not in context.user_data["delete_selected_ids"]
    update2.callback_query.edit_message_reply_markup.assert_called_once()
    kb2 = update2.callback_query.edit_message_reply_markup.call_args[1]["reply_markup"]
    texts2 = [b.text for row in kb2.inline_keyboard for b in row]
    assert "⬜ 1" in texts2
    assert not any("Auswahl löschen" in t for t in texts2)


@pytest.mark.asyncio
async def test_list_delete_confirm_dialog(mock_db, monkeypatch):
    """Verify confirmation prompt lists selected entries to be deleted."""
    monkeypatch.setattr(config, "ALLOWED_USER_IDS", {12345})
    id1 = mock_db.add_entry(
        user_id=12345,
        type="detailed",
        timestamp="2026-10-19 18:30",
        partner_name="Alex",
    )

    context = MagicMock()
    context.user_data = {"delete_selected_ids": {id1}}
    update = create_mock_update(user_id=12345, callback_data=bot.LIST_DELETE_CONFIRM)

    await bot.handle_list_delete_confirm(update, context)

    update.callback_query.edit_message_text.assert_called_once()
    msg = update.callback_query.edit_message_text.call_args[0][0]
    assert "Sicherheitsabfrage" in msg
    assert "19.10.2026 um 18:30 Uhr" in msg
    assert "Alex" in msg
    kb = update.callback_query.edit_message_text.call_args[1]["reply_markup"]
    callbacks = [b.callback_data for row in kb.inline_keyboard for b in row]
    assert bot.LIST_DELETE_EXECUTE in callbacks
    assert bot.LIST_DELETE_MODE in callbacks
    assert bot.LIST_DELETE_CANCEL in callbacks


@pytest.mark.asyncio
async def test_list_delete_confirm_empty_selection(mock_db, monkeypatch):
    """Verify alert when trying to confirm without selection."""
    monkeypatch.setattr(config, "ALLOWED_USER_IDS", {12345})
    context = MagicMock()
    context.user_data = {"delete_selected_ids": set()}
    update = create_mock_update(user_id=12345, callback_data=bot.LIST_DELETE_CONFIRM)

    await bot.handle_list_delete_confirm(update, context)

    update.callback_query.answer.assert_called_once_with(
        "Bitte wähle mindestens einen Eintrag aus.", show_alert=True
    )
    update.callback_query.edit_message_text.assert_not_called()


@pytest.mark.asyncio
async def test_list_delete_execute_success(mock_db, monkeypatch):
    """Verify deletion removes selected entries from DB and updates view."""
    monkeypatch.setattr(config, "ALLOWED_USER_IDS", {12345})
    id1 = mock_db.add_entry(user_id=12345, type="simple", timestamp="2026-10-18 10:00")
    id2 = mock_db.add_entry(user_id=12345, type="simple", timestamp="2026-10-19 10:00")

    context = MagicMock()
    context.user_data = {"delete_selected_ids": {id1}}
    update = create_mock_update(user_id=12345, callback_data=bot.LIST_DELETE_EXECUTE)

    await bot.handle_list_delete_execute(update, context)

    # id1 must be deleted, id2 must remain
    assert mock_db.get_entry_by_id(id1, user_id=12345) is None
    assert mock_db.get_entry_by_id(id2, user_id=12345) is not None

    update.callback_query.answer.assert_called_once_with("✅ 1 Eintrag/Einträge gelöscht.")
    update.callback_query.edit_message_text.assert_called_once()
    msg = update.callback_query.edit_message_text.call_args[0][0]
    assert "1 Eintrag/Einträge erfolgreich gelöscht." in msg


@pytest.mark.asyncio
async def test_list_delete_cancel_restores_view(mock_db, monkeypatch):
    """Verify cancel clears selected IDs and restores list view."""
    monkeypatch.setattr(config, "ALLOWED_USER_IDS", {12345})
    mock_db.add_entry(user_id=12345, type="simple", timestamp="2026-10-19 10:00")

    context = MagicMock()
    context.user_data = {"delete_selected_ids": {999}}
    update = create_mock_update(user_id=12345, callback_data=bot.LIST_DELETE_CANCEL)

    await bot.handle_list_delete_cancel(update, context)

    assert "delete_selected_ids" not in context.user_data
    update.callback_query.edit_message_text.assert_called_once()
    kb = update.callback_query.edit_message_text.call_args[1]["reply_markup"]
    assert kb.inline_keyboard[0][0].callback_data == bot.LIST_DELETE_MODE


@pytest.mark.asyncio
async def test_user_cannot_delete_other_users_entry(mock_db, monkeypatch):
    """Security test: Ensure User A cannot delete User B's entries."""
    monkeypatch.setattr(config, "ALLOWED_USER_IDS", {12345, 99999})
    user_b_entry = mock_db.add_entry(user_id=99999, type="simple", timestamp="2026-10-19 12:00")

    # User A tries to delete user B's entry directly via DB method
    deleted = mock_db.delete_entry(entry_id=user_b_entry, user_id=12345)
    assert deleted is False
    assert mock_db.get_entry_by_id(user_b_entry, user_id=99999) is not None

    # User A tries to delete user B's entry via bot handler
    context = MagicMock()
    context.user_data = {"delete_selected_ids": {user_b_entry}}
    update_user_a = create_mock_update(user_id=12345, callback_data=bot.LIST_DELETE_EXECUTE)

    await bot.handle_list_delete_execute(update_user_a, context)

    # Entry must still be in DB
    assert mock_db.get_entry_by_id(user_b_entry, user_id=99999) is not None
