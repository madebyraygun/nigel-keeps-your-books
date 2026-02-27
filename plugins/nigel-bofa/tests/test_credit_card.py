from pathlib import Path
from nigel_bofa.credit_card import parse_credit_card, detect_credit_card

FIXTURES = Path(__file__).parent / "fixtures"


def test_detect_credit_card():
    assert detect_credit_card(FIXTURES / "bofa_credit_card_sample.csv") is True
    assert detect_credit_card(FIXTURES / "bofa_checking_sample.csv") is False


def test_parse_credit_card():
    rows = parse_credit_card(FIXTURES / "bofa_credit_card_sample.csv")
    assert len(rows) == 3
    assert rows[0].amount == -54.43
    assert rows[0].date == "2025-03-10"
    assert "ADOBE" in rows[0].description
    assert rows[2].amount == 500.00
