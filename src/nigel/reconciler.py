import sqlite3


def reconcile(conn: sqlite3.Connection, account_name: str, month: str, statement_balance: float) -> dict:
    """Reconcile an account for a given month against a statement balance."""
    account = conn.execute("SELECT id FROM accounts WHERE name = ?", (account_name,)).fetchone()
    if account is None:
        raise ValueError(f"Unknown account: {account_name}")

    account_id = account["id"]

    # Sum all transactions for this account up to end of month
    cursor = conn.execute(
        "SELECT COALESCE(SUM(amount), 0) as total FROM transactions "
        "WHERE account_id = ? AND date <= ? || '-31'",
        (account_id, month),
    )
    calculated = cursor.fetchone()["total"]
    discrepancy = abs(calculated - statement_balance)
    is_reconciled = discrepancy < 0.01  # float tolerance

    conn.execute(
        "INSERT INTO reconciliations (account_id, month, statement_balance, calculated_balance, is_reconciled, reconciled_at) "
        "VALUES (?, ?, ?, ?, ?, CASE WHEN ? THEN datetime('now') ELSE NULL END)",
        (account_id, month, statement_balance, calculated, 1 if is_reconciled else 0, is_reconciled),
    )
    conn.commit()

    return {
        "is_reconciled": is_reconciled,
        "statement_balance": statement_balance,
        "calculated_balance": calculated,
        "discrepancy": round(discrepancy, 2),
    }
