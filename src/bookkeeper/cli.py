import os
from pathlib import Path

import typer

from bookkeeper.db import get_connection, init_db

app = typer.Typer(help="Raygun Bookkeeper — cash-basis bookkeeping CLI.", invoke_without_command=True)


@app.callback()
def main():
    """Raygun Bookkeeper — cash-basis bookkeeping CLI."""

DEFAULT_DATA_DIR = Path.home() / "Documents" / "bookkeeper"


def get_data_dir() -> Path:
    return Path(os.environ.get("BOOKKEEPER_DATA_DIR", str(DEFAULT_DATA_DIR)))


def get_db_path() -> Path:
    return get_data_dir() / "raygun.db"


@app.command()
def init():
    """Initialize the bookkeeper database and seed categories."""
    data_dir = get_data_dir()
    data_dir.mkdir(parents=True, exist_ok=True)
    (data_dir / "imports").mkdir(exist_ok=True)
    (data_dir / "exports").mkdir(exist_ok=True)

    conn = get_connection(data_dir / "raygun.db")
    init_db(conn)
    conn.close()

    typer.echo(f"Initialized bookkeeper at {data_dir}")


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


if __name__ == "__main__":
    app()
