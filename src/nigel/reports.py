import sqlite3

from rich.console import Console
from rich.table import Table

console = Console()


def _date_filter(year: int | None = None, month: int | None = None,
                 from_date: str | None = None, to_date: str | None = None) -> tuple[str, list]:
    """Build a WHERE clause fragment for date filtering."""
    if from_date and to_date:
        return "t.date BETWEEN ? AND ?", [from_date, to_date]
    if year and month:
        prefix = f"{year}-{month:02d}"
        return "t.date LIKE ?", [f"{prefix}%"]
    if year:
        return "t.date LIKE ?", [f"{year}%"]
    # Default: current year
    from datetime import date
    return "t.date LIKE ?", [f"{date.today().year}%"]


def get_pnl(conn: sqlite3.Connection, year: int | None = None, month: int | None = None,
            from_date: str | None = None, to_date: str | None = None) -> dict:
    date_clause, params = _date_filter(year, month, from_date, to_date)

    income_rows = conn.execute(
        f"SELECT c.name, SUM(t.amount) as total "
        f"FROM transactions t JOIN categories c ON t.category_id = c.id "
        f"WHERE {date_clause} AND c.category_type = 'income' "
        f"GROUP BY c.name ORDER BY total DESC",
        params,
    ).fetchall()

    expense_rows = conn.execute(
        f"SELECT c.name, SUM(t.amount) as total "
        f"FROM transactions t JOIN categories c ON t.category_id = c.id "
        f"WHERE {date_clause} AND c.category_type = 'expense' "
        f"GROUP BY c.name ORDER BY total ASC",
        params,
    ).fetchall()

    total_income = sum(r["total"] for r in income_rows)
    total_expenses = sum(r["total"] for r in expense_rows)

    return {
        "income": [{"name": r["name"], "total": r["total"]} for r in income_rows],
        "expenses": [{"name": r["name"], "total": r["total"]} for r in expense_rows],
        "total_income": total_income,
        "total_expenses": total_expenses,
        "net": total_income + total_expenses,  # expenses are negative
    }


def get_expense_breakdown(conn: sqlite3.Connection, year: int | None = None, month: int | None = None,
                          from_date: str | None = None, to_date: str | None = None) -> dict:
    date_clause, params = _date_filter(year, month, from_date, to_date)

    rows = conn.execute(
        f"SELECT c.name, SUM(t.amount) as total, COUNT(*) as count "
        f"FROM transactions t JOIN categories c ON t.category_id = c.id "
        f"WHERE {date_clause} AND c.category_type = 'expense' "
        f"GROUP BY c.name ORDER BY total ASC",
        params,
    ).fetchall()

    total = sum(r["total"] for r in rows)

    categories = []
    for r in rows:
        pct = (r["total"] / total * 100) if total != 0 else 0
        categories.append({"name": r["name"], "total": r["total"], "count": r["count"], "pct": pct})

    # Top vendors
    vendors = conn.execute(
        f"SELECT t.vendor, SUM(t.amount) as total, COUNT(*) as count "
        f"FROM transactions t JOIN categories c ON t.category_id = c.id "
        f"WHERE {date_clause} AND c.category_type = 'expense' AND t.vendor IS NOT NULL "
        f"GROUP BY t.vendor ORDER BY total ASC LIMIT 10",
        params,
    ).fetchall()

    return {
        "categories": categories,
        "total": total,
        "top_vendors": [{"vendor": v["vendor"], "total": v["total"], "count": v["count"]} for v in vendors],
    }


def get_tax_summary(conn: sqlite3.Connection, year: int | None = None) -> dict:
    date_clause, params = _date_filter(year)

    rows = conn.execute(
        f"SELECT c.name, c.tax_line, c.category_type, SUM(t.amount) as total "
        f"FROM transactions t JOIN categories c ON t.category_id = c.id "
        f"WHERE {date_clause} "
        f"GROUP BY c.name, c.tax_line, c.category_type "
        f"ORDER BY c.category_type DESC, c.tax_line",
        params,
    ).fetchall()

    return {
        "line_items": [
            {"name": r["name"], "tax_line": r["tax_line"], "type": r["category_type"], "total": r["total"]}
            for r in rows
        ]
    }


def get_cashflow(conn: sqlite3.Connection, year: int | None = None) -> dict:
    date_clause, params = _date_filter(year)

    rows = conn.execute(
        f"SELECT substr(t.date, 1, 7) as month, "
        f"SUM(CASE WHEN t.amount > 0 THEN t.amount ELSE 0 END) as inflows, "
        f"SUM(CASE WHEN t.amount < 0 THEN t.amount ELSE 0 END) as outflows "
        f"FROM transactions t WHERE {date_clause} "
        f"GROUP BY substr(t.date, 1, 7) ORDER BY month",
        params,
    ).fetchall()

    months = []
    running = 0.0
    for r in rows:
        running += r["inflows"] + r["outflows"]
        months.append({
            "month": r["month"],
            "inflows": r["inflows"],
            "outflows": r["outflows"],
            "net": r["inflows"] + r["outflows"],
            "running_balance": running,
        })
    return {"months": months}


def get_flagged(conn: sqlite3.Connection) -> list:
    return conn.execute(
        "SELECT t.id, t.date, t.description, t.amount, a.name as account_name, t.flag_reason "
        "FROM transactions t JOIN accounts a ON t.account_id = a.id "
        "WHERE t.is_flagged = 1 ORDER BY t.date"
    ).fetchall()


def get_balance(conn: sqlite3.Connection) -> dict:
    accounts = conn.execute(
        "SELECT a.id, a.name, a.account_type, COALESCE(SUM(t.amount), 0) as balance "
        "FROM accounts a LEFT JOIN transactions t ON a.id = t.account_id "
        "GROUP BY a.id ORDER BY a.name"
    ).fetchall()

    total = sum(a["balance"] for a in accounts)

    # YTD net income
    from datetime import date as date_cls
    year = date_cls.today().year
    ytd = conn.execute(
        "SELECT COALESCE(SUM(amount), 0) as net FROM transactions WHERE date LIKE ?",
        (f"{year}%",),
    ).fetchone()

    return {
        "accounts": [{"name": a["name"], "type": a["account_type"], "balance": a["balance"]} for a in accounts],
        "total": total,
        "ytd_net_income": ytd["net"],
    }
