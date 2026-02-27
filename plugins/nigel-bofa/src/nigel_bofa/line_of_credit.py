import csv
from pathlib import Path

from nigel.importer import parse_amount, parse_date_mdy
from nigel.models import ParsedRow


def detect_line_of_credit(file_path: Path) -> bool:
    """Same header as credit card -- differentiated by account_type, not detection."""
    return False


def parse_line_of_credit(file_path: Path) -> list[ParsedRow]:
    """Parse a BofA line of credit CSV."""
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
                date = parse_date_mdy(line[3])
            except ValueError:
                continue
            description = line[5].strip()
            amount = parse_amount(line[6])
            amount = -amount
            rows.append(ParsedRow(date=date, description=description, amount=amount))
    return rows
