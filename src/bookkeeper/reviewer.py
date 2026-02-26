import sqlite3

from rich.console import Console
from rich.prompt import Confirm, Prompt
from rich.rule import Rule
from rich.table import Table

console = Console()


def get_flagged_transactions(conn: sqlite3.Connection) -> list:
    return conn.execute(
        "SELECT t.id, t.date, t.description, t.amount, a.name as account_name "
        "FROM transactions t JOIN accounts a ON t.account_id = a.id "
        "WHERE t.is_flagged = 1 ORDER BY t.date"
    ).fetchall()


def get_categories(conn: sqlite3.Connection) -> list:
    return conn.execute(
        "SELECT id, name, category_type FROM categories WHERE is_active = 1 ORDER BY category_type, name"
    ).fetchall()


def apply_review(
    conn: sqlite3.Connection,
    transaction_id: int,
    category_id: int,
    vendor: str | None = None,
    create_rule: bool = False,
    rule_pattern: str | None = None,
) -> None:
    """Apply a review decision to a transaction."""
    conn.execute(
        "UPDATE transactions SET category_id = ?, vendor = ?, is_flagged = 0, flag_reason = NULL WHERE id = ?",
        (category_id, vendor, transaction_id),
    )
    if create_rule and rule_pattern:
        conn.execute(
            "INSERT INTO rules (pattern, match_type, vendor, category_id) VALUES (?, 'contains', ?, ?)",
            (rule_pattern, vendor, category_id),
        )
    conn.commit()


def run_review(conn: sqlite3.Connection) -> None:
    """Interactive review loop for flagged transactions."""
    flagged = get_flagged_transactions(conn)
    if not flagged:
        console.print("[green]No flagged transactions to review.[/green]")
        return

    categories = get_categories(conn)
    console.print(f"\n[bold]{len(flagged)} transactions to review[/bold]\n")

    # Print category list for reference
    cat_table = Table(title="Categories", show_lines=False)
    cat_table.add_column("#", style="dim")
    cat_table.add_column("Name")
    cat_table.add_column("Type", style="dim")
    for i, cat in enumerate(categories, 1):
        cat_table.add_row(str(i), cat["name"], cat["category_type"])
    console.print(cat_table)
    console.print()

    for txn in flagged:
        console.print(Rule())
        console.print(f"  [bold]Date:[/bold]        {txn['date']}")
        console.print(f"  [bold]Description:[/bold] {txn['description']}")
        amount = txn["amount"]
        color = "red" if amount < 0 else "green"
        console.print(f"  [bold]Amount:[/bold]      [{color}]${abs(amount):,.2f}[/{color}]")
        console.print(f"  [bold]Account:[/bold]     {txn['account_name']}")
        console.print()

        choice = Prompt.ask(
            "Category # (or [bold]s[/bold]kip, [bold]q[/bold]uit)",
        )

        if choice.lower() == "q":
            console.print("[yellow]Review paused.[/yellow]")
            return
        if choice.lower() == "s":
            continue

        try:
            idx = int(choice) - 1
            cat = categories[idx]
        except (ValueError, IndexError):
            console.print("[red]Invalid choice, skipping.[/red]")
            continue

        vendor = Prompt.ask("Vendor name (or Enter to skip)", default="")
        vendor = vendor if vendor else None

        create_rule = Confirm.ask("Create rule for future matches?", default=False)
        rule_pattern = None
        if create_rule:
            # Suggest first two words of description as pattern
            words = txn["description"].split()
            suggested = " ".join(words[:2]) if len(words) >= 2 else words[0]
            rule_pattern = Prompt.ask("Rule pattern", default=suggested)

        apply_review(
            conn,
            transaction_id=txn["id"],
            category_id=cat["id"],
            vendor=vendor,
            create_rule=create_rule,
            rule_pattern=rule_pattern,
        )
        console.print(f"[green]→ Categorized as {cat['name']}[/green]\n")

    console.print("[green]Review complete![/green]")
