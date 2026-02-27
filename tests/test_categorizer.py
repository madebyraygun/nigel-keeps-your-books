from nigel.categorizer import categorize_transactions


def test_categorize_by_contains_rule(db):
    db.execute("INSERT INTO accounts (name, account_type) VALUES ('Test', 'checking')")
    cat_id = db.execute(
        "SELECT id FROM categories WHERE name = 'Software & Subscriptions'"
    ).fetchone()["id"]
    db.execute(
        "INSERT INTO rules (pattern, match_type, vendor, category_id, priority) VALUES (?, ?, ?, ?, ?)",
        ("ADOBE", "contains", "Adobe", cat_id, 10),
    )
    db.execute(
        "INSERT INTO transactions (account_id, date, description, amount, is_flagged, flag_reason) "
        "VALUES (1, '2025-03-10', 'ADOBE INC SUBSCRIPTION', -54.43, 1, 'No matching rule')",
    )
    db.commit()

    result = categorize_transactions(db)
    assert result["categorized"] == 1
    assert result["still_flagged"] == 0

    txn = db.execute("SELECT * FROM transactions WHERE id = 1").fetchone()
    assert txn["category_id"] == cat_id
    assert txn["vendor"] == "Adobe"
    assert txn["is_flagged"] == 0


def test_categorize_by_starts_with_rule(db):
    db.execute("INSERT INTO accounts (name, account_type) VALUES ('Test', 'checking')")
    cat_id = db.execute(
        "SELECT id FROM categories WHERE name = 'Travel'"
    ).fetchone()["id"]
    db.execute(
        "INSERT INTO rules (pattern, match_type, vendor, category_id, priority) VALUES (?, ?, ?, ?, ?)",
        ("UNITED AIR", "starts_with", "United Airlines", cat_id, 10),
    )
    db.execute(
        "INSERT INTO transactions (account_id, date, description, amount, is_flagged, flag_reason) "
        "VALUES (1, '2025-03-10', 'UNITED AIRLINES BOOKING', -350.00, 1, 'No matching rule')",
    )
    db.commit()

    result = categorize_transactions(db)
    assert result["categorized"] == 1


def test_categorize_by_regex_rule(db):
    db.execute("INSERT INTO accounts (name, account_type) VALUES ('Test', 'checking')")
    cat_id = db.execute(
        "SELECT id FROM categories WHERE name = 'Bank & Merchant Fees'"
    ).fetchone()["id"]
    db.execute(
        "INSERT INTO rules (pattern, match_type, vendor, category_id, priority) VALUES (?, ?, ?, ?, ?)",
        (r"STRIPE.*FEE", "regex", "Stripe", cat_id, 10),
    )
    db.execute(
        "INSERT INTO transactions (account_id, date, description, amount, is_flagged, flag_reason) "
        "VALUES (1, '2025-03-10', 'STRIPE PROCESSING FEE', -12.50, 1, 'No matching rule')",
    )
    db.commit()

    result = categorize_transactions(db)
    assert result["categorized"] == 1


def test_higher_priority_rule_wins(db):
    db.execute("INSERT INTO accounts (name, account_type) VALUES ('Test', 'checking')")
    cat_low = db.execute("SELECT id FROM categories WHERE name = 'Office Expense'").fetchone()["id"]
    cat_high = db.execute("SELECT id FROM categories WHERE name = 'Software & Subscriptions'").fetchone()["id"]
    db.execute(
        "INSERT INTO rules (pattern, match_type, vendor, category_id, priority) VALUES (?, ?, ?, ?, ?)",
        ("ADOBE", "contains", "Adobe Office", cat_low, 1),
    )
    db.execute(
        "INSERT INTO rules (pattern, match_type, vendor, category_id, priority) VALUES (?, ?, ?, ?, ?)",
        ("ADOBE", "contains", "Adobe Software", cat_high, 10),
    )
    db.execute(
        "INSERT INTO transactions (account_id, date, description, amount, is_flagged, flag_reason) "
        "VALUES (1, '2025-03-10', 'ADOBE CREATIVE CLOUD', -54.43, 1, 'No matching rule')",
    )
    db.commit()

    categorize_transactions(db)
    txn = db.execute("SELECT * FROM transactions WHERE id = 1").fetchone()
    assert txn["category_id"] == cat_high
    assert txn["vendor"] == "Adobe Software"


def test_unmatched_transactions_stay_flagged(db):
    db.execute("INSERT INTO accounts (name, account_type) VALUES ('Test', 'checking')")
    db.execute(
        "INSERT INTO transactions (account_id, date, description, amount, is_flagged, flag_reason) "
        "VALUES (1, '2025-03-10', 'MYSTERY VENDOR XYZ', -100.00, 1, 'No matching rule')",
    )
    db.commit()

    result = categorize_transactions(db)
    assert result["categorized"] == 0
    assert result["still_flagged"] == 1


def test_categorize_increments_hit_count(db):
    db.execute("INSERT INTO accounts (name, account_type) VALUES ('Test', 'checking')")
    cat_id = db.execute("SELECT id FROM categories WHERE name = 'Software & Subscriptions'").fetchone()["id"]
    db.execute(
        "INSERT INTO rules (pattern, match_type, vendor, category_id, priority) VALUES (?, ?, ?, ?, ?)",
        ("ADOBE", "contains", "Adobe", cat_id, 10),
    )
    db.execute(
        "INSERT INTO transactions (account_id, date, description, amount, is_flagged, flag_reason) "
        "VALUES (1, '2025-03-10', 'ADOBE INC', -54.43, 1, 'No matching rule')",
    )
    db.execute(
        "INSERT INTO transactions (account_id, date, description, amount, is_flagged, flag_reason) "
        "VALUES (1, '2025-04-10', 'ADOBE INC', -54.43, 1, 'No matching rule')",
    )
    db.commit()

    categorize_transactions(db)
    rule = db.execute("SELECT hit_count FROM rules WHERE id = 1").fetchone()
    assert rule["hit_count"] == 2
