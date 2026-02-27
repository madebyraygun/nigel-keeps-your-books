from pathlib import Path

from nigel_export_pdf.renderer import render_report_to_pdf


def _assert_valid_pdf(path: Path):
    assert path.exists()
    assert path.stat().st_size > 0
    assert path.read_bytes()[:5] == b"%PDF-"


def test_render_pnl(tmp_path):
    data = {
        "income": [{"name": "Client Services", "total": 10000.0}],
        "expenses": [{"name": "Software", "total": -500.0}],
        "total_income": 10000.0,
        "total_expenses": -500.0,
        "net": 9500.0,
    }
    out = tmp_path / "pnl.pdf"
    render_report_to_pdf("pnl.html", data, out, title="P&L Test")
    _assert_valid_pdf(out)


def test_render_expenses(tmp_path):
    data = {
        "categories": [{"name": "Software", "total": -500.0, "count": 3, "pct": 100.0}],
        "total": -500.0,
        "top_vendors": [{"vendor": "Adobe", "total": -300.0, "count": 2}],
    }
    out = tmp_path / "expenses.pdf"
    render_report_to_pdf("expenses.html", data, out, title="Expenses Test")
    _assert_valid_pdf(out)


def test_render_tax(tmp_path):
    data = {
        "line_items": [{"name": "Software", "tax_line": "Line 18", "type": "expense", "total": -500.0}],
    }
    out = tmp_path / "tax.pdf"
    render_report_to_pdf("tax.html", data, out, title="Tax Test")
    _assert_valid_pdf(out)


def test_render_cashflow(tmp_path):
    data = {
        "months": [{"month": "2025-01", "inflows": 10000.0, "outflows": -5000.0, "net": 5000.0, "running_balance": 5000.0}],
    }
    out = tmp_path / "cashflow.pdf"
    render_report_to_pdf("cashflow.html", data, out, title="Cashflow Test")
    _assert_valid_pdf(out)


def test_render_balance(tmp_path):
    data = {
        "accounts": [{"name": "Checking", "type": "checking", "balance": 15000.0}],
        "total": 15000.0,
        "ytd_net_income": 9500.0,
    }
    out = tmp_path / "balance.pdf"
    render_report_to_pdf("balance.html", data, out, title="Balance Test")
    _assert_valid_pdf(out)


def test_render_flagged(tmp_path):
    data = {
        "transactions": [
            {"id": 1, "date": "2025-01-15", "description": "Unknown Vendor", "amount": -99.0, "account_name": "Checking"},
        ],
    }
    out = tmp_path / "flagged.pdf"
    render_report_to_pdf("flagged.html", data, out, title="Flagged Test")
    _assert_valid_pdf(out)


def test_render_flagged_empty(tmp_path):
    data = {"transactions": []}
    out = tmp_path / "flagged_empty.pdf"
    render_report_to_pdf("flagged.html", data, out, title="Flagged Empty")
    _assert_valid_pdf(out)
