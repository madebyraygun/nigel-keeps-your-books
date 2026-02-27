from nigel_k1.migrations import create_k1_tables
from nigel_k1.categories import K1_CATEGORIES
from nigel.plugins import PluginHooks, seed_plugin_categories


def _setup_k1(db):
    """Set up K-1 tables, categories, entity config, and shareholders."""
    create_k1_tables(db)
    hooks = PluginHooks()
    hooks.add_categories(K1_CATEGORIES)
    seed_plugin_categories(db, hooks)

    # Entity config
    db.executemany("INSERT INTO entity_config (key, value) VALUES (?, ?)", [
        ("entity_name", "Test LLC"),
        ("entity_type", "s_corp"),
        ("ein", "12-3456789"),
        ("tax_year", "2025"),
    ])

    # Shareholders
    db.execute(
        "INSERT INTO shareholders (name, ownership_pct, is_officer, annual_compensation) "
        "VALUES (?, ?, ?, ?)",
        ("Alice", 0.50, 1, 60000),
    )
    db.execute(
        "INSERT INTO shareholders (name, ownership_pct, is_officer, annual_compensation) "
        "VALUES (?, ?, ?, ?)",
        ("Bob", 0.50, 1, 60000),
    )
    db.commit()


def _seed_transactions(db):
    """Add sample transactions for a tax year."""
    db.execute("INSERT INTO accounts (name, account_type) VALUES ('Checking', 'checking')")

    income_cat = db.execute("SELECT id FROM categories WHERE name = 'Client Services'").fetchone()["id"]
    software_cat = db.execute("SELECT id FROM categories WHERE name = 'Software & Subscriptions'").fetchone()["id"]
    meals_cat = db.execute("SELECT id FROM categories WHERE name = 'Meals'").fetchone()["id"]
    officer_cat = db.execute("SELECT id FROM categories WHERE name = 'Officer Compensation'").fetchone()["id"]
    payroll_tax_cat = db.execute("SELECT id FROM categories WHERE name = 'Payroll — Taxes'").fetchone()["id"]
    distribution_cat = db.execute("SELECT id FROM categories WHERE name = 'Owner Draw / Distribution'").fetchone()["id"]

    txns = [
        (1, "2025-03-01", "Client payment", 50000.00, income_cat, "Client A"),
        (1, "2025-06-01", "Client payment", 50000.00, income_cat, "Client B"),
        (1, "2025-03-10", "Adobe", -600.00, software_cat, "Adobe"),
        (1, "2025-03-15", "Lunch meeting", -200.00, meals_cat, "Restaurant"),
        (1, "2025-01-31", "Officer wages", -10000.00, officer_cat, "Payroll"),
        (1, "2025-02-28", "Officer wages", -10000.00, officer_cat, "Payroll"),
        (1, "2025-03-31", "Employer taxes", -2000.00, payroll_tax_cat, "Gusto"),
        (1, "2025-06-30", "Distribution", -15000.00, distribution_cat, "Owner Draw"),
    ]
    for acct, date, desc, amt, cat, vendor in txns:
        db.execute(
            "INSERT INTO transactions (account_id, date, description, amount, category_id, vendor, is_flagged) "
            "VALUES (?, ?, ?, ?, ?, ?, 0)",
            (acct, date, desc, amt, cat, vendor),
        )
    db.commit()


def test_get_income_summary(db):
    _setup_k1(db)
    _seed_transactions(db)
    from nigel_k1.reports import get_income_summary
    result = get_income_summary(db, year=2025)

    assert result["gross_receipts"] == 100000.00
    assert result["officer_compensation"] == 20000.00
    assert result["total_deductions"] < 0
    assert "ordinary_business_income" in result


def test_get_schedule_k(db):
    _setup_k1(db)
    _seed_transactions(db)
    from nigel_k1.reports import get_schedule_k
    result = get_schedule_k(db, year=2025)

    assert "line_1" in result  # Ordinary business income
    assert "line_16d" in result  # Distributions
    assert result["line_16d"] == 15000.00


def test_get_shareholder_worksheets(db):
    _setup_k1(db)
    _seed_transactions(db)
    from nigel_k1.reports import get_shareholder_worksheets
    worksheets = get_shareholder_worksheets(db, year=2025)

    assert len(worksheets) == 2
    assert worksheets[0]["name"] == "Alice"
    assert worksheets[0]["ownership_pct"] == 0.50
    # Each shareholder gets 50% of ordinary income
    assert worksheets[0]["line_1"] == worksheets[1]["line_1"]


def test_meals_50_pct_limitation(db):
    _setup_k1(db)
    _seed_transactions(db)
    from nigel_k1.reports import get_line_19_detail
    detail = get_line_19_detail(db, year=2025)

    meals_item = next(d for d in detail if d["name"] == "Meals")
    # Full amount is 200, deductible is 50% = 100
    assert meals_item["full_amount"] == 200.00
    assert meals_item["deductible_amount"] == 100.00


def test_validation_checks(db):
    _setup_k1(db)
    _seed_transactions(db)

    # Add a flagged transaction
    db.execute(
        "INSERT INTO transactions (account_id, date, description, amount, is_flagged, flag_reason) "
        "VALUES (1, '2025-04-01', 'Mystery', -50.00, 1, 'No matching rule')"
    )
    db.commit()

    from nigel_k1.reports import get_validation_checks
    checks = get_validation_checks(db, year=2025)

    assert checks["uncategorized_count"] > 0
    assert "distribution_proportional" in checks
