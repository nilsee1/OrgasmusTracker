"""Configuration management for the Telegram bot."""

import logging
import os
import zoneinfo
from dotenv import load_dotenv

# Load environment variables from .env if present
load_dotenv()

logger = logging.getLogger(__name__)

# Telegram Bot Token
TELEGRAM_BOT_TOKEN: str = os.getenv("TELEGRAM_BOT_TOKEN", "").strip()

# Database Path
DB_PATH: str = os.getenv("DB_PATH", "/app/data/tracker.db").strip()

# Timezone Configuration
TZ_NAME: str = os.getenv("TZ", "Europe/Berlin").strip()
try:
    BOT_TZ = zoneinfo.ZoneInfo(TZ_NAME)
except Exception as e:
    logger.warning(
        "Konnte Zeitzone '%s' nicht initialisieren (%s). Fallback auf 'Europe/Berlin'.",
        TZ_NAME,
        e,
    )
    BOT_TZ = zoneinfo.ZoneInfo("Europe/Berlin")


def parse_allowed_user_ids(raw_ids: str) -> set[int]:
    """Parse comma-separated Telegram user IDs into a set of integers."""
    allowed = set()
    if not raw_ids:
        return allowed

    for item in raw_ids.split(","):
        cleaned = item.strip()
        if not cleaned:
            continue
        try:
            allowed.add(int(cleaned))
        except ValueError:
            logger.warning("Ungültige User-ID in ALLOWED_USER_IDS ignoriert: '%s'", cleaned)
    return allowed


ALLOWED_USER_IDS: set[int] = parse_allowed_user_ids(os.getenv("ALLOWED_USER_IDS", ""))


def is_user_allowed(user_id: int | None) -> bool:
    """Check if the given Telegram user ID is authorized."""
    if user_id is None:
        return False
    return user_id in ALLOWED_USER_IDS


def validate_config() -> None:
    """Validate mandatory configuration variables on startup."""
    if not TELEGRAM_BOT_TOKEN:
        raise ValueError(
            "Umgebungsvariable TELEGRAM_BOT_TOKEN fehlt! Bitte in .env oder Umgebung setzen."
        )

    if not ALLOWED_USER_IDS:
        logger.warning(
            "⚠️ WARNUNG: ALLOWED_USER_IDS ist leer! Der Bot wird alle Interaktionen ablehnen, "
            "bis mindestens eine Telegram User-ID konfiguriert ist."
        )
    else:
        logger.info(
            "Konfiguration geladen: %d autorisierte(r) Nutzer, Zeitzone=%s, DB-Pfad=%s",
            len(ALLOWED_USER_IDS),
            BOT_TZ.key,
            DB_PATH,
        )

