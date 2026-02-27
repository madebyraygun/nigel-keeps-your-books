from pathlib import Path

import typer

from nigel.db import get_connection, init_db
from nigel.settings import get_data_dir, load_settings, save_settings, DEFAULTS
from nigel.plugins import load_plugins, apply_migrations, seed_plugin_categories

app = typer.Typer(help="Nigel — cash-basis bookkeeping CLI for small consultancies.", invoke_without_command=True)

report_app = typer.Typer(help="Generate reports.")
app.add_typer(report_app, name="report")

_plugin_hooks = load_plugins(app, report_app)


@app.callback()
def main():
    """Nigel — cash-basis bookkeeping CLI for small consultancies."""


def get_db_path() -> Path:
    return get_data_dir() / "nigel.db"


@app.command()
def init(
    data_dir: str = typer.Option(None, "--data-dir", help="Path for Nigel data (default: ~/Documents/nigel)"),
):
    """Set up Nigel: choose a data directory and initialize the database."""
    settings = load_settings()

    if data_dir:
        settings["data_dir"] = str(Path(data_dir).expanduser().resolve())
    elif settings == DEFAULTS:
        # First run — prompt for data dir
        default = settings["data_dir"]
        chosen = typer.prompt("Data directory", default=default)
        settings["data_dir"] = str(Path(chosen).expanduser().resolve())

    save_settings(settings)

    resolved = Path(settings["data_dir"])
    resolved.mkdir(parents=True, exist_ok=True)
    (resolved / "imports").mkdir(exist_ok=True)
    (resolved / "exports").mkdir(exist_ok=True)

    conn = get_connection(resolved / "nigel.db")
    init_db(conn)
    apply_migrations(conn, _plugin_hooks)
    seed_plugin_categories(conn, _plugin_hooks)
    conn.close()

    typer.echo(f"Initialized nigel at {resolved}")


# --- Accounts ---

from rich.console import Console
from rich.table import Table

console = Console()

accounts_app = typer.Typer(help="Manage accounts.")
app.add_typer(accounts_app, name="accounts")


@accounts_app.command("add")
def accounts_add(
    name: str = typer.Argument(help="Account name, e.g. 'BofA Checking'"),
    type: str = typer.Option(help="Account type: checking, credit_card, line_of_credit, payroll"),
    institution: str = typer.Option(None, help="Institution name"),
    last_four: str = typer.Option(None, help="Last 4 digits of account number"),
):
    """Add a new account."""
    conn = get_connection(get_db_path())
    conn.execute(
        "INSERT INTO accounts (name, account_type, institution, last_four) VALUES (?, ?, ?, ?)",
        (name, type, institution, last_four),
    )
    conn.commit()
    conn.close()
    typer.echo(f"Added account: {name}")


@accounts_app.command("list")
def accounts_list():
    """List all accounts."""
    conn = get_connection(get_db_path())
    rows = conn.execute("SELECT id, name, account_type, institution, last_four FROM accounts").fetchall()
    conn.close()

    table = Table(title="Accounts")
    table.add_column("ID", style="dim")
    table.add_column("Name")
    table.add_column("Type")
    table.add_column("Institution")
    table.add_column("Last Four")
    for row in rows:
        table.add_row(str(row["id"]), row["name"], row["account_type"], row["institution"] or "", row["last_four"] or "")
    console.print(table)


# --- Import ---

from nigel.importer import import_file
from nigel.categorizer import categorize_transactions


@app.command("import")
def import_cmd(
    file: Path = typer.Argument(help="Path to CSV or XLSX file to import"),
    account: str = typer.Option(help="Account name to import into"),
):
    """Import a CSV/XLSX file and auto-categorize transactions."""
    import shutil

    conn = get_connection(get_db_path())
    result = import_file(conn, file, account)

    if result.get("duplicate_file"):
        conn.close()
        typer.echo("This file has already been imported (duplicate checksum).")
        return

    typer.echo(f"{result['imported']} imported, {result['skipped']} skipped (duplicates)")

    # Auto-categorize
    cat_result = categorize_transactions(conn)
    typer.echo(f"{cat_result['categorized']} categorized, {cat_result['still_flagged']} still flagged")
    conn.close()

    # Archive the import file
    dest = get_data_dir() / "imports" / file.name
    if not dest.exists():
        shutil.copy2(file, dest)


# --- Categorize ---


@app.command()
def categorize():
    """Re-run categorization rules on uncategorized transactions."""
    conn = get_connection(get_db_path())
    result = categorize_transactions(conn)
    conn.close()
    typer.echo(f"{result['categorized']} categorized, {result['still_flagged']} still flagged")


# --- Rules ---

rules_app = typer.Typer(help="Manage categorization rules.")
app.add_typer(rules_app, name="rules")


@rules_app.command("add")
def rules_add(
    pattern: str = typer.Argument(help="Pattern to match against transaction descriptions"),
    category: str = typer.Option(help="Category name to assign"),
    vendor: str = typer.Option(None, help="Normalized vendor name"),
    match_type: str = typer.Option("contains", help="Match type: contains, starts_with, regex"),
    priority: int = typer.Option(0, help="Rule priority (higher wins)"),
):
    """Add a categorization rule."""
    conn = get_connection(get_db_path())
    cat = conn.execute("SELECT id FROM categories WHERE name = ?", (category,)).fetchone()
    if cat is None:
        typer.echo(f"Unknown category: {category}")
        raise typer.Exit(1)
    conn.execute(
        "INSERT INTO rules (pattern, match_type, vendor, category_id, priority) VALUES (?, ?, ?, ?, ?)",
        (pattern, match_type, vendor, cat["id"], priority),
    )
    conn.commit()
    conn.close()
    typer.echo(f"Added rule: '{pattern}' → {category}")


@rules_app.command("list")
def rules_list():
    """List all categorization rules."""
    conn = get_connection(get_db_path())
    rows = conn.execute(
        "SELECT r.id, r.pattern, r.match_type, r.vendor, c.name as category, r.priority, r.hit_count "
        "FROM rules r JOIN categories c ON r.category_id = c.id "
        "WHERE r.is_active = 1 ORDER BY r.priority DESC"
    ).fetchall()
    conn.close()

    table = Table(title="Rules")
    table.add_column("ID", style="dim")
    table.add_column("Pattern")
    table.add_column("Type")
    table.add_column("Vendor")
    table.add_column("Category")
    table.add_column("Priority")
    table.add_column("Hits")
    for row in rows:
        table.add_row(
            str(row["id"]), row["pattern"], row["match_type"],
            row["vendor"] or "", row["category"],
            str(row["priority"]), str(row["hit_count"]),
        )
    console.print(table)


# --- Review ---

from nigel.reviewer import run_review


@app.command()
def review():
    """Interactively review flagged transactions."""
    conn = get_connection(get_db_path())
    run_review(conn)
    conn.close()


# --- Reports ---

from nigel.reports import (
    get_pnl, get_expense_breakdown, get_tax_summary,
    get_cashflow, get_flagged, get_balance,
)

def _parse_date_opts(month: str | None, year: int | None, from_date: str | None, to_date: str | None) -> dict:
    if from_date and to_date:
        return {"from_date": from_date, "to_date": to_date}
    if month:
        parts = month.split("-")
        return {"year": int(parts[0]), "month": int(parts[1])}
    if year:
        return {"year": year}
    return {}


@report_app.command("pnl")
def report_pnl(
    month: str = typer.Option(None, help="Month filter: YYYY-MM"),
    year: int = typer.Option(None, help="Year filter: YYYY"),
    from_date: str = typer.Option(None, "--from", help="Start date: YYYY-MM-DD"),
    to_date: str = typer.Option(None, "--to", help="End date: YYYY-MM-DD"),
):
    """Profit & Loss report."""
    conn = get_connection(get_db_path())
    pnl = get_pnl(conn, **_parse_date_opts(month, year, from_date, to_date))
    conn.close()

    table = Table(title="Profit & Loss")
    table.add_column("Category")
    table.add_column("Amount", justify="right")

    if pnl["income"]:
        table.add_row("[bold green]INCOME[/bold green]", "")
        for item in pnl["income"]:
            table.add_row(f"  {item['name']}", f"${item['total']:,.2f}")
        table.add_row("[bold]Total Income[/bold]", f"[bold]${pnl['total_income']:,.2f}[/bold]")
        table.add_row("", "")

    if pnl["expenses"]:
        table.add_row("[bold red]EXPENSES[/bold red]", "")
        for item in pnl["expenses"]:
            table.add_row(f"  {item['name']}", f"${abs(item['total']):,.2f}")
        table.add_row("[bold]Total Expenses[/bold]", f"[bold]${abs(pnl['total_expenses']):,.2f}[/bold]")
        table.add_row("", "")

    color = "green" if pnl["net"] >= 0 else "red"
    table.add_row(f"[bold {color}]NET[/bold {color}]", f"[bold {color}]${pnl['net']:,.2f}[/bold {color}]")
    console.print(table)


@report_app.command("expenses")
def report_expenses(
    month: str = typer.Option(None, help="Month filter: YYYY-MM"),
    year: int = typer.Option(None, help="Year filter: YYYY"),
):
    """Expense breakdown report."""
    conn = get_connection(get_db_path())
    data = get_expense_breakdown(conn, **_parse_date_opts(month, year, None, None))
    conn.close()

    table = Table(title="Expense Breakdown")
    table.add_column("Category")
    table.add_column("Amount", justify="right")
    table.add_column("%", justify="right")
    table.add_column("Count", justify="right")
    for item in data["categories"]:
        table.add_row(item["name"], f"${abs(item['total']):,.2f}", f"{item['pct']:.1f}%", str(item["count"]))
    table.add_row("[bold]Total[/bold]", f"[bold]${abs(data['total']):,.2f}[/bold]", "", "")
    console.print(table)

    if data["top_vendors"]:
        vtable = Table(title="Top Vendors")
        vtable.add_column("Vendor")
        vtable.add_column("Amount", justify="right")
        vtable.add_column("Count", justify="right")
        for v in data["top_vendors"]:
            vtable.add_row(v["vendor"], f"${abs(v['total']):,.2f}", str(v["count"]))
        console.print(vtable)


@report_app.command("tax")
def report_tax(year: int = typer.Option(None, help="Year filter: YYYY")):
    """Tax summary organized by IRS line items."""
    conn = get_connection(get_db_path())
    data = get_tax_summary(conn, year=year)
    conn.close()

    table = Table(title="Tax Summary")
    table.add_column("Category")
    table.add_column("Tax Line")
    table.add_column("Type")
    table.add_column("Amount", justify="right")
    for item in data["line_items"]:
        table.add_row(item["name"], item["tax_line"] or "", item["type"], f"${abs(item['total']):,.2f}")
    console.print(table)


@report_app.command("cashflow")
def report_cashflow(
    month: str = typer.Option(None, help="Month filter: YYYY-MM"),
    year: int = typer.Option(None, help="Year filter: YYYY"),
):
    """Cash flow report with monthly inflows/outflows."""
    conn = get_connection(get_db_path())
    data = get_cashflow(conn, **_parse_date_opts(month, year, None, None))
    conn.close()

    table = Table(title="Cash Flow")
    table.add_column("Month")
    table.add_column("Inflows", justify="right", style="green")
    table.add_column("Outflows", justify="right", style="red")
    table.add_column("Net", justify="right")
    table.add_column("Running", justify="right")
    for m in data["months"]:
        net_color = "green" if m["net"] >= 0 else "red"
        table.add_row(
            m["month"],
            f"${m['inflows']:,.2f}",
            f"${abs(m['outflows']):,.2f}",
            f"[{net_color}]${m['net']:,.2f}[/{net_color}]",
            f"${m['running_balance']:,.2f}",
        )
    console.print(table)


@report_app.command("flagged")
def report_flagged():
    """Show all flagged/uncategorized transactions."""
    conn = get_connection(get_db_path())
    rows = get_flagged(conn)
    conn.close()

    if not rows:
        typer.echo("No flagged transactions.")
        return

    table = Table(title=f"Flagged Transactions ({len(rows)})")
    table.add_column("ID", style="dim")
    table.add_column("Date")
    table.add_column("Description")
    table.add_column("Amount", justify="right")
    table.add_column("Account")
    for r in rows:
        color = "red" if r["amount"] < 0 else "green"
        table.add_row(str(r["id"]), r["date"], r["description"], f"[{color}]${abs(r['amount']):,.2f}[/{color}]", r["account_name"])
    console.print(table)


@report_app.command("balance")
def report_balance():
    """Cash position snapshot."""
    conn = get_connection(get_db_path())
    data = get_balance(conn)
    conn.close()

    table = Table(title="Cash Position")
    table.add_column("Account")
    table.add_column("Type", style="dim")
    table.add_column("Balance", justify="right")
    for a in data["accounts"]:
        color = "green" if a["balance"] >= 0 else "red"
        table.add_row(a["name"], a["type"], f"[{color}]${a['balance']:,.2f}[/{color}]")
    table.add_row("[bold]Total[/bold]", "", f"[bold]${data['total']:,.2f}[/bold]")
    console.print(table)
    console.print(f"\nYTD Net Income: ${data['ytd_net_income']:,.2f}")


# --- Reconcile ---

from nigel.reconciler import reconcile


@app.command("reconcile")
def reconcile_cmd(
    account: str = typer.Argument(help="Account name"),
    month: str = typer.Option(help="Month: YYYY-MM"),
    balance: float = typer.Option(help="Statement ending balance"),
):
    """Reconcile an account against a statement balance."""
    conn = get_connection(get_db_path())
    result = reconcile(conn, account_name=account, month=month, statement_balance=balance)
    conn.close()

    if result["is_reconciled"]:
        typer.echo(f"Reconciled! Calculated: ${result['calculated_balance']:,.2f}")
    else:
        typer.echo(
            f"DISCREPANCY: ${result['discrepancy']:,.2f}\n"
            f"  Statement:  ${result['statement_balance']:,.2f}\n"
            f"  Calculated: ${result['calculated_balance']:,.2f}"
        )


if __name__ == "__main__":
    app()
