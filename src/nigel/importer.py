import hashlib
import sqlite3
from datetime import datetime
from pathlib import Path

from nigel.models import ParsedRow
from nigel.registry import registry


def _compute_checksum(file_path: Path) -> str:
    return hashlib.sha256(file_path.read_bytes()).hexdigest()


def parse_amount(raw: str) -> float:
    """Strip commas and quotes, return float."""
    return float(raw.replace(",", "").replace('"', "").strip())


def parse_date_mdy(raw: str) -> str:
    """Convert MM/DD/YYYY to ISO 8601 YYYY-MM-DD."""
    return datetime.strptime(raw.strip(), "%m/%d/%Y").strftime("%Y-%m-%d")


def excel_serial_to_date(serial: int | float) -> str:
    """Convert Excel serial date number to ISO 8601 string."""
    from datetime import timedelta
    base = datetime(1899, 12, 30)  # Excel epoch (accounting for the 1900 leap year bug)
    return (base + timedelta(days=int(serial))).strftime("%Y-%m-%d")


def _is_duplicate_row(conn: sqlite3.Connection, account_id: int, row: ParsedRow) -> bool:
    cursor = conn.execute(
        "SELECT 1 FROM transactions WHERE account_id = ? AND date = ? AND amount = ? AND description = ?",
        (account_id, row.date, row.amount, row.description),
    )
    return cursor.fetchone() is not None


def import_file(
    conn: sqlite3.Connection,
    file_path: Path,
    account_name: str,
    format_key: str | None = None,
) -> dict:
    """Import a file into the database. Returns counts of imported/skipped."""
    cursor = conn.execute(
        "SELECT id, account_type FROM accounts WHERE name = ?", (account_name,)
    )
    account = cursor.fetchone()
    if account is None:
        raise ValueError(f"Unknown account: {account_name}")

    account_id = account["id"]
    account_type = account["account_type"]

    checksum = _compute_checksum(file_path)
    cursor = conn.execute(
        "SELECT 1 FROM imports WHERE checksum = ? AND account_id = ?",
        (checksum, account_id),
    )
    if cursor.fetchone() is not None:
        return {"imported": 0, "skipped": 0, "duplicate_file": True}

    if format_key:
        importer = registry.get_by_key(format_key)
        if importer is None:
            raise ValueError(f"Unknown format: {format_key}")
    else:
        importer = registry.get_for_file(account_type, file_path)
        if importer is None:
            raise ValueError(f"No importer for account type: {account_type}")

    parsed_rows = importer.parse(file_path)

    imported = 0
    skipped = 0
    for row in parsed_rows:
        if _is_duplicate_row(conn, account_id, row):
            skipped += 1
            continue
        conn.execute(
            "INSERT INTO transactions (account_id, date, description, amount, is_flagged, flag_reason) "
            "VALUES (?, ?, ?, ?, 1, 'No matching rule')",
            (account_id, row.date, row.description, row.amount),
        )
        imported += 1

    dates = [r.date for r in parsed_rows]
    conn.execute(
        "INSERT INTO imports (filename, account_id, record_count, date_range_start, date_range_end, checksum) "
        "VALUES (?, ?, ?, ?, ?, ?)",
        (file_path.name, account_id, imported, min(dates) if dates else None, max(dates) if dates else None, checksum),
    )
    conn.commit()

    if importer.post_import:
        importer.post_import(conn, account_id, parsed_rows)

    return {"imported": imported, "skipped": skipped, "duplicate_file": False}
