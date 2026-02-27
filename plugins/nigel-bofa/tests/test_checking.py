from pathlib import Path
from nigel_bofa.checking import parse_checking, detect_checking

FIXTURES = Path(__file__).parent / "fixtures"


def test_detect_checking():
    assert detect_checking(FIXTURES / "bofa_checking_sample.csv") is True
    assert detect_checking(FIXTURES / "bofa_credit_card_sample.csv") is False


def test_parse_checking():
    rows = parse_checking(FIXTURES / "bofa_checking_sample.csv")
    assert len(rows) == 5
    assert rows[0].date == "2024-12-02"
    assert rows[0].amount == -2500.00
    assert "ACME CORP" in rows[0].description
    assert rows[3].date == "2025-01-09"
    assert rows[3].amount == 5000.00


def test_parse_checking_skips_preamble():
    rows = parse_checking(FIXTURES / "bofa_checking_sample.csv")
    for row in rows:
        assert "Beginning balance" not in row.description
        assert "Summary" not in row.description
