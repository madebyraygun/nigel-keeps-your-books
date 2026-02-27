import csv
from pathlib import Path

from nigel.importer import parse_amount, parse_date_mdy
from nigel.models import ParsedRow


def detect_checking(file_path: Path) -> bool:
    """Return True if file looks like a BofA checking CSV."""
    try:
        with open(file_path, newline="", encoding="utf-8-sig") as f:
            for line in csv.reader(f):
                if len(line) >= 4 and line[0].strip() == "Date" and "Description" in line[1]:
                    return True
        return False
    except Exception:
        return False


def parse_checking(file_path: Path) -> list[ParsedRow]:
    """Parse a BofA checking CSV, skipping the preamble rows."""
    rows: list[ParsedRow] = []
    with open(file_path, newline="", encoding="utf-8-sig") as f:
        reader = csv.reader(f)
        for line in reader:
            if len(line) >= 4 and line[0].strip() == "Date" and "Description" in line[1]:
                break
        for line in reader:
            if len(line) < 3 or not line[0].strip():
                continue
            try:
                date = parse_date_mdy(line[0])
            except ValueError:
                continue
            description = line[1].strip()
            if not description or "Beginning balance" in description:
                continue
            amount = parse_amount(line[2])
            rows.append(ParsedRow(date=date, description=description, amount=amount))
    return rows
