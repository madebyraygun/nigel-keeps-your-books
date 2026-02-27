from nigel.reconciler import reconcile


def test_reconcile_matching_balance(db):
    db.execute("INSERT INTO accounts (name, account_type) VALUES ('Test', 'checking')")
    db.execute(
        "INSERT INTO transactions (account_id, date, description, amount) "
        "VALUES (1, '2025-03-01', 'Deposit', 5000.00)",
    )
    db.execute(
        "INSERT INTO transactions (account_id, date, description, amount) "
        "VALUES (1, '2025-03-15', 'Payment', -2000.00)",
    )
    db.commit()

    result = reconcile(db, account_name="Test", month="2025-03", statement_balance=3000.00)
    assert result["is_reconciled"] is True
    assert result["discrepancy"] == 0.0


def test_reconcile_with_discrepancy(db):
    db.execute("INSERT INTO accounts (name, account_type) VALUES ('Test', 'checking')")
    db.execute(
        "INSERT INTO transactions (account_id, date, description, amount) "
        "VALUES (1, '2025-03-01', 'Deposit', 5000.00)",
    )
    db.commit()

    result = reconcile(db, account_name="Test", month="2025-03", statement_balance=4500.00)
    assert result["is_reconciled"] is False
    assert result["discrepancy"] == 500.00


def test_reconcile_stores_record(db):
    db.execute("INSERT INTO accounts (name, account_type) VALUES ('Test', 'checking')")
    db.execute(
        "INSERT INTO transactions (account_id, date, description, amount) "
        "VALUES (1, '2025-03-01', 'Deposit', 1000.00)",
    )
    db.commit()

    reconcile(db, account_name="Test", month="2025-03", statement_balance=1000.00)

    rec = db.execute("SELECT * FROM reconciliations WHERE month = '2025-03'").fetchone()
    assert rec is not None
    assert rec["is_reconciled"] == 1
