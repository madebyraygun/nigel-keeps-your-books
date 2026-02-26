from pathlib import Path

from bookkeeper.importer import parse_bofa_checking, import_file
from bookkeeper.db import init_db, get_connection

FIXTURES = Path(__file__).parent / "fixtures"


def test_parse_bofa_checking():
    rows = parse_bofa_checking(FIXTURES / "bofa_checking_sample.csv")
    assert len(rows) == 5
    # First transaction
    assert rows[0].date == "2024-12-02"
    assert rows[0].amount == -2500.00
    assert "ACME CORP" in rows[0].description
    # Positive amount (deposit)
    assert rows[3].date == "2025-01-09"
    assert rows[3].amount == 5000.00


def test_parse_bofa_checking_skips_preamble():
    rows = parse_bofa_checking(FIXTURES / "bofa_checking_sample.csv")
    for row in rows:
        assert "Beginning balance" not in row.description
        assert "Summary" not in row.description


def test_import_file_inserts_transactions(db):
    db.execute(
        "INSERT INTO accounts (name, account_type) VALUES (?, ?)",
        ("BofA Checking", "checking"),
    )
    db.commit()

    result = import_file(db, FIXTURES / "bofa_checking_sample.csv", "BofA Checking")
    assert result["imported"] == 5
    assert result["skipped"] == 0

    cursor = db.execute("SELECT count(*) FROM transactions")
    assert cursor.fetchone()[0] == 5


def test_import_file_detects_file_duplicate(db):
    db.execute(
        "INSERT INTO accounts (name, account_type) VALUES (?, ?)",
        ("BofA Checking", "checking"),
    )
    db.commit()

    import_file(db, FIXTURES / "bofa_checking_sample.csv", "BofA Checking")
    result = import_file(db, FIXTURES / "bofa_checking_sample.csv", "BofA Checking")
    assert result["duplicate_file"] is True


def test_import_file_detects_row_duplicates(db, tmp_path):
    db.execute(
        "INSERT INTO accounts (name, account_type) VALUES (?, ?)",
        ("BofA Checking", "checking"),
    )
    db.commit()

    import_file(db, FIXTURES / "bofa_checking_sample.csv", "BofA Checking")

    # Create a different file with overlapping rows + one new row
    overlap_file = tmp_path / "overlap.csv"
    overlap_file.write_text(
        'Date,Description,Amount,Running Bal.\n'
        '12/02/2024,"ACME CORP DES:PAYMENT ID:12345 INDN:Raygun Design, LLC CO ID:XXXXX42850 CCD","-2,500.00","20,048.05"\n'
        '02/01/2025,"NEW VENDOR PAYMENT","-100.00","24,543.33"\n'
    )
    result = import_file(db, overlap_file, "BofA Checking")
    assert result["imported"] == 1
    assert result["skipped"] == 1

    cursor = db.execute("SELECT count(*) FROM transactions")
    assert cursor.fetchone()[0] == 6


def test_import_file_records_import_batch(db):
    db.execute(
        "INSERT INTO accounts (name, account_type) VALUES (?, ?)",
        ("BofA Checking", "checking"),
    )
    db.commit()

    import_file(db, FIXTURES / "bofa_checking_sample.csv", "BofA Checking")

    cursor = db.execute("SELECT * FROM imports")
    record = cursor.fetchone()
    assert record["filename"] == "bofa_checking_sample.csv"
    assert record["record_count"] == 5
    assert record["checksum"] is not None
