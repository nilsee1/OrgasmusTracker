"""Unit tests for bot formatting, date parsing, and keyboard builders."""

import pytest
from datetime import datetime
from src.bot import (
    parse_german_datetime,
    format_timestamp_display,
    format_detailed_summary,
    format_entries_list_text,
    MAX_DESCRIPTION_LENGTH,
    get_choose_type_keyboard,
    get_datetime_keyboard,
    get_company_keyboard,
    get_quality_keyboard,
    get_description_keyboard,
    ACTION_FINISH,
    ACTION_CANCEL,
    TYPE_SIMPLE,
    TYPE_DETAILED,
    TIME_NOW,
    COMPANY_ALONE,
    COMPANY_PARTNER,
)


def test_parse_german_datetime_valid():
    dt = parse_german_datetime("19.10.2026 18:30")
    assert dt is not None
    assert dt.day == 19
    assert dt.month == 10
    assert dt.year == 2026
    assert dt.hour == 18
    assert dt.minute == 30


def test_parse_german_datetime_invalid():
    assert parse_german_datetime("invalid-date") is None
    assert parse_german_datetime("2026-10-19 18:30") is None
    assert parse_german_datetime("32.10.2026 18:30") is None
    assert parse_german_datetime("19.13.2026 18:30") is None


def test_format_timestamp_display():
    # Standard format
    assert format_timestamp_display("2026-10-19 18:30") == "19.10.2026 um 18:30 Uhr"
    # ISO format
    assert "19.10.2026 um 18:30 Uhr" in format_timestamp_display("2026-10-19T18:30:00")
    # Empty string fallback
    assert format_timestamp_display("") == ""


def test_format_detailed_summary():
    # Full summary
    summary = format_detailed_summary(
        timestamp="2026-10-19 18:30",
        company="Mit anderer Person",
        partner_name="Julia",
        quality=10,
        description="Wunderbar!",
    )
    assert "Ausführlicher Orgasmus erfolgreich gespeichert!" in summary
    assert "19.10.2026 um 18:30 Uhr" in summary
    assert "Mit anderer Person (Julia)" in summary
    assert "10/10" in summary
    assert "Wunderbar!" in summary


def test_format_detailed_summary_with_nulls():
    summary = format_detailed_summary(
        timestamp="2026-10-19 18:30",
        company=None,
        partner_name=None,
        quality=None,
        description=None,
    )
    assert "Ausführlicher Orgasmus erfolgreich gespeichert!" in summary
    assert "Keine Angabe" in summary
    assert "Keine" in summary


def test_format_detailed_summary_alone():
    summary = format_detailed_summary(
        timestamp="2026-10-19 18:30",
        company="Alleine",
        partner_name=None,
        quality=8,
        description=None,
    )
    assert "Begleitung:</b> Alleine" in summary
    assert "8/10" in summary


def test_keyboards_structure():
    # Type keyboard
    type_kb = get_choose_type_keyboard()
    callback_data_list = [btn.callback_data for row in type_kb.inline_keyboard for btn in row]
    assert TYPE_SIMPLE in callback_data_list
    assert TYPE_DETAILED in callback_data_list
    assert ACTION_CANCEL in callback_data_list

    # Datetime keyboard
    dt_kb = get_datetime_keyboard()
    dt_callbacks = [btn.callback_data for row in dt_kb.inline_keyboard for btn in row]
    assert TIME_NOW in dt_callbacks
    assert ACTION_FINISH in dt_callbacks
    assert ACTION_CANCEL in dt_callbacks

    # Company keyboard
    company_kb = get_company_keyboard()
    company_callbacks = [btn.callback_data for row in company_kb.inline_keyboard for btn in row]
    assert COMPANY_ALONE in company_callbacks
    assert COMPANY_PARTNER in company_callbacks
    assert ACTION_FINISH in company_callbacks
    assert ACTION_CANCEL in company_callbacks

    # Quality keyboard (1-10 in 2 rows + actions)
    quality_kb = get_quality_keyboard()
    assert len(quality_kb.inline_keyboard) == 3
    assert len(quality_kb.inline_keyboard[0]) == 5
    assert len(quality_kb.inline_keyboard[1]) == 5
    quality_callbacks = [btn.callback_data for row in quality_kb.inline_keyboard for btn in row]
    for i in range(1, 11):
        assert f"QUALITY_{i}" in quality_callbacks
    assert ACTION_FINISH in quality_callbacks
    assert ACTION_CANCEL in quality_callbacks

    # Description keyboard
    desc_kb = get_description_keyboard()
    desc_callbacks = [btn.callback_data for row in desc_kb.inline_keyboard for btn in row]
    assert ACTION_FINISH in desc_callbacks
    assert ACTION_CANCEL in desc_callbacks


def test_format_entries_list_text_truncation():
    """Verify that descriptions longer than MAX_DESCRIPTION_LENGTH are cleanly truncated."""
    long_desc = "A" * (MAX_DESCRIPTION_LENGTH + 50)
    entries = [
        {
            "id": 1,
            "type": "detailed",
            "timestamp": "2026-10-19 18:30",
            "company": "Alleine",
            "partner_name": None,
            "quality": 8,
            "description": long_desc,
        }
    ]
    text = format_entries_list_text(entries)
    expected_truncated = ("A" * MAX_DESCRIPTION_LENGTH) + "…"
    assert expected_truncated in text
    assert long_desc not in text


