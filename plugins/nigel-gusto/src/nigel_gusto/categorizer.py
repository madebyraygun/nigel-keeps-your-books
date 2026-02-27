import sqlite3

from nigel.models import ParsedRow


def auto_categorize_payroll(
    conn: sqlite3.Connection, account_id: int, rows: list[ParsedRow]
) -> None:
    """Post-import hook: auto-categorize Gusto payroll transactions."""
    payroll_categories = {}
    for cat_name in ("Payroll — Wages", "Payroll — Taxes", "Payroll — Benefits"):
        row = conn.execute("SELECT id FROM categories WHERE name = ?", (cat_name,)).fetchone()
        if row:
            payroll_categories[cat_name] = row["id"]

    for parsed_row in rows:
        category_id = None
        if "Wages" in parsed_row.description:
            category_id = payroll_categories.get("Payroll — Wages")
        elif "Taxes" in parsed_row.description:
            category_id = payroll_categories.get("Payroll — Taxes")
        elif "Benefits" in parsed_row.description:
            category_id = payroll_categories.get("Payroll — Benefits")

        if category_id:
            conn.execute(
                "UPDATE transactions SET category_id = ?, is_flagged = 0, flag_reason = NULL "
                "WHERE account_id = ? AND date = ? AND amount = ? AND description = ?",
                (category_id, account_id, parsed_row.date, parsed_row.amount, parsed_row.description),
            )
    conn.commit()
