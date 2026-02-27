import importlib.metadata
import sqlite3

import typer

from nigel.models import ImporterInfo
from nigel.registry import registry


class PluginHooks:
    def __init__(self):
        self.importers: list[ImporterInfo] = []
        self.commands: list[tuple[typer.Typer, callable]] = []
        self.migrations: list[callable] = []
        self.categories: list[dict] = []

    def add_importer(self, info: ImporterInfo) -> None:
        self.importers.append(info)

    def add_command(self, parent: typer.Typer, command: callable) -> None:
        self.commands.append((parent, command))

    def add_migration(self, fn: callable) -> None:
        self.migrations.append(fn)

    def add_categories(self, categories: list[dict]) -> None:
        self.categories.extend(categories)


def load_plugins(app: typer.Typer, report_app: typer.Typer) -> PluginHooks:
    """Discover installed plugins and collect their hooks."""
    hooks = PluginHooks()

    eps = importlib.metadata.entry_points(group="nigel.plugins")
    for ep in eps:
        plugin_module = ep.load()
        if hasattr(plugin_module, "register"):
            plugin_module.register(hooks, app=app, report_app=report_app)

    for info in hooks.importers:
        registry.register(info)

    for parent, command in hooks.commands:
        parent.command()(command)

    return hooks


def apply_migrations(conn: sqlite3.Connection, hooks: PluginHooks) -> None:
    for fn in hooks.migrations:
        fn(conn)
    conn.commit()


def seed_plugin_categories(conn: sqlite3.Connection, hooks: PluginHooks) -> None:
    for cat in hooks.categories:
        existing = conn.execute(
            "SELECT 1 FROM categories WHERE name = ?", (cat["name"],)
        ).fetchone()
        if existing is None:
            conn.execute(
                "INSERT INTO categories (name, category_type, tax_line, description) VALUES (?, ?, ?, ?)",
                (cat["name"], cat["category_type"], cat.get("tax_line"), cat.get("description")),
            )
    conn.commit()
