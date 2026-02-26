import os
from pathlib import Path

import typer

from bookkeeper.db import get_connection, init_db

app = typer.Typer(help="Raygun Bookkeeper — cash-basis bookkeeping CLI.", invoke_without_command=True)


@app.callback()
def main():
    """Raygun Bookkeeper — cash-basis bookkeeping CLI."""

DEFAULT_DATA_DIR = Path.home() / "Documents" / "bookkeeper"


def get_data_dir() -> Path:
    return Path(os.environ.get("BOOKKEEPER_DATA_DIR", str(DEFAULT_DATA_DIR)))


def get_db_path() -> Path:
    return get_data_dir() / "raygun.db"


@app.command()
def init():
    """Initialize the bookkeeper database and seed categories."""
    data_dir = get_data_dir()
    data_dir.mkdir(parents=True, exist_ok=True)
    (data_dir / "imports").mkdir(exist_ok=True)
    (data_dir / "exports").mkdir(exist_ok=True)

    conn = get_connection(data_dir / "raygun.db")
    init_db(conn)
    conn.close()

    typer.echo(f"Initialized bookkeeper at {data_dir}")


if __name__ == "__main__":
    app()
