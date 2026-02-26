from bookkeeper.db import init_db, get_connection


def test_init_db_creates_tables(tmp_path):
    db_path = tmp_path / "test.db"
    conn = get_connection(db_path)
    init_db(conn)

    cursor = conn.execute(
        "SELECT name FROM sqlite_master WHERE type='table' ORDER BY name"
    )
    tables = [row[0] for row in cursor.fetchall()]
    assert "accounts" in tables
    assert "categories" in tables
    assert "transactions" in tables
    assert "rules" in tables
    assert "imports" in tables
    assert "reconciliations" in tables


def test_init_db_is_idempotent(tmp_path):
    db_path = tmp_path / "test.db"
    conn = get_connection(db_path)
    init_db(conn)
    init_db(conn)  # Should not raise

    cursor = conn.execute(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='accounts'"
    )
    assert cursor.fetchone()[0] == 1


def test_init_db_seeds_categories(tmp_path):
    db_path = tmp_path / "test.db"
    conn = get_connection(db_path)
    init_db(conn)

    cursor = conn.execute("SELECT count(*) FROM categories")
    count = cursor.fetchone()[0]
    assert count >= 25  # At least 25 default categories from the taxonomy


def test_init_db_seeds_income_and_expense_categories(tmp_path):
    db_path = tmp_path / "test.db"
    conn = get_connection(db_path)
    init_db(conn)

    cursor = conn.execute(
        "SELECT count(*) FROM categories WHERE category_type = 'income'"
    )
    income_count = cursor.fetchone()[0]
    assert income_count >= 5

    cursor = conn.execute(
        "SELECT count(*) FROM categories WHERE category_type = 'expense'"
    )
    expense_count = cursor.fetchone()[0]
    assert expense_count >= 20
