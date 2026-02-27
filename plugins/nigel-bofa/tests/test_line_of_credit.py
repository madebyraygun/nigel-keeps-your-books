from pathlib import Path
from nigel_bofa.line_of_credit import parse_line_of_credit

FIXTURES = Path(__file__).parent / "fixtures"


def test_parse_line_of_credit():
    rows = parse_line_of_credit(FIXTURES / "bofa_loc_sample.csv")
    assert len(rows) == 3
    assert rows[0].amount == -110.97
    assert "FINANCE CHARGE" in rows[0].description
    assert rows[1].amount == 500.00
