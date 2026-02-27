import csv
from pathlib import Path

from nigel.importer import parse_amount, parse_date_mdy
from nigel.models import ParsedRow


def detect_credit_card(file_path: Path) -> bool:
    """Return True if file looks like a BofA credit card CSV."""
    try:
        with open(file_path, newline="", encoding="utf-8-sig") as f:
            for line in csv.reader(f):
                if any("CardHolder Name" in cell for cell in line):
                    return True
        return False
    except Exception:
        return False


def parse_credit_card(file_path: Path) -> list[ParsedRow]:
    """Parse a BofA credit card CSV."""
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
            txn_type = line[9].strip()
            if txn_type == "D":
                amount = -abs(amount)
            else:
                amount = abs(amount)
            rows.append(ParsedRow(date=date, description=description, amount=amount))
    return rows
