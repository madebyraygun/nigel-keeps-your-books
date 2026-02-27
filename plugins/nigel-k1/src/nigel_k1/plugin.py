import typer

from nigel_k1.migrations import create_k1_tables
from nigel_k1.categories import K1_CATEGORIES
from nigel_k1.cli import k1_prep, k1_setup


def register(hooks, app=None, report_app=None, **kwargs):
    hooks.add_migration(create_k1_tables)
    hooks.add_categories(K1_CATEGORIES)

    if report_app is not None:
        hooks.add_command(report_app, k1_prep)

    if app is not None:
        k1_app = typer.Typer(help="K-1 prep report configuration.")
        k1_app.command("setup")(k1_setup)
        app.add_typer(k1_app, name="k1")
