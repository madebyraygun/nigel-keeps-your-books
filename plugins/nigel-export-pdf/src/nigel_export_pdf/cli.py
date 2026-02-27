from pathlib import Path

import typer

from nigel_export_pdf.renderer import render_report_to_pdf


def _get_conn_and_data_dir():
    from nigel.db import get_connection
    from nigel.settings import get_data_dir
    data_dir = get_data_dir()
    conn = get_connection(data_dir / "nigel.db")
    return conn, data_dir


def _default_output(data_dir: Path, report_name: str) -> Path:
    from datetime import date
    return data_dir / "exports" / f"{report_name}-{date.today()}.pdf"


def export_pnl(
    year: int = typer.Option(None, help="Year filter"),
    month: str = typer.Option(None, help="Month filter: YYYY-MM"),
    output: Path = typer.Option(None, help="Output PDF path"),
):
    """Export Profit & Loss report to PDF."""
    from nigel.reports import get_pnl
    conn, data_dir = _get_conn_and_data_dir()
    kwargs = {}
    if month:
        parts = month.split("-")
        kwargs = {"year": int(parts[0]), "month": int(parts[1])}
    elif year:
        kwargs = {"year": year}
    data = get_pnl(conn, **kwargs)
    conn.close()

    out = output or _default_output(data_dir, "pnl")
    date_range = month or str(year) if year else "YTD"
    render_report_to_pdf("pnl.html", data, out, title="Profit & Loss", date_range=date_range)
    typer.echo(f"Exported to {out}")


def export_expenses(
    year: int = typer.Option(None, help="Year filter"),
    month: str = typer.Option(None, help="Month filter: YYYY-MM"),
    output: Path = typer.Option(None, help="Output PDF path"),
):
    """Export Expense Breakdown report to PDF."""
    from nigel.reports import get_expense_breakdown
    conn, data_dir = _get_conn_and_data_dir()
    kwargs = {}
    if month:
        parts = month.split("-")
        kwargs = {"year": int(parts[0]), "month": int(parts[1])}
    elif year:
        kwargs = {"year": year}
    data = get_expense_breakdown(conn, **kwargs)
    conn.close()

    out = output or _default_output(data_dir, "expenses")
    date_range = month or str(year) if year else "YTD"
    render_report_to_pdf("expenses.html", data, out, title="Expense Breakdown", date_range=date_range)
    typer.echo(f"Exported to {out}")


def export_tax(
    year: int = typer.Option(None, help="Year filter"),
    output: Path = typer.Option(None, help="Output PDF path"),
):
    """Export Tax Summary report to PDF."""
    from nigel.reports import get_tax_summary
    conn, data_dir = _get_conn_and_data_dir()
    data = get_tax_summary(conn, year=year)
    conn.close()

    out = output or _default_output(data_dir, "tax")
    render_report_to_pdf("tax.html", data, out, title="Tax Summary", date_range=str(year) if year else "YTD")
    typer.echo(f"Exported to {out}")


def export_cashflow(
    year: int = typer.Option(None, help="Year filter"),
    output: Path = typer.Option(None, help="Output PDF path"),
):
    """Export Cash Flow report to PDF."""
    from nigel.reports import get_cashflow
    conn, data_dir = _get_conn_and_data_dir()
    data = get_cashflow(conn, year=year)
    conn.close()

    out = output or _default_output(data_dir, "cashflow")
    render_report_to_pdf("cashflow.html", data, out, title="Cash Flow", date_range=str(year) if year else "YTD")
    typer.echo(f"Exported to {out}")


def export_balance(
    output: Path = typer.Option(None, help="Output PDF path"),
):
    """Export Cash Position report to PDF."""
    from nigel.reports import get_balance
    conn, data_dir = _get_conn_and_data_dir()
    data = get_balance(conn)
    conn.close()

    out = output or _default_output(data_dir, "balance")
    render_report_to_pdf("balance.html", data, out, title="Cash Position")
    typer.echo(f"Exported to {out}")


def export_flagged(
    output: Path = typer.Option(None, help="Output PDF path"),
):
    """Export Flagged Transactions report to PDF."""
    from nigel.reports import get_flagged
    conn, data_dir = _get_conn_and_data_dir()
    rows = get_flagged(conn)
    conn.close()

    data = {"transactions": [dict(r) for r in rows]}
    out = output or _default_output(data_dir, "flagged")
    render_report_to_pdf("flagged.html", data, out, title=f"Flagged Transactions ({len(rows)})")
    typer.echo(f"Exported to {out}")


def export_all(
    year: int = typer.Option(None, help="Year filter"),
    output_dir: Path = typer.Option(None, "--output-dir", help="Output directory"),
):
    """Export all reports to PDF."""
    from datetime import date
    from nigel.reports import get_pnl, get_expense_breakdown, get_tax_summary, get_cashflow, get_balance
    conn, data_dir = _get_conn_and_data_dir()

    out_dir = output_dir or (data_dir / "exports")
    out_dir.mkdir(parents=True, exist_ok=True)
    date_range = str(year) if year else "YTD"
    kwargs = {"year": year} if year else {}

    reports = [
        ("pnl.html", get_pnl(conn, **kwargs), "Profit & Loss", "pnl"),
        ("expenses.html", get_expense_breakdown(conn, **kwargs), "Expense Breakdown", "expenses"),
        ("tax.html", get_tax_summary(conn, **kwargs), "Tax Summary", "tax"),
        ("cashflow.html", get_cashflow(conn, **kwargs), "Cash Flow", "cashflow"),
        ("balance.html", get_balance(conn), "Cash Position", "balance"),
    ]
    conn.close()

    for template, data, title, name in reports:
        out = out_dir / f"{name}-{date.today()}.pdf"
        render_report_to_pdf(template, data, out, title=title, date_range=date_range)
        typer.echo(f"  {out}")

    typer.echo(f"Exported {len(reports)} reports to {out_dir}")
