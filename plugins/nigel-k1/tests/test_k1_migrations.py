from nigel_k1.migrations import create_k1_tables
from nigel_k1.categories import K1_CATEGORIES
from nigel.plugins import PluginHooks, seed_plugin_categories


def test_create_k1_tables(db):
    create_k1_tables(db)
    tables = [r[0] for r in db.execute(
        "SELECT name FROM sqlite_master WHERE type='table'"
    ).fetchall()]
    assert "entity_config" in tables
    assert "shareholders" in tables


def test_create_k1_tables_idempotent(db):
    create_k1_tables(db)
    create_k1_tables(db)  # Should not raise


def test_k1_categories_seeded(db):
    hooks = PluginHooks()
    hooks.add_categories(K1_CATEGORIES)
    seed_plugin_categories(db, hooks)

    row = db.execute(
        "SELECT * FROM categories WHERE name = 'Officer Compensation'"
    ).fetchone()
    assert row is not None
    assert row["form_line"] == "1120S-7"
