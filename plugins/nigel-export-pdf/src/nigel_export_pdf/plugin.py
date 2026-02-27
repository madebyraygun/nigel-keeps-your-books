import typer

from nigel_export_pdf.cli import (
    export_pnl, export_expenses, export_tax, export_cashflow,
    export_balance, export_flagged, export_all,
)


def register(hooks, app=None, **kwargs):
    if app is None:
        return

    export_app = typer.Typer(help="Export reports to PDF.")
    export_app.command("pnl")(export_pnl)
    export_app.command("expenses")(export_expenses)
    export_app.command("tax")(export_tax)
    export_app.command("cashflow")(export_cashflow)
    export_app.command("balance")(export_balance)
    export_app.command("flagged")(export_flagged)
    export_app.command("all")(export_all)

    app.add_typer(export_app, name="export")
