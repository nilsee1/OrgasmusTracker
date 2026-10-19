"""Unit tests for SQLite database layer."""

import os
import tempfile
import pytest
from src.database import Database


@pytest.fixture
def temp_db():
    """Create a temporary database for isolated testing."""
    with tempfile.NamedTemporaryFile(suffix=".db", delete=False) as f:
        temp_path = f.name

    db = Database(db_path=temp_path)
    db.init_db()
    yield db

    # Cleanup temporary file
    if os.path.exists(temp_path):
        os.remove(temp_path)


def test_init_db(temp_db):
    """Verify that tables and indices are created correctly."""
    with temp_db.get_connection() as conn:
        cursor = conn.cursor()
        cursor.execute("SELECT name FROM sqlite_master WHERE type='table' AND name='entries';")
        assert cursor.fetchone() is not None

        cursor.execute("PRAGMA table_info(entries);")
        columns = {row["name"]: row["type"] for row in cursor.fetchall()}
        assert "id" in columns
        assert "user_id" in columns
        assert "type" in columns
        assert "timestamp" in columns
        assert "company" in columns
        assert "partner_name" in columns
        assert "quality" in columns
        assert "description" in columns
        assert "created_at" in columns


def test_add_simple_entry(temp_db):
    """Test adding a simple entry."""
    entry_id = temp_db.add_entry(
        user_id=123,
        type="simple",
        timestamp="2026-10-19 18:30",
    )
    assert entry_id > 0

    entry = temp_db.get_entry_by_id(entry_id, user_id=123)
    assert entry is not None
    assert entry["user_id"] == 123
    assert entry["type"] == "simple"
    assert entry["timestamp"] == "2026-10-19 18:30"
    assert entry["company"] is None
    assert entry["partner_name"] is None
    assert entry["quality"] is None
    assert entry["description"] is None
    assert entry["created_at"] is not None


def test_add_detailed_entry(temp_db):
    """Test adding a detailed entry with all fields."""
    entry_id = temp_db.add_entry(
        user_id=456,
        type="detailed",
        timestamp="2026-10-19 20:15",
        company="Mit anderer Person",
        partner_name="Alex",
        quality=9,
        description="Sehr intensiv und schön",
    )
    assert entry_id > 0

    entry = temp_db.get_entry_by_id(entry_id, user_id=456)
    assert entry is not None
    assert entry["user_id"] == 456
    assert entry["type"] == "detailed"
    assert entry["timestamp"] == "2026-10-19 20:15"
    assert entry["company"] == "Mit anderer Person"
    assert entry["partner_name"] == "Alex"
    assert entry["quality"] == 9
    assert entry["description"] == "Sehr intensiv und schön"


def test_invalid_type_raises_error(temp_db):
    """Test that invalid types raise ValueError."""
    with pytest.raises(ValueError):
        temp_db.add_entry(user_id=123, type="invalid_type", timestamp="2026-10-19 12:00")


def test_invalid_quality_raises_error(temp_db):
    """Test quality bounds (1 to 10)."""
    with pytest.raises(ValueError):
        temp_db.add_entry(user_id=123, type="detailed", timestamp="2026-10-19 12:00", quality=0)

    with pytest.raises(ValueError):
        temp_db.add_entry(user_id=123, type="detailed", timestamp="2026-10-19 12:00", quality=11)


def test_get_last_entries_ordering_and_limit(temp_db):
    """Verify ordering by timestamp DESC and limit to 10."""
    user_id = 999
    # Add 12 entries with different dates
    for i in range(1, 13):
        day = f"{i:02d}"
        temp_db.add_entry(
            user_id=user_id,
            type="simple",
            timestamp=f"2026-10-{day} 10:00",
        )

    # Should only return the last 10 entries (days 12 down to 03)
    entries = temp_db.get_last_entries(user_id=user_id, limit=10)
    assert len(entries) == 10
    assert entries[0]["timestamp"] == "2026-10-12 10:00"
    assert entries[-1]["timestamp"] == "2026-10-03 10:00"


def test_isolation_between_users(temp_db):
    """Ensure user entries are strictly isolated."""
    temp_db.add_entry(user_id=100, type="simple", timestamp="2026-10-19 12:00")
    temp_db.add_entry(user_id=200, type="simple", timestamp="2026-10-19 12:00")

    entries_user_100 = temp_db.get_last_entries(user_id=100)
    assert len(entries_user_100) == 1
    assert entries_user_100[0]["user_id"] == 100

    entries_user_200 = temp_db.get_last_entries(user_id=200)
    assert len(entries_user_200) == 1
    assert entries_user_200[0]["user_id"] == 200

    entries_user_300 = temp_db.get_last_entries(user_id=300)
    assert len(entries_user_300) == 0


@pytest.mark.asyncio
async def test_async_database_methods(temp_db):
    """Test async wrappers for DB operations."""
    entry_id = await temp_db.async_add_entry(
        user_id=777,
        type="simple",
        timestamp="2026-10-19 15:00",
    )
    assert entry_id > 0

    entries = await temp_db.async_get_last_entries(user_id=777, limit=5)
    assert len(entries) == 1
    assert entries[0]["user_id"] == 777

