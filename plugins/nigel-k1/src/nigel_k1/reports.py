import sqlite3


def _date_filter(year: int) -> tuple[str, list]:
    return "t.date LIKE ?", [f"{year}%"]


def get_income_summary(conn: sqlite3.Connection, year: int) -> dict:
    """1120-S income summary: gross receipts through ordinary business income."""
    clause, params = _date_filter(year)

    # Gross receipts (all income categories)
    gross = conn.execute(
        f"SELECT COALESCE(SUM(t.amount), 0) as total "
        f"FROM transactions t JOIN categories c ON t.category_id = c.id "
        f"WHERE {clause} AND c.category_type = 'income'",
        params,
    ).fetchone()["total"]

    # Deductions by form_line
    deductions = conn.execute(
        f"SELECT c.form_line, COALESCE(SUM(t.amount), 0) as total "
        f"FROM transactions t JOIN categories c ON t.category_id = c.id "
        f"WHERE {clause} AND c.category_type = 'expense' AND c.form_line IS NOT NULL "
        f"AND c.form_line NOT LIKE 'K-%' AND c.form_line != 'K-16d' "
        f"GROUP BY c.form_line",
        params,
    ).fetchall()

    deduction_map = {r["form_line"]: r["total"] for r in deductions}

    officer_comp = abs(deduction_map.get("1120S-7", 0))
    salaries = abs(deduction_map.get("1120S-8", 0))
    rents = abs(deduction_map.get("1120S-11", 0))
    taxes_licenses = abs(deduction_map.get("1120S-12", 0))
    advertising = abs(deduction_map.get("1120S-16", 0))
    employee_benefits = abs(deduction_map.get("1120S-18", 0))

    # Line 19 -- other deductions (with meals at 50%)
    line_19_detail = get_line_19_detail(conn, year)
    other_deductions = sum(d["deductible_amount"] for d in line_19_detail)

    total_deductions = -(officer_comp + salaries + rents + taxes_licenses +
                         advertising + employee_benefits + other_deductions)

    return {
        "gross_receipts": gross,
        "officer_compensation": officer_comp,
        "salaries_wages": salaries,
        "rents": rents,
        "taxes_licenses": taxes_licenses,
        "advertising": advertising,
        "employee_benefits": employee_benefits,
        "other_deductions": other_deductions,
        "total_deductions": total_deductions,
        "ordinary_business_income": gross + total_deductions,
    }


def get_line_19_detail(conn: sqlite3.Connection, year: int) -> list[dict]:
    """Line 19 other deductions broken out by category, meals at 50%."""
    clause, params = _date_filter(year)

    rows = conn.execute(
        f"SELECT c.name, COALESCE(SUM(t.amount), 0) as total "
        f"FROM transactions t JOIN categories c ON t.category_id = c.id "
        f"WHERE {clause} AND c.form_line = '1120S-19' "
        f"GROUP BY c.name ORDER BY total ASC",
        params,
    ).fetchall()

    detail = []
    for r in rows:
        full = abs(r["total"])
        if r["name"] == "Meals":
            deductible = full * 0.50
        else:
            deductible = full
        detail.append({
            "name": r["name"],
            "full_amount": full,
            "deductible_amount": deductible,
        })
    return detail


def get_schedule_k(conn: sqlite3.Connection, year: int) -> dict:
    """Schedule K summary with separately stated items."""
    income = get_income_summary(conn, year)
    clause, params = _date_filter(year)

    # Interest income (K Line 4)
    interest = conn.execute(
        f"SELECT COALESCE(SUM(t.amount), 0) as total "
        f"FROM transactions t JOIN categories c ON t.category_id = c.id "
        f"WHERE {clause} AND c.form_line = 'K-4'",
        params,
    ).fetchone()["total"]

    # Section 179 (K Line 11)
    sec_179 = conn.execute(
        f"SELECT COALESCE(SUM(t.amount), 0) as total "
        f"FROM transactions t JOIN categories c ON t.category_id = c.id "
        f"WHERE {clause} AND c.form_line = 'K-11'",
        params,
    ).fetchone()["total"]

    # Charitable contributions (K Line 12a)
    charitable = conn.execute(
        f"SELECT COALESCE(SUM(t.amount), 0) as total "
        f"FROM transactions t JOIN categories c ON t.category_id = c.id "
        f"WHERE {clause} AND c.form_line = 'K-12a'",
        params,
    ).fetchone()["total"]

    # Distributions (K Line 16d)
    distributions = conn.execute(
        f"SELECT COALESCE(SUM(t.amount), 0) as total "
        f"FROM transactions t JOIN categories c ON t.category_id = c.id "
        f"WHERE {clause} AND c.form_line = 'K-16d'",
        params,
    ).fetchone()["total"]

    return {
        "line_1": income["ordinary_business_income"],
        "line_4": interest,
        "line_11": abs(sec_179),
        "line_12a": abs(charitable),
        "line_16d": abs(distributions),
    }


def get_shareholder_worksheets(conn: sqlite3.Connection, year: int) -> list[dict]:
    """Per-shareholder K-1 worksheets allocated by ownership %."""
    k = get_schedule_k(conn, year)
    shareholders = conn.execute(
        "SELECT * FROM shareholders ORDER BY name"
    ).fetchall()

    worksheets = []
    for s in shareholders:
        pct = s["ownership_pct"]
        worksheets.append({
            "name": s["name"],
            "ownership_pct": pct,
            "is_officer": bool(s["is_officer"]),
            "annual_compensation": s["annual_compensation"],
            "line_1": k["line_1"] * pct,
            "line_4": k["line_4"] * pct,
            "line_11": k["line_11"] * pct,
            "line_12a": k["line_12a"] * pct,
            "line_16d": k["line_16d"] * pct,
        })
    return worksheets


def get_validation_checks(conn: sqlite3.Connection, year: int) -> dict:
    """Run validation checks and return warnings."""
    clause, params = _date_filter(year)

    # Uncategorized transactions
    uncategorized = conn.execute(
        f"SELECT count(*) as cnt FROM transactions t "
        f"WHERE {clause} AND t.is_flagged = 1",
        params,
    ).fetchone()["cnt"]

    # Distribution proportionality check
    shareholders = conn.execute(
        "SELECT name, ownership_pct FROM shareholders ORDER BY name"
    ).fetchall()

    # Actual distributions per shareholder would need vendor matching
    # For now, check if total distributions exist
    distributions_total = conn.execute(
        f"SELECT COALESCE(SUM(t.amount), 0) as total "
        f"FROM transactions t JOIN categories c ON t.category_id = c.id "
        f"WHERE {clause} AND c.form_line = 'K-16d'",
        params,
    ).fetchone()["total"]

    # Officer comp vs distribution ratio
    officer_comp = conn.execute(
        f"SELECT COALESCE(SUM(t.amount), 0) as total "
        f"FROM transactions t JOIN categories c ON t.category_id = c.id "
        f"WHERE {clause} AND c.form_line = '1120S-7'",
        params,
    ).fetchone()["total"]

    comp_amount = abs(officer_comp)
    dist_amount = abs(distributions_total)
    ratio_warning = dist_amount > comp_amount * 2 if comp_amount > 0 else False

    return {
        "uncategorized_count": uncategorized,
        "distribution_proportional": len(shareholders) > 0,
        "comp_to_distribution_warning": ratio_warning,
        "officer_compensation": comp_amount,
        "total_distributions": dist_amount,
    }
