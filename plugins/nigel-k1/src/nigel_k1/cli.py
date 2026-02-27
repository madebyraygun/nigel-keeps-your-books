import typer
from rich.console import Console
from rich.table import Table

console = Console()


def k1_prep(
    year: int = typer.Option(..., help="Tax year"),
):
    """K-1 prep worksheet — maps transactions to 1120-S / Schedule K line items."""
    from nigel.cli import get_db_path
    from nigel.db import get_connection
    from nigel_k1.reports import (
        get_income_summary, get_line_19_detail, get_schedule_k,
        get_shareholder_worksheets, get_validation_checks,
    )

    conn = get_connection(get_db_path())

    # Entity info
    entity = {}
    for row in conn.execute("SELECT key, value FROM entity_config").fetchall():
        entity[row["key"]] = row["value"]

    entity_name = entity.get("entity_name", "Unknown")
    entity_type = entity.get("entity_type", "s_corp")

    # 1. Income Summary
    income = get_income_summary(conn, year)
    t = Table(title=f"{entity_name} ({entity_type.upper()}) — 1120-S Income Summary — Tax Year {year}")
    t.add_column("Item")
    t.add_column("Amount", justify="right")

    t.add_row("Gross Receipts (Line 1a)", f"${income['gross_receipts']:,.2f}")
    t.add_row("", "")
    t.add_row("[bold]Deductions:[/bold]", "")
    t.add_row("  Officer Compensation (Line 7)", f"(${income['officer_compensation']:,.2f})")
    t.add_row("  Salaries & Wages (Line 8)", f"(${income['salaries_wages']:,.2f})")
    t.add_row("  Rents (Line 11)", f"(${income['rents']:,.2f})")
    t.add_row("  Taxes & Licenses (Line 12)", f"(${income['taxes_licenses']:,.2f})")
    t.add_row("  Advertising (Line 16)", f"(${income['advertising']:,.2f})")
    t.add_row("  Employee Benefits (Line 18)", f"(${income['employee_benefits']:,.2f})")
    t.add_row("  Other Deductions (Line 19)", f"(${income['other_deductions']:,.2f})")
    t.add_row("", "")
    t.add_row("[bold]Total Deductions (Line 20)[/bold]", f"[bold](${abs(income['total_deductions']):,.2f})[/bold]")
    t.add_row("", "")
    net_color = "green" if income["ordinary_business_income"] >= 0 else "red"
    t.add_row(
        f"[bold {net_color}]Ordinary Business Income (Line 21)[/bold {net_color}]",
        f"[bold {net_color}]${income['ordinary_business_income']:,.2f}[/bold {net_color}]",
    )
    console.print(t)
    console.print()

    # 2. Line 19 Detail
    detail = get_line_19_detail(conn, year)
    if detail:
        t2 = Table(title="Other Deductions (Line 19) — Detail")
        t2.add_column("Category")
        t2.add_column("Full Amount", justify="right")
        t2.add_column("Deductible", justify="right")
        total_deductible = 0.0
        for d in detail:
            note = " (50%)" if d["name"] == "Meals" else ""
            t2.add_row(
                f"{d['name']}{note}",
                f"${d['full_amount']:,.2f}",
                f"${d['deductible_amount']:,.2f}",
            )
            total_deductible += d["deductible_amount"]
        t2.add_row("[bold]Total[/bold]", "", f"[bold]${total_deductible:,.2f}[/bold]")
        console.print(t2)
        console.print()

    # 3. Schedule K
    k = get_schedule_k(conn, year)
    t3 = Table(title="Schedule K — S-Corp Items")
    t3.add_column("Line")
    t3.add_column("Description")
    t3.add_column("Amount", justify="right")
    t3.add_row("1", "Ordinary business income", f"${k['line_1']:,.2f}")
    t3.add_row("4", "Interest income", f"${k['line_4']:,.2f}")
    t3.add_row("11", "Section 179 deduction", f"(${k['line_11']:,.2f})")
    t3.add_row("12a", "Charitable contributions", f"(${k['line_12a']:,.2f})")
    t3.add_row("16d", "Distributions", f"${k['line_16d']:,.2f}")
    console.print(t3)
    console.print()

    # 4. Per-Shareholder Worksheets
    worksheets = get_shareholder_worksheets(conn, year)
    for ws in worksheets:
        t4 = Table(title=f"K-1 Worksheet — {ws['name']} ({ws['ownership_pct']:.0%})")
        t4.add_column("Item")
        t4.add_column("Amount", justify="right")
        t4.add_row("Line 1: Ordinary business income", f"${ws['line_1']:,.2f}")
        t4.add_row("Line 4: Interest income", f"${ws['line_4']:,.2f}")
        t4.add_row("Line 11: Section 179 deduction", f"(${ws['line_11']:,.2f})")
        t4.add_row("Line 12a: Charitable contributions", f"(${ws['line_12a']:,.2f})")
        t4.add_row("Line 16d: Distributions", f"${ws['line_16d']:,.2f}")
        if ws["is_officer"]:
            t4.add_row("", "")
            t4.add_row("[dim]W-2 wages (via Gusto)[/dim]", f"[dim]${ws['annual_compensation']:,.2f}[/dim]")
        console.print(t4)
        console.print()

    # 5. Validation
    checks = get_validation_checks(conn, year)
    if checks["uncategorized_count"] > 0:
        console.print(f"[bold yellow]WARNING:[/bold yellow] {checks['uncategorized_count']} uncategorized transactions in {year}")
    if checks["comp_to_distribution_warning"]:
        console.print(
            f"[bold yellow]WARNING:[/bold yellow] Distributions (${checks['total_distributions']:,.2f}) "
            f"exceed 2x officer compensation (${checks['officer_compensation']:,.2f}) — "
            f"review reasonable compensation."
        )

    conn.close()


def k1_setup():
    """Configure entity and shareholder details for K-1 reporting."""
    from nigel.cli import get_db_path
    from nigel.db import get_connection

    conn = get_connection(get_db_path())

    # Entity config
    entity_name = typer.prompt("Entity name", default="")
    entity_type = typer.prompt("Entity type (s_corp or partnership)", default="s_corp")
    ein = typer.prompt("EIN (optional)", default="")
    tax_year = typer.prompt("Tax year", default="2025")

    for key, value in [
        ("entity_name", entity_name),
        ("entity_type", entity_type),
        ("ein", ein),
        ("tax_year", tax_year),
    ]:
        conn.execute(
            "INSERT INTO entity_config (key, value) VALUES (?, ?) "
            "ON CONFLICT(key) DO UPDATE SET value = ?",
            (key, value, value),
        )

    # Shareholders
    console.print("\n[bold]Shareholders[/bold] (enter blank name to finish)")
    while True:
        name = typer.prompt("Shareholder name", default="")
        if not name:
            break
        pct = typer.prompt("Ownership %", type=float)
        is_officer = typer.confirm("Is officer (receives W-2)?", default=True)
        comp = 0.0
        if is_officer:
            comp = typer.prompt("Annual W-2 compensation", type=float, default=0.0)

        conn.execute(
            "INSERT INTO shareholders (name, ownership_pct, is_officer, annual_compensation) "
            "VALUES (?, ?, ?, ?)",
            (name, pct / 100.0, int(is_officer), comp),
        )

    conn.commit()
    conn.close()
    typer.echo("K-1 entity configuration saved.")
