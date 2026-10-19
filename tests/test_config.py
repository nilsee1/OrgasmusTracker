"""Unit tests for configuration loading and security filters."""

import os
import pytest
from src.config import parse_allowed_user_ids, is_user_allowed


def test_parse_allowed_user_ids_empty():
    assert parse_allowed_user_ids("") == set()
    assert parse_allowed_user_ids("   ") == set()


def test_parse_allowed_user_ids_valid():
    raw = "12345, 67890,  112233 "
    result = parse_allowed_user_ids(raw)
    assert result == {12345, 67890, 112233}


def test_parse_allowed_user_ids_with_invalid():
    raw = "12345, abc, 67890, , -999"
    result = parse_allowed_user_ids(raw)
    assert 12345 in result
    assert 67890 in result
    assert -999 in result


def test_is_user_allowed(monkeypatch):
    import src.config
    monkeypatch.setattr(src.config, "ALLOWED_USER_IDS", {111, 222})

    assert src.config.is_user_allowed(111) is True
    assert src.config.is_user_allowed(222) is True
    assert src.config.is_user_allowed(333) is False
    assert src.config.is_user_allowed(None) is False

