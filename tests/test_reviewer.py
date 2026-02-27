from nigel.reviewer import get_flagged_transactions, apply_review


def test_get_flagged_transactions(db):
    db.execute("INSERT INTO accounts (name, account_type) VALUES ('Test', 'checking')")
    db.execute(
        "INSERT INTO transactions (account_id, date, description, amount, is_flagged, flag_reason) "
        "VALUES (1, '2025-03-10', 'MYSTERY VENDOR', -100.00, 1, 'No matching rule')",
    )
    db.commit()

    flagged = get_flagged_transactions(db)
    assert len(flagged) == 1
    assert flagged[0]["description"] == "MYSTERY VENDOR"


def test_apply_review_categorizes_transaction(db):
    db.execute("INSERT INTO accounts (name, account_type) VALUES ('Test', 'checking')")
    cat_id = db.execute("SELECT id FROM categories WHERE name = 'Office Expense'").fetchone()["id"]
    db.execute(
        "INSERT INTO transactions (account_id, date, description, amount, is_flagged, flag_reason) "
        "VALUES (1, '2025-03-10', 'STAPLES STORE', -45.00, 1, 'No matching rule')",
    )
    db.commit()

    apply_review(db, transaction_id=1, category_id=cat_id, vendor="Staples")

    txn = db.execute("SELECT * FROM transactions WHERE id = 1").fetchone()
    assert txn["category_id"] == cat_id
    assert txn["vendor"] == "Staples"
    assert txn["is_flagged"] == 0


def test_apply_review_with_rule_creation(db):
    db.execute("INSERT INTO accounts (name, account_type) VALUES ('Test', 'checking')")
    cat_id = db.execute("SELECT id FROM categories WHERE name = 'Office Expense'").fetchone()["id"]
    db.execute(
        "INSERT INTO transactions (account_id, date, description, amount, is_flagged, flag_reason) "
        "VALUES (1, '2025-03-10', 'STAPLES STORE 1234', -45.00, 1, 'No matching rule')",
    )
    db.commit()

    apply_review(db, transaction_id=1, category_id=cat_id, vendor="Staples",
                 create_rule=True, rule_pattern="STAPLES")

    rule = db.execute("SELECT * FROM rules WHERE pattern = 'STAPLES'").fetchone()
    assert rule is not None
    assert rule["category_id"] == cat_id
    assert rule["vendor"] == "Staples"
