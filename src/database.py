"""SQLite database layer for persisting orgasm tracking entries."""

import asyncio
import logging
import os
import sqlite3
from datetime import datetime, timezone
from typing import Any

from src.config import BOT_TZ

logger = logging.getLogger(__name__)


class Database:
    """Thread-safe SQLite database manager for tracker entries."""

    def __init__(self, db_path: str = "/app/data/tracker.db") -> None:
        self.db_path = db_path
        self._ensure_directory()

    def _ensure_directory(self) -> None:
        """Ensure parent directory of database file exists."""
        parent_dir = os.path.dirname(os.path.abspath(self.db_path))
        if parent_dir:
            try:
                os.makedirs(parent_dir, exist_ok=True)
            except OSError as e:
                logger.warning("Verzeichnis '%s' konnte nicht vorab erstellt werden: %s", parent_dir, e)

    def get_connection(self) -> sqlite3.Connection:
        """Create and return a configured sqlite3 connection with Row factory."""
        conn = sqlite3.connect(self.db_path)
        conn.row_factory = sqlite3.Row
        return conn

    def init_db(self) -> None:
        """Create database tables and indices if they do not exist."""
        self._ensure_directory()
        with self.get_connection() as conn:
            # Enable WAL mode for better concurrency performance
            conn.execute("PRAGMA journal_mode = WAL;")
            conn.execute("PRAGMA foreign_keys = ON;")

            conn.execute(
                """
                CREATE TABLE IF NOT EXISTS entries (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    user_id INTEGER NOT NULL,
                    type TEXT NOT NULL,
                    timestamp TEXT NOT NULL,
                    company TEXT NULL,
                    partner_name TEXT NULL,
                    quality INTEGER NULL,
                    description TEXT NULL,
                    created_at TEXT NOT NULL
                );
                """
            )

            conn.execute(
                """
                CREATE INDEX IF NOT EXISTS idx_entries_user_timestamp
                ON entries (user_id, timestamp DESC, id DESC);
                """
            )
            conn.commit()
        logger.info("Datenbank erfolgreich initialisiert unter: %s", self.db_path)

    def add_entry(
        self,
        user_id: int,
        type: str,
        timestamp: str,
        company: str | None = None,
        partner_name: str | None = None,
        quality: int | None = None,
        description: str | None = None,
        created_at: str | None = None,
    ) -> int:
        """Insert a new orgasm log entry and return its generated ID."""
        if type not in ("simple", "detailed"):
            raise ValueError(f"Ungültiger Eintragstyp: '{type}'. Erlaubt sind 'simple' oder 'detailed'.")

        if quality is not None:
            if not isinstance(quality, int) or quality < 1 or quality > 10:
                raise ValueError("Qualität muss eine Ganzzahl zwischen 1 und 10 sein.")

        if created_at is None:
            created_at = datetime.now(BOT_TZ).isoformat()

        query = """
            INSERT INTO entries (
                user_id, type, timestamp, company, partner_name, quality, description, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?);
        """

        with self.get_connection() as conn:
            cursor = conn.cursor()
            cursor.execute(
                query,
                (
                    user_id,
                    type,
                    timestamp,
                    company,
                    partner_name,
                    quality,
                    description,
                    created_at,
                ),
            )
            conn.commit()
            entry_id = cursor.lastrowid
            logger.info("Neuer Eintrag gespeichert: ID=%s für User-ID=%s (%s)", entry_id, user_id, type)
            return entry_id

    def get_last_entries(self, user_id: int, limit: int = 10) -> list[dict[str, Any]]:
        """Retrieve the last N entries for a specific user ordered by timestamp DESC."""
        query = """
            SELECT id, user_id, type, timestamp, company, partner_name, quality, description, created_at
            FROM entries
            WHERE user_id = ?
            ORDER BY timestamp DESC, id DESC
            LIMIT ?;
        """
        with self.get_connection() as conn:
            cursor = conn.cursor()
            cursor.execute(query, (user_id, limit))
            rows = cursor.fetchall()
            return [dict(row) for row in rows]

    def get_entry_by_id(self, entry_id: int, user_id: int) -> dict[str, Any] | None:
        """Fetch a specific entry by ID and user_id."""
        query = """
            SELECT id, user_id, type, timestamp, company, partner_name, quality, description, created_at
            FROM entries
            WHERE id = ? AND user_id = ?;
        """
        with self.get_connection() as conn:
            cursor = conn.cursor()
            cursor.execute(query, (entry_id, user_id))
            row = cursor.fetchone()
            return dict(row) if row else None

    def delete_entry(self, entry_id: int, user_id: int) -> bool:
        """Delete an entry by ID belonging to user_id. Returns True if deleted."""
        query = "DELETE FROM entries WHERE id = ? AND user_id = ?;"
        with self.get_connection() as conn:
            cursor = conn.cursor()
            cursor.execute(query, (entry_id, user_id))
            conn.commit()
            return cursor.rowcount > 0

    # Async thread-safe wrappers for non-blocking execution in python-telegram-bot
    async def async_init_db(self) -> None:
        """Asynchronously initialize the database."""
        await asyncio.to_thread(self.init_db)

    async def async_add_entry(
        self,
        user_id: int,
        type: str,
        timestamp: str,
        company: str | None = None,
        partner_name: str | None = None,
        quality: int | None = None,
        description: str | None = None,
        created_at: str | None = None,
    ) -> int:
        """Asynchronously add an entry."""
        return await asyncio.to_thread(
            self.add_entry,
            user_id,
            type,
            timestamp,
            company,
            partner_name,
            quality,
            description,
            created_at,
        )

    async def async_get_last_entries(
        self, user_id: int, limit: int = 10
    ) -> list[dict[str, Any]]:
        """Asynchronously fetch the last entries."""
        return await asyncio.to_thread(self.get_last_entries, user_id, limit)
