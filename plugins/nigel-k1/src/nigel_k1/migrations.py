import sqlite3


def create_k1_tables(conn: sqlite3.Connection) -> None:
    conn.executescript("""
        CREATE TABLE IF NOT EXISTS entity_config (
            id INTEGER PRIMARY KEY,
            key TEXT NOT NULL UNIQUE,
            value TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS shareholders (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            ownership_pct REAL NOT NULL,
            is_officer INTEGER DEFAULT 0,
            annual_compensation REAL DEFAULT 0,
            capital_contribution REAL DEFAULT 0,
            beginning_capital REAL DEFAULT 0
        );
    """)
