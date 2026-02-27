import typer

from nigel.plugins import PluginHooks
from nigel.models import ImporterInfo


def _dummy_parser(path):
    return []


def test_add_importer():
    hooks = PluginHooks()
    info = ImporterInfo(
        key="test", name="Test", account_types=["test"],
        file_extensions=[".csv"], parse=_dummy_parser,
    )
    hooks.add_importer(info)
    assert len(hooks.importers) == 1


def test_add_command():
    hooks = PluginHooks()
    parent = typer.Typer()

    @parent.command()
    def existing():
        pass

    def new_cmd():
        """A plugin command."""
        pass

    hooks.add_command(parent, new_cmd)
    assert len(hooks.commands) == 1


def test_add_migration():
    hooks = PluginHooks()

    def migrate(conn):
        conn.execute("CREATE TABLE IF NOT EXISTS test_table (id INTEGER PRIMARY KEY)")

    hooks.add_migration(migrate)
    assert len(hooks.migrations) == 1


def test_add_categories():
    hooks = PluginHooks()
    hooks.add_categories([
        {"name": "Test Cat", "category_type": "expense", "tax_line": "Line 99"},
    ])
    assert len(hooks.categories) == 1


def test_apply_migrations(tmp_path):
    from nigel.db import get_connection, init_db
    from nigel.plugins import apply_migrations

    conn = get_connection(tmp_path / "test.db")
    init_db(conn)

    hooks = PluginHooks()
    hooks.add_migration(
        lambda c: c.execute("CREATE TABLE IF NOT EXISTS plugin_test (id INTEGER PRIMARY KEY)")
    )
    apply_migrations(conn, hooks)

    tables = [r[0] for r in conn.execute(
        "SELECT name FROM sqlite_master WHERE type='table'"
    ).fetchall()]
    assert "plugin_test" in tables
    conn.close()


def test_seed_plugin_categories(tmp_path):
    from nigel.db import get_connection, init_db
    from nigel.plugins import seed_plugin_categories

    conn = get_connection(tmp_path / "test.db")
    init_db(conn)

    hooks = PluginHooks()
    hooks.add_categories([
        {"name": "Charitable Contributions", "category_type": "expense", "tax_line": "K Line 12a"},
    ])
    seed_plugin_categories(conn, hooks)

    row = conn.execute("SELECT * FROM categories WHERE name = 'Charitable Contributions'").fetchone()
    assert row is not None
    assert row["category_type"] == "expense"
    conn.close()


def test_seed_plugin_categories_idempotent(tmp_path):
    from nigel.db import get_connection, init_db
    from nigel.plugins import seed_plugin_categories

    conn = get_connection(tmp_path / "test.db")
    init_db(conn)

    hooks = PluginHooks()
    hooks.add_categories([
        {"name": "Charitable Contributions", "category_type": "expense", "tax_line": "K Line 12a"},
    ])
    seed_plugin_categories(conn, hooks)
    seed_plugin_categories(conn, hooks)

    count = conn.execute(
        "SELECT count(*) FROM categories WHERE name = 'Charitable Contributions'"
    ).fetchone()[0]
    assert count == 1
    conn.close()
