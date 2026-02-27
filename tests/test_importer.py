import csv
from pathlib import Path

from nigel.importer import import_file, parse_amount, parse_date_mdy, excel_serial_to_date
from nigel.models import ImporterInfo, ParsedRow
from nigel.registry import registry


def _create_synthetic_csv(path: Path) -> None:
    """Write a minimal CSV that our test importer can parse."""
    with open(path, "w", newline="") as f:
        writer = csv.writer(f)
        writer.writerow(["date", "description", "amount"])
        writer.writerow(["01/15/2025", "VENDOR A payment", "-500.00"])
        writer.writerow(["01/16/2025", "CLIENT B deposit", "2000.00"])
        writer.writerow(["01/20/2025", "VENDOR C charge", "-75.50"])


def _parse_synthetic(file_path: Path) -> list[ParsedRow]:
    rows = []
    with open(file_path, newline="") as f:
        reader = csv.DictReader(f)
        for row in reader:
            rows.append(ParsedRow(
                date=parse_date_mdy(row["date"]),
                description=row["description"],
                amount=parse_amount(row["amount"]),
            ))
    return rows


# Register a test importer (unique account_type to avoid collision with plugins)
registry.register(ImporterInfo(
    key="synthetic_test", name="Synthetic Test",
    account_types=["synthetic"], file_extensions=[".csv"],
    parse=_parse_synthetic,
))


def test_import_file_inserts_transactions(db, tmp_path):
    db.execute("INSERT INTO accounts (name, account_type) VALUES (?, ?)", ("Test Acct", "synthetic"))
    db.commit()

    csv_file = tmp_path / "test.csv"
    _create_synthetic_csv(csv_file)

    result = import_file(db, csv_file, "Test Acct")
    assert result["imported"] == 3
    assert result["skipped"] == 0

    cursor = db.execute("SELECT count(*) FROM transactions")
    assert cursor.fetchone()[0] == 3


def test_import_file_detects_file_duplicate(db, tmp_path):
    db.execute("INSERT INTO accounts (name, account_type) VALUES (?, ?)", ("Test Acct", "synthetic"))
    db.commit()

    csv_file = tmp_path / "test.csv"
    _create_synthetic_csv(csv_file)

    import_file(db, csv_file, "Test Acct")
    result = import_file(db, csv_file, "Test Acct")
    assert result["duplicate_file"] is True


def test_import_file_detects_row_duplicates(db, tmp_path):
    db.execute("INSERT INTO accounts (name, account_type) VALUES (?, ?)", ("Test Acct", "synthetic"))
    db.commit()

    csv_file = tmp_path / "test.csv"
    _create_synthetic_csv(csv_file)
    import_file(db, csv_file, "Test Acct")

    overlap = tmp_path / "overlap.csv"
    with open(overlap, "w", newline="") as f:
        writer = csv.writer(f)
        writer.writerow(["date", "description", "amount"])
        writer.writerow(["01/15/2025", "VENDOR A payment", "-500.00"])  # duplicate
        writer.writerow(["02/01/2025", "NEW VENDOR", "-100.00"])  # new

    result = import_file(db, overlap, "Test Acct")
    assert result["imported"] == 1
    assert result["skipped"] == 1

    cursor = db.execute("SELECT count(*) FROM transactions")
    assert cursor.fetchone()[0] == 4


def test_import_file_records_batch(db, tmp_path):
    db.execute("INSERT INTO accounts (name, account_type) VALUES (?, ?)", ("Test Acct", "synthetic"))
    db.commit()

    csv_file = tmp_path / "test.csv"
    _create_synthetic_csv(csv_file)
    import_file(db, csv_file, "Test Acct")

    record = db.execute("SELECT * FROM imports").fetchone()
    assert record["filename"] == "test.csv"
    assert record["record_count"] == 3
    assert record["checksum"] is not None


def test_import_file_with_format_key(db, tmp_path):
    db.execute("INSERT INTO accounts (name, account_type) VALUES (?, ?)", ("Test Acct", "synthetic"))
    db.commit()

    csv_file = tmp_path / "test.csv"
    _create_synthetic_csv(csv_file)

    result = import_file(db, csv_file, "Test Acct", format_key="synthetic_test")
    assert result["imported"] == 3


def test_public_helpers():
    assert parse_amount("1,234.56") == 1234.56
    assert parse_date_mdy("01/15/2025") == "2025-01-15"
    assert excel_serial_to_date(45667) == "2025-01-10"
