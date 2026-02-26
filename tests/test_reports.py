from bookkeeper.reports import get_pnl, get_expense_breakdown


def _seed_transactions(db):
    db.execute("INSERT INTO accounts (name, account_type) VALUES ('Test', 'checking')")
    income_cat = db.execute("SELECT id FROM categories WHERE name = 'Client Services'").fetchone()["id"]
    software_cat = db.execute("SELECT id FROM categories WHERE name = 'Software & Subscriptions'").fetchone()["id"]
    meals_cat = db.execute("SELECT id FROM categories WHERE name = 'Meals'").fetchone()["id"]

    db.execute(
        "INSERT INTO transactions (account_id, date, description, amount, category_id, vendor) "
        "VALUES (1, '2025-03-01', 'Client payment', 5000.00, ?, 'Client A')",
        (income_cat,),
    )
    db.execute(
        "INSERT INTO transactions (account_id, date, description, amount, category_id, vendor) "
        "VALUES (1, '2025-03-10', 'Adobe', -54.43, ?, 'Adobe')",
        (software_cat,),
    )
    db.execute(
        "INSERT INTO transactions (account_id, date, description, amount, category_id, vendor) "
        "VALUES (1, '2025-03-15', 'Lunch meeting', -45.00, ?, 'Restaurant')",
        (meals_cat,),
    )
    db.execute(
        "INSERT INTO transactions (account_id, date, description, amount, category_id, vendor) "
        "VALUES (1, '2025-04-01', 'Client payment', 6000.00, ?, 'Client B')",
        (income_cat,),
    )
    db.commit()


def test_pnl_ytd(db):
    _seed_transactions(db)
    pnl = get_pnl(db, year=2025)
    assert pnl["total_income"] == 11000.00
    assert pnl["total_expenses"] == -99.43
    assert pnl["net"] == 11000.00 - 99.43


def test_pnl_by_month(db):
    _seed_transactions(db)
    pnl = get_pnl(db, year=2025, month=3)
    assert pnl["total_income"] == 5000.00
    assert pnl["total_expenses"] == -99.43


def test_expense_breakdown(db):
    _seed_transactions(db)
    breakdown = get_expense_breakdown(db, year=2025)
    # Should have 2 expense categories
    assert len(breakdown["categories"]) == 2
    # Software should be in there
    names = [c["name"] for c in breakdown["categories"]]
    assert "Software & Subscriptions" in names
    assert "Meals" in names
