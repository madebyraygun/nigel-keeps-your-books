import csv
import hashlib
import sqlite3
from datetime import datetime
from pathlib import Path

from bookkeeper.models import ParsedRow


def _compute_checksum(file_path: Path) -> str:
    return hashlib.sha256(file_path.read_bytes()).hexdigest()


def _parse_amount(raw: str) -> float:
    """Strip commas and quotes, return float."""
    return float(raw.replace(",", "").replace('"', "").strip())


def _parse_date_mdy(raw: str) -> str:
    """Convert MM/DD/YYYY to ISO 8601 YYYY-MM-DD."""
    return datetime.strptime(raw.strip(), "%m/%d/%Y").strftime("%Y-%m-%d")


def parse_bofa_checking(file_path: Path) -> list[ParsedRow]:
    """Parse a BofA checking CSV, skipping the preamble rows."""
    rows: list[ParsedRow] = []
    with open(file_path, newline="", encoding="utf-8-sig") as f:
        reader = csv.reader(f)
        # Find the header row: "Date,Description,Amount,Running Bal."
        for line in reader:
            if len(line) >= 4 and line[0].strip() == "Date" and "Description" in line[1]:
                break
        # Parse data rows
        for line in reader:
            if len(line) < 3 or not line[0].strip():
                continue
            try:
                date = _parse_date_mdy(line[0])
            except ValueError:
                continue
            description = line[1].strip()
            if not description or "Beginning balance" in description:
                continue
            amount = _parse_amount(line[2])
            rows.append(ParsedRow(date=date, description=description, amount=amount))
    return rows


def parse_bofa_credit_card(file_path: Path) -> list[ParsedRow]:
    """Parse a BofA credit card CSV. Amounts are always positive; use Transaction Type for sign."""
    rows: list[ParsedRow] = []
    with open(file_path, newline="", encoding="utf-8-sig") as f:
        reader = csv.reader(f)
        # Find header row containing "Posting Date"
        for line in reader:
            if any("Posting Date" in cell for cell in line):
                break
        for line in reader:
            if len(line) < 10 or not line[2].strip():
                continue
            try:
                date = _parse_date_mdy(line[3])  # Trans. Date
            except ValueError:
                continue
            description = line[5].strip()
            amount = _parse_amount(line[6])
            txn_type = line[9].strip()
            # D = charge (expense), C = credit/payment
            if txn_type == "D":
                amount = -abs(amount)
            else:
                amount = abs(amount)
            rows.append(ParsedRow(date=date, description=description, amount=amount))
    return rows


def parse_bofa_line_of_credit(file_path: Path) -> list[ParsedRow]:
    """Parse a BofA line of credit CSV. Same columns as credit card but amounts are pre-signed."""
    rows: list[ParsedRow] = []
    with open(file_path, newline="", encoding="utf-8-sig") as f:
        reader = csv.reader(f)
        for line in reader:
            if any("Posting Date" in cell for cell in line):
                break
        for line in reader:
            if len(line) < 10 or not line[2].strip():
                continue
            try:
                date = _parse_date_mdy(line[3])  # Trans. Date
            except ValueError:
                continue
            description = line[5].strip()
            amount = _parse_amount(line[6])
            # Line of credit: positive = charge/expense (negate), negative = payment (keep)
            amount = -amount
            rows.append(ParsedRow(date=date, description=description, amount=amount))
    return rows


PARSER_MAP = {
    "checking": parse_bofa_checking,
    "credit_card": parse_bofa_credit_card,
    "line_of_credit": parse_bofa_line_of_credit,
}


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
) -> dict:
    """Import a CSV file into the database. Returns counts of imported/skipped."""
    # Look up account
    cursor = conn.execute(
        "SELECT id, account_type FROM accounts WHERE name = ?", (account_name,)
    )
    account = cursor.fetchone()
    if account is None:
        raise ValueError(f"Unknown account: {account_name}")

    account_id = account["id"]
    account_type = account["account_type"]

    # Check file-level duplicate
    checksum = _compute_checksum(file_path)
    cursor = conn.execute(
        "SELECT 1 FROM imports WHERE checksum = ? AND account_id = ?",
        (checksum, account_id),
    )
    if cursor.fetchone() is not None:
        return {"imported": 0, "skipped": 0, "duplicate_file": True}

    # Parse
    parser = PARSER_MAP.get(account_type)
    if parser is None:
        raise ValueError(f"No parser for account type: {account_type}")
    parsed_rows = parser(file_path)

    # Insert
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

    # Record import batch
    dates = [r.date for r in parsed_rows]
    conn.execute(
        "INSERT INTO imports (filename, account_id, record_count, date_range_start, date_range_end, checksum) "
        "VALUES (?, ?, ?, ?, ?, ?)",
        (
            file_path.name,
            account_id,
            imported,
            min(dates) if dates else None,
            max(dates) if dates else None,
            checksum,
        ),
    )
    conn.commit()

    return {"imported": imported, "skipped": skipped, "duplicate_file": False}
