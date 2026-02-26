# Bookkeeper Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a Python CLI tool (`bookkeeper`) for cash-basis bookkeeping with CSV/XLSX import, rules-based categorization, and SQLite storage.

**Architecture:** Typer CLI → focused business logic modules → SQLite via raw SQL. Data lives at `~/Documents/bookkeeper/`. Each module takes a DB connection as parameter. `db.py` owns schema and connection.

**Tech Stack:** Python 3.12+, uv, Typer, rich, openpyxl, pytest

---

### Task 1: Project Scaffolding

**Files:**
- Create: `pyproject.toml`
- Create: `src/bookkeeper/__init__.py`
- Create: `src/bookkeeper/cli.py`
- Create: `.gitignore`
- Create: `.python-version`

**Step 1: Create pyproject.toml**

```toml
[project]
name = "bookkeeper"
version = "0.1.0"
description = "Cash-basis bookkeeping CLI for Raygun"
requires-python = ">=3.12"
dependencies = [
    "typer>=0.15",
    "rich>=13",
    "openpyxl>=3.1",
]

[project.scripts]
bookkeeper = "bookkeeper.cli:app"

[build-system]
requires = ["hatchling"]
build-backend = "hatchling.build"

[tool.hatch.build.targets.wheel]
packages = ["src/bookkeeper"]

[tool.pytest.ini_options]
testpaths = ["tests"]

[dependency-groups]
dev = [
    "pytest>=8",
]
```

**Step 2: Create .python-version**

```
3.12
```

**Step 3: Create .gitignore**

```
__pycache__/
*.pyc
.venv/
*.egg-info/
dist/
data/
.pytest_cache/
```

**Step 4: Create src/bookkeeper/__init__.py**

```python
"""Raygun Bookkeeper — cash-basis bookkeeping CLI."""
```

**Step 5: Create minimal CLI entry point**

`src/bookkeeper/cli.py`:
```python
import typer

app = typer.Typer(help="Raygun Bookkeeper — cash-basis bookkeeping CLI.")


@app.command()
def init():
    """Initialize the bookkeeper database and seed categories."""
    typer.echo("Not yet implemented.")


if __name__ == "__main__":
    app()
```

**Step 6: Install and verify**

```bash
cd /Users/dalton/Dev/bookkeeper
uv sync
uv run bookkeeper --help
```

Expected: Help output showing `init` command.

**Step 7: Commit**

```bash
git add pyproject.toml .python-version .gitignore src/
git commit -m "Scaffold project with uv, Typer CLI entry point"
```

---

### Task 2: Database Layer — Schema and Connection

**Files:**
- Create: `src/bookkeeper/db.py`
- Create: `tests/conftest.py`
- Create: `tests/test_db.py`

**Step 1: Write the failing test**

`tests/test_db.py`:
```python
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
```

**Step 2: Run test to verify it fails**

```bash
uv run pytest tests/test_db.py -v
```

Expected: FAIL — `ModuleNotFoundError: No module named 'bookkeeper.db'`

**Step 3: Write conftest.py with shared fixtures**

`tests/conftest.py`:
```python
import sqlite3

import pytest

from bookkeeper.db import get_connection, init_db


@pytest.fixture
def db(tmp_path):
    """Provide an initialized in-memory-style temp DB connection."""
    db_path = tmp_path / "test.db"
    conn = get_connection(db_path)
    init_db(conn)
    yield conn
    conn.close()
```

**Step 4: Implement db.py**

`src/bookkeeper/db.py`:
```python
import sqlite3
from pathlib import Path

SCHEMA = """
CREATE TABLE IF NOT EXISTS accounts (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    account_type TEXT NOT NULL,
    institution TEXT,
    last_four TEXT,
    created_at TEXT DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS categories (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    parent_id INTEGER,
    category_type TEXT NOT NULL,
    tax_line TEXT,
    description TEXT,
    is_active INTEGER DEFAULT 1,
    FOREIGN KEY (parent_id) REFERENCES categories(id)
);

CREATE TABLE IF NOT EXISTS imports (
    id INTEGER PRIMARY KEY,
    filename TEXT NOT NULL,
    account_id INTEGER NOT NULL,
    import_date TEXT DEFAULT (datetime('now')),
    record_count INTEGER,
    date_range_start TEXT,
    date_range_end TEXT,
    checksum TEXT,
    FOREIGN KEY (account_id) REFERENCES accounts(id)
);

CREATE TABLE IF NOT EXISTS transactions (
    id INTEGER PRIMARY KEY,
    account_id INTEGER NOT NULL,
    date TEXT NOT NULL,
    description TEXT NOT NULL,
    amount REAL NOT NULL,
    category_id INTEGER,
    vendor TEXT,
    notes TEXT,
    is_flagged INTEGER DEFAULT 0,
    flag_reason TEXT,
    import_id INTEGER,
    created_at TEXT DEFAULT (datetime('now')),
    FOREIGN KEY (account_id) REFERENCES accounts(id),
    FOREIGN KEY (category_id) REFERENCES categories(id),
    FOREIGN KEY (import_id) REFERENCES imports(id)
);

CREATE TABLE IF NOT EXISTS rules (
    id INTEGER PRIMARY KEY,
    pattern TEXT NOT NULL,
    match_type TEXT DEFAULT 'contains',
    vendor TEXT,
    category_id INTEGER NOT NULL,
    priority INTEGER DEFAULT 0,
    hit_count INTEGER DEFAULT 0,
    is_active INTEGER DEFAULT 1,
    created_at TEXT DEFAULT (datetime('now')),
    FOREIGN KEY (category_id) REFERENCES categories(id)
);

CREATE TABLE IF NOT EXISTS reconciliations (
    id INTEGER PRIMARY KEY,
    account_id INTEGER NOT NULL,
    month TEXT NOT NULL,
    statement_balance REAL,
    calculated_balance REAL,
    is_reconciled INTEGER DEFAULT 0,
    reconciled_at TEXT,
    notes TEXT,
    FOREIGN KEY (account_id) REFERENCES accounts(id)
);
"""

DEFAULT_CATEGORIES = [
    # Income
    ("Client Services", None, "income", "Gross receipts", "Project fees, retainer payments"),
    ("Hosting & Maintenance", None, "income", "Gross receipts", "Recurring client hosting/maintenance fees"),
    ("Reimbursements", None, "income", "Gross receipts", "Client reimbursements for expenses"),
    ("Interest Income", None, "income", "Other income", "Bank interest"),
    ("Other Income", None, "income", "Other income", "Anything else"),
    # Expenses
    ("Advertising & Marketing", None, "expense", "Line 8", "Ads, sponsorships, marketing tools"),
    ("Car & Truck", None, "expense", "Line 9", "Mileage, fuel, parking"),
    ("Commissions & Fees", None, "expense", "Line 10", "Subcontractor commissions, platform fees"),
    ("Contract Labor", None, "expense", "Line 11", "Freelancers, subcontractors (1099 work)"),
    ("Insurance", None, "expense", "Line 15", "Business insurance, E&O"),
    ("Legal & Professional", None, "expense", "Line 17", "Accountant, lawyer, professional services"),
    ("Office Expense", None, "expense", "Line 18", "Office supplies, minor equipment"),
    ("Rent / Lease", None, "expense", "Line 20b", "Office rent, coworking"),
    ("Software & Subscriptions", None, "expense", "Line 18/27a", "SaaS tools, domain renewals, cloud services"),
    ("Hosting & Infrastructure", None, "expense", "Line 18/27a", "AWS, server costs, CDN"),
    ("Taxes & Licenses", None, "expense", "Line 23", "Business licenses, state fees"),
    ("Travel", None, "expense", "Line 24a", "Flights, hotels, conference travel"),
    ("Meals", None, "expense", "Line 24b", "Business meals (50% deductible)"),
    ("Utilities", None, "expense", "Line 25", "Internet, phone (business portion)"),
    ("Payroll — Wages", None, "expense", "Line 26", "Employee salaries (from Gusto)"),
    ("Payroll — Taxes", None, "expense", "Line 23", "Employer payroll taxes (from Gusto)"),
    ("Payroll — Benefits", None, "expense", "Line 14", "Health insurance, retirement (from Gusto)"),
    ("Bank & Merchant Fees", None, "expense", "Line 27a", "Stripe fees, bank charges, wire fees"),
    ("Education & Training", None, "expense", "Line 27a", "Courses, books, conferences"),
    ("Equipment", None, "expense", "Line 13", "Hardware, major purchases"),
    ("Home Office", None, "expense", "Line 30", "Simplified method or actual expenses"),
    ("Owner Draw / Distribution", None, "expense", "Not deductible", "Owner payments, distributions"),
    ("Transfer", None, "expense", "Not deductible", "Transfers between own accounts"),
    ("Uncategorized", None, "expense", "—", "Needs review"),
]


def get_connection(db_path: Path) -> sqlite3.Connection:
    """Open a SQLite connection with WAL mode and foreign keys enabled."""
    conn = sqlite3.connect(str(db_path))
    conn.execute("PRAGMA journal_mode=WAL")
    conn.execute("PRAGMA foreign_keys=ON")
    conn.row_factory = sqlite3.Row
    return conn


def init_db(conn: sqlite3.Connection) -> None:
    """Create tables and seed default categories. Idempotent."""
    conn.executescript(SCHEMA)

    cursor = conn.execute("SELECT count(*) FROM categories")
    if cursor.fetchone()[0] == 0:
        conn.executemany(
            "INSERT INTO categories (name, parent_id, category_type, tax_line, description) "
            "VALUES (?, ?, ?, ?, ?)",
            DEFAULT_CATEGORIES,
        )
        conn.commit()
```

**Step 5: Run tests**

```bash
uv run pytest tests/test_db.py -v
```

Expected: All 4 tests PASS.

**Step 6: Commit**

```bash
git add src/bookkeeper/db.py tests/conftest.py tests/test_db.py
git commit -m "Add database layer with schema and default category seeding"
```

---

### Task 3: Models

**Files:**
- Create: `src/bookkeeper/models.py`

**Step 1: Create dataclasses**

`src/bookkeeper/models.py`:
```python
from dataclasses import dataclass
from datetime import date


@dataclass
class Account:
    id: int
    name: str
    account_type: str  # checking, credit_card, line_of_credit, payroll
    institution: str | None = None
    last_four: str | None = None


@dataclass
class Category:
    id: int
    name: str
    category_type: str  # income or expense
    parent_id: int | None = None
    tax_line: str | None = None
    description: str | None = None
    is_active: bool = True


@dataclass
class Transaction:
    id: int | None
    account_id: int
    date: str  # ISO 8601
    description: str
    amount: float  # negative = expense, positive = income
    category_id: int | None = None
    vendor: str | None = None
    notes: str | None = None
    is_flagged: bool = False
    flag_reason: str | None = None
    import_id: int | None = None


@dataclass
class Rule:
    id: int | None
    pattern: str
    category_id: int
    match_type: str = "contains"  # contains, starts_with, regex
    vendor: str | None = None
    priority: int = 0
    hit_count: int = 0
    is_active: bool = True


@dataclass
class ImportRecord:
    id: int | None
    filename: str
    account_id: int
    record_count: int | None = None
    date_range_start: str | None = None
    date_range_end: str | None = None
    checksum: str | None = None


@dataclass
class ParsedRow:
    """Intermediate representation from a CSV/XLSX parser before DB insert."""
    date: str  # ISO 8601
    description: str
    amount: float  # normalized: negative = expense, positive = income
```

**Step 2: Commit**

```bash
git add src/bookkeeper/models.py
git commit -m "Add dataclass models for all entities"
```

---

### Task 4: CLI — init command with data directory setup

**Files:**
- Modify: `src/bookkeeper/cli.py`
- Create: `tests/test_cli.py`

**Step 1: Write the failing test**

`tests/test_cli.py`:
```python
from typer.testing import CliRunner

from bookkeeper.cli import app

runner = CliRunner()


def test_init_creates_data_dir_and_db(tmp_path, monkeypatch):
    data_dir = tmp_path / "bookkeeper"
    monkeypatch.setenv("BOOKKEEPER_DATA_DIR", str(data_dir))

    result = runner.invoke(app, ["init"])
    assert result.exit_code == 0
    assert (data_dir / "raygun.db").exists()
    assert (data_dir / "imports").is_dir()
    assert (data_dir / "exports").is_dir()


def test_init_is_idempotent(tmp_path, monkeypatch):
    data_dir = tmp_path / "bookkeeper"
    monkeypatch.setenv("BOOKKEEPER_DATA_DIR", str(data_dir))

    result1 = runner.invoke(app, ["init"])
    result2 = runner.invoke(app, ["init"])
    assert result1.exit_code == 0
    assert result2.exit_code == 0
```

**Step 2: Run test to verify it fails**

```bash
uv run pytest tests/test_cli.py -v
```

Expected: FAIL

**Step 3: Implement cli.py with init command**

`src/bookkeeper/cli.py`:
```python
import os
from pathlib import Path

import typer

from bookkeeper.db import get_connection, init_db

app = typer.Typer(help="Raygun Bookkeeper — cash-basis bookkeeping CLI.")

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
```

**Step 4: Run tests**

```bash
uv run pytest tests/test_cli.py -v
```

Expected: PASS

**Step 5: Commit**

```bash
git add src/bookkeeper/cli.py tests/test_cli.py
git commit -m "Implement init command with data directory setup"
```

---

### Task 5: Accounts Management

**Files:**
- Modify: `src/bookkeeper/cli.py`
- Modify: `tests/test_cli.py`

**Step 1: Write failing tests**

Append to `tests/test_cli.py`:
```python
def test_accounts_add_and_list(tmp_path, monkeypatch):
    data_dir = tmp_path / "bookkeeper"
    monkeypatch.setenv("BOOKKEEPER_DATA_DIR", str(data_dir))
    runner.invoke(app, ["init"])

    result = runner.invoke(
        app,
        ["accounts", "add", "BofA Checking", "--type", "checking",
         "--institution", "Bank of America", "--last-four", "1234"],
    )
    assert result.exit_code == 0

    result = runner.invoke(app, ["accounts", "list"])
    assert result.exit_code == 0
    assert "BofA Checking" in result.output
```

**Step 2: Run test to verify it fails**

```bash
uv run pytest tests/test_cli.py::test_accounts_add_and_list -v
```

Expected: FAIL

**Step 3: Add accounts subcommands to cli.py**

Add to `src/bookkeeper/cli.py`:
```python
from rich.console import Console
from rich.table import Table

console = Console()

accounts_app = typer.Typer(help="Manage accounts.")
app.add_typer(accounts_app, name="accounts")


@accounts_app.command("add")
def accounts_add(
    name: str = typer.Argument(help="Account name, e.g. 'BofA Checking'"),
    type: str = typer.Option(help="Account type: checking, credit_card, line_of_credit, payroll"),
    institution: str = typer.Option(None, help="Institution name"),
    last_four: str = typer.Option(None, help="Last 4 digits of account number"),
):
    """Add a new account."""
    conn = get_connection(get_db_path())
    conn.execute(
        "INSERT INTO accounts (name, account_type, institution, last_four) VALUES (?, ?, ?, ?)",
        (name, type, institution, last_four),
    )
    conn.commit()
    conn.close()
    typer.echo(f"Added account: {name}")


@accounts_app.command("list")
def accounts_list():
    """List all accounts."""
    conn = get_connection(get_db_path())
    rows = conn.execute("SELECT id, name, account_type, institution, last_four FROM accounts").fetchall()
    conn.close()

    table = Table(title="Accounts")
    table.add_column("ID", style="dim")
    table.add_column("Name")
    table.add_column("Type")
    table.add_column("Institution")
    table.add_column("Last Four")
    for row in rows:
        table.add_row(str(row["id"]), row["name"], row["account_type"], row["institution"] or "", row["last_four"] or "")
    console.print(table)
```

**Step 4: Run tests**

```bash
uv run pytest tests/test_cli.py -v
```

Expected: All PASS

**Step 5: Commit**

```bash
git add src/bookkeeper/cli.py tests/test_cli.py
git commit -m "Add accounts add and list commands"
```

---

### Task 6: BofA Checking Importer

**Files:**
- Create: `src/bookkeeper/importer.py`
- Create: `tests/test_importer.py`
- Create: `tests/fixtures/bofa_checking_sample.csv`

**Step 1: Create test fixture**

`tests/fixtures/bofa_checking_sample.csv`:
```csv
Description,,,,Summary Amt.
Total credits,,,,,"10,000.00"
Total debits,,,,,"-5,000.00"

,,,,
12/02/2024,"Beginning balance as of 12/02/2024",,"22,548.05"
Date,Description,Amount,Running Bal.
12/02/2024,"ACME CORP DES:PAYMENT ID:12345 INDN:Raygun Design, LLC CO ID:XXXXX42850 CCD","-2,500.00","20,048.05"
12/03/2024,"CLIENT PROJ DES:IAT PAYPAL ID:XXXXX54881426 INDN:Some LLC CO ID:XXXXX0487C IAT","-304.72","19,743.33"
12/06/2024,"Bank of America Business Card Bill Payment","-2,500.00","17,243.33"
01/09/2025,"Online Banking advance from CRD 4445 Confirmation# XXXXX05873","5,000.00","22,243.33"
01/13/2025,"CLIENT PAY DES:Receivable ID:016AE INDN:Client LLC CO ID:XXXXX95317 CCD","2,400.00","24,643.33"
```

**Step 2: Write failing tests**

`tests/test_importer.py`:
```python
from pathlib import Path

from bookkeeper.importer import parse_bofa_checking, import_file
from bookkeeper.db import init_db, get_connection

FIXTURES = Path(__file__).parent / "fixtures"


def test_parse_bofa_checking():
    rows = parse_bofa_checking(FIXTURES / "bofa_checking_sample.csv")
    assert len(rows) == 5
    # First transaction
    assert rows[0].date == "2024-12-02"
    assert rows[0].amount == -2500.00
    assert "ACME CORP" in rows[0].description
    # Positive amount (deposit)
    assert rows[3].date == "2025-01-09"
    assert rows[3].amount == 5000.00


def test_parse_bofa_checking_skips_preamble():
    rows = parse_bofa_checking(FIXTURES / "bofa_checking_sample.csv")
    for row in rows:
        assert "Beginning balance" not in row.description
        assert "Summary" not in row.description


def test_import_file_inserts_transactions(db):
    db.execute(
        "INSERT INTO accounts (name, account_type) VALUES (?, ?)",
        ("BofA Checking", "checking"),
    )
    db.commit()

    result = import_file(db, FIXTURES / "bofa_checking_sample.csv", "BofA Checking")
    assert result["imported"] == 5
    assert result["skipped"] == 0

    cursor = db.execute("SELECT count(*) FROM transactions")
    assert cursor.fetchone()[0] == 5


def test_import_file_detects_row_duplicates(db):
    db.execute(
        "INSERT INTO accounts (name, account_type) VALUES (?, ?)",
        ("BofA Checking", "checking"),
    )
    db.commit()

    import_file(db, FIXTURES / "bofa_checking_sample.csv", "BofA Checking")
    result = import_file(db, FIXTURES / "bofa_checking_sample.csv", "BofA Checking")
    assert result["imported"] == 0
    assert result["skipped"] == 5

    cursor = db.execute("SELECT count(*) FROM transactions")
    assert cursor.fetchone()[0] == 5


def test_import_file_records_import_batch(db):
    db.execute(
        "INSERT INTO accounts (name, account_type) VALUES (?, ?)",
        ("BofA Checking", "checking"),
    )
    db.commit()

    import_file(db, FIXTURES / "bofa_checking_sample.csv", "BofA Checking")

    cursor = db.execute("SELECT * FROM imports")
    record = cursor.fetchone()
    assert record["filename"] == "bofa_checking_sample.csv"
    assert record["record_count"] == 5
    assert record["checksum"] is not None
```

**Step 3: Run tests to verify they fail**

```bash
uv run pytest tests/test_importer.py -v
```

Expected: FAIL — `ModuleNotFoundError`

**Step 4: Implement importer.py**

`src/bookkeeper/importer.py`:
```python
import csv
import hashlib
import shutil
import sqlite3
from datetime import datetime
from pathlib import Path

from bookkeeper.models import ParsedRow


def _compute_checksum(file_path: Path) -> str:
    return hashlib.sha256(file_path.read_bytes()).hexdigest()


def _parse_amount(raw: str) -> float:
    """Strip commas and quotes, return float."""
    return float(raw.replace(",", "").replace('"', "").strip())


def _parse_date_mdy(raw: str) -> str:
    """Convert MM/DD/YYYY to ISO 8601 YYYY-MM-DD."""
    return datetime.strptime(raw.strip(), "%m/%d/%Y").strftime("%Y-%m-%d")


def parse_bofa_checking(file_path: Path) -> list[ParsedRow]:
    """Parse a BofA checking CSV, skipping the preamble rows."""
    rows: list[ParsedRow] = []
    with open(file_path, newline="", encoding="utf-8-sig") as f:
        reader = csv.reader(f)
        # Find the header row: "Date,Description,Amount,Running Bal."
        for line in reader:
            if len(line) >= 4 and line[0].strip() == "Date" and "Description" in line[1]:
                break
        # Parse data rows
        for line in reader:
            if len(line) < 3 or not line[0].strip():
                continue
            try:
                date = _parse_date_mdy(line[0])
            except ValueError:
                continue
            description = line[1].strip()
            if not description or "Beginning balance" in description:
                continue
            amount = _parse_amount(line[2])
            rows.append(ParsedRow(date=date, description=description, amount=amount))
    return rows


def parse_bofa_credit_card(file_path: Path) -> list[ParsedRow]:
    """Parse a BofA credit card CSV. Amounts are always positive; use Transaction Type for sign."""
    rows: list[ParsedRow] = []
    with open(file_path, newline="", encoding="utf-8-sig") as f:
        reader = csv.reader(f)
        # Find header row containing "Posting Date"
        for line in reader:
            if any("Posting Date" in cell for cell in line):
                break
        for line in reader:
            if len(line) < 10 or not line[2].strip():
                continue
            try:
                date = _parse_date_mdy(line[3])  # Trans. Date
            except ValueError:
                continue
            description = line[5].strip()
            amount = _parse_amount(line[6])
            txn_type = line[9].strip()
            # D = charge (expense), C = credit/payment
            if txn_type == "D":
                amount = -abs(amount)
            else:
                amount = abs(amount)
            rows.append(ParsedRow(date=date, description=description, amount=amount))
    return rows


def parse_bofa_line_of_credit(file_path: Path) -> list[ParsedRow]:
    """Parse a BofA line of credit CSV. Same columns as credit card but amounts are pre-signed."""
    rows: list[ParsedRow] = []
    with open(file_path, newline="", encoding="utf-8-sig") as f:
        reader = csv.reader(f)
        for line in reader:
            if any("Posting Date" in cell for cell in line):
                break
        for line in reader:
            if len(line) < 10 or not line[2].strip():
                continue
            try:
                date = _parse_date_mdy(line[3])  # Trans. Date
            except ValueError:
                continue
            description = line[5].strip()
            amount = _parse_amount(line[6])
            # Line of credit: positive = charge/expense (negate), negative = payment (keep)
            amount = -amount
            rows.append(ParsedRow(date=date, description=description, amount=amount))
    return rows


PARSER_MAP = {
    "checking": parse_bofa_checking,
    "credit_card": parse_bofa_credit_card,
    "line_of_credit": parse_bofa_line_of_credit,
}


def _is_duplicate_row(conn: sqlite3.Connection, account_id: int, row: ParsedRow) -> bool:
    cursor = conn.execute(
        "SELECT 1 FROM transactions WHERE account_id = ? AND date = ? AND amount = ? AND description = ?",
        (account_id, row.date, row.amount, row.description),
    )
    return cursor.fetchone() is not None


def import_file(
    conn: sqlite3.Connection,
    file_path: Path,
    account_name: str,
) -> dict:
    """Import a CSV file into the database. Returns counts of imported/skipped."""
    # Look up account
    cursor = conn.execute(
        "SELECT id, account_type FROM accounts WHERE name = ?", (account_name,)
    )
    account = cursor.fetchone()
    if account is None:
        raise ValueError(f"Unknown account: {account_name}")

    account_id = account["id"]
    account_type = account["account_type"]

    # Check file-level duplicate
    checksum = _compute_checksum(file_path)
    cursor = conn.execute(
        "SELECT 1 FROM imports WHERE checksum = ? AND account_id = ?",
        (checksum, account_id),
    )
    if cursor.fetchone() is not None:
        return {"imported": 0, "skipped": 0, "duplicate_file": True}

    # Parse
    parser = PARSER_MAP.get(account_type)
    if parser is None:
        raise ValueError(f"No parser for account type: {account_type}")
    parsed_rows = parser(file_path)

    # Insert
    imported = 0
    skipped = 0
    for row in parsed_rows:
        if _is_duplicate_row(conn, account_id, row):
            skipped += 1
            continue
        conn.execute(
            "INSERT INTO transactions (account_id, date, description, amount, is_flagged, flag_reason) "
            "VALUES (?, ?, ?, ?, 1, 'No matching rule')",
            (account_id, row.date, row.description, row.amount),
        )
        imported += 1

    # Record import batch
    dates = [r.date for r in parsed_rows]
    conn.execute(
        "INSERT INTO imports (filename, account_id, record_count, date_range_start, date_range_end, checksum) "
        "VALUES (?, ?, ?, ?, ?, ?)",
        (
            file_path.name,
            account_id,
            imported,
            min(dates) if dates else None,
            max(dates) if dates else None,
            checksum,
        ),
    )
    conn.commit()

    return {"imported": imported, "skipped": skipped, "duplicate_file": False}
```

**Step 5: Run tests**

```bash
uv run pytest tests/test_importer.py -v
```

Expected: All PASS

**Step 6: Commit**

```bash
git add src/bookkeeper/importer.py tests/test_importer.py tests/fixtures/
git commit -m "Add BofA checking CSV parser with duplicate detection"
```

---

### Task 7: BofA Credit Card and Line of Credit Importers

**Files:**
- Create: `tests/fixtures/bofa_credit_card_sample.csv`
- Create: `tests/fixtures/bofa_loc_sample.csv`
- Modify: `tests/test_importer.py`

**Step 1: Create credit card test fixture**

`tests/fixtures/bofa_credit_card_sample.csv`:
```csv
Description,,,,Summary Amt.
Total credits,,,,,"-500.00"
Total debits,,,,,"1200.00"

CardHolder Name,Account/Card Number - last 4 digits,Posting Date,Trans. Date,Reference ID,Description,Amount,MCC,Merchant Category,Transaction Type,Expense Category
DALTON ROONEY,3083,03/12/2025,03/10/2025,Ref: 244310650701,"ADOBE INC","54.43",5818,"DIGITAL GOODS",D,Software
DALTON ROONEY,3083,03/15/2025,03/14/2025,Ref: 247933851590,"AMZN MKTP US","127.99",5942,"BOOK STORES",D,Shopping
STACEY EDELSTEIN,8840,03/18/2025,03/17/2025,Ref: 240070451729,"PAYMENT RECEIVED","500.00",0000,"",C,
```

**Step 2: Create line of credit test fixture**

`tests/fixtures/bofa_loc_sample.csv`:
```csv
Description,,,,Summary Amt.
Total credits,,,,,"-3000.00"
Total debits,,,,,"260.97"

CardHolder Name,Account/Card Number - last 4 digits,Posting Date,Trans. Date,Reference ID,Description,Amount,MCC,Merchant Category,Transaction Type,Expense Category
RAYGUN DESIGN LLC,2194,12/13/2024,12/13/2024,,CASH  * FINANCE CHARGE *,110.97,0000,,D,
RAYGUN DESIGN LLC,2194,12/06/2024,12/06/2024,Ref: 34183204320120600051648,PAYMENT - THANK YOU,-500.00,0000,,C,
RAYGUN DESIGN LLC,2194,08/01/2025,08/01/2025,,ANNUAL MEMBERSHIP FEE,150.00,0000,,D,
```

**Step 3: Write failing tests**

Append to `tests/test_importer.py`:
```python
def test_parse_bofa_credit_card():
    rows = parse_bofa_credit_card(FIXTURES / "bofa_credit_card_sample.csv")
    assert len(rows) == 3
    # D type = charge, should be negative
    assert rows[0].amount == -54.43
    assert rows[0].date == "2025-03-10"  # Uses Trans. Date, not Posting Date
    assert "ADOBE" in rows[0].description
    # C type = payment, should be positive
    assert rows[2].amount == 500.00


def test_parse_bofa_line_of_credit():
    rows = parse_bofa_line_of_credit(FIXTURES / "bofa_loc_sample.csv")
    assert len(rows) == 3
    # Positive in CSV = charge → negate to -110.97
    assert rows[0].amount == -110.97
    assert "FINANCE CHARGE" in rows[0].description
    # Negative in CSV = payment → negate to +500.00
    assert rows[1].amount == 500.00


def test_import_credit_card(db):
    db.execute(
        "INSERT INTO accounts (name, account_type) VALUES (?, ?)",
        ("BofA CC", "credit_card"),
    )
    db.commit()
    result = import_file(db, FIXTURES / "bofa_credit_card_sample.csv", "BofA CC")
    assert result["imported"] == 3


def test_import_line_of_credit(db):
    db.execute(
        "INSERT INTO accounts (name, account_type) VALUES (?, ?)",
        ("BofA LOC", "line_of_credit"),
    )
    db.commit()
    result = import_file(db, FIXTURES / "bofa_loc_sample.csv", "BofA LOC")
    assert result["imported"] == 3
```

**Step 4: Run tests**

```bash
uv run pytest tests/test_importer.py -v
```

Expected: All PASS (parsers already implemented in Task 6)

**Step 5: Commit**

```bash
git add tests/fixtures/ tests/test_importer.py
git commit -m "Add BofA credit card and line of credit parser tests"
```

---

### Task 8: Gusto Payroll XLSX Importer

**Files:**
- Modify: `src/bookkeeper/importer.py`
- Modify: `tests/test_importer.py`
- Create: `tests/fixtures/gusto_sample.xlsx` (generated programmatically in test)

**Step 1: Write failing tests**

Append to `tests/test_importer.py`:
```python
import openpyxl

from bookkeeper.importer import parse_gusto_payroll


def _create_gusto_fixture(path):
    """Create a minimal Gusto XLSX fixture programmatically."""
    wb = openpyxl.Workbook()

    # payrolls sheet
    ws = wb.active
    ws.title = "payrolls"
    ws.append(["Id", "Employee id", "Employee name", "Check date", "Payment period start",
               "Payment period end", "Payment method", "Gross pay", "Net pay"])
    # Check date as Excel serial: 2025-01-10 = 45667
    ws.append(["payrolls_aaa", "emp1", "Doe, Jane", 45667, 45649, 45662, "Direct Deposit", 4000.00, 3000.00])
    ws.append(["payrolls_aaa", "emp2", "Doe, John", 45667, 45649, 45662, "Direct Deposit", 3500.00, 2600.00])
    # Second pay period: 2025-01-24 = 45681
    ws.append(["payrolls_bbb", "emp1", "Doe, Jane", 45681, 45663, 45676, "Direct Deposit", 4000.00, 3000.00])
    ws.append(["payrolls_bbb", "emp2", "Doe, John", 45681, 45663, 45676, "Direct Deposit", 3500.00, 2600.00])

    # taxes sheet
    ws2 = wb.create_sheet("taxes")
    ws2.append(["Employee id", "Employee name", "Payroll id", "Payroll check date",
                "Payroll payment period", "Tax", "Type", "Amount", "Subject wage", "Gross subject wage"])
    # Employer taxes for payrolls_aaa
    ws2.append(["emp1", "Doe, Jane", "payrolls_aaa", 45667, "2024-12-23 - 2025-01-05",
                "Social Security", "Employer", 248.00, 4000.00, 4000.00])
    ws2.append(["emp1", "Doe, Jane", "payrolls_aaa", 45667, "2024-12-23 - 2025-01-05",
                "Medicare", "Employer", 58.00, 4000.00, 4000.00])
    ws2.append(["emp1", "Doe, Jane", "payrolls_aaa", 45667, "2024-12-23 - 2025-01-05",
                "FUTA", "Employer", 24.00, 4000.00, 4000.00])
    ws2.append(["emp2", "Doe, John", "payrolls_aaa", 45667, "2024-12-23 - 2025-01-05",
                "Social Security", "Employer", 217.00, 3500.00, 3500.00])
    ws2.append(["emp2", "Doe, John", "payrolls_aaa", 45667, "2024-12-23 - 2025-01-05",
                "Medicare", "Employer", 50.75, 3500.00, 3500.00])
    ws2.append(["emp2", "Doe, John", "payrolls_aaa", 45667, "2024-12-23 - 2025-01-05",
                "FUTA", "Employer", 21.00, 3500.00, 3500.00])
    # Employee taxes (should be ignored for our purposes)
    ws2.append(["emp1", "Doe, Jane", "payrolls_aaa", 45667, "2024-12-23 - 2025-01-05",
                "Federal Income Tax", "Employee", 500.00, 4000.00, 4000.00])

    # deductions sheet (header only per real data)
    ws3 = wb.create_sheet("deductions")
    ws3.append(["Employee id", "Employee name", "Payroll id", "Payroll check date",
                "Payroll payment period", "Name", "Type", "Employee deduction", "Employer contribution"])

    wb.save(path)


def test_parse_gusto_payroll(tmp_path):
    fixture = tmp_path / "gusto_sample.xlsx"
    _create_gusto_fixture(fixture)

    rows = parse_gusto_payroll(fixture)
    # Should produce 2 rows per pay period: wages + employer taxes
    # Pay period 1: wages=7500, employer taxes=618.75
    # Pay period 2: wages=7500, employer taxes (not in fixture, so just wages)
    wages_rows = [r for r in rows if "Wages" in r.description]
    tax_rows = [r for r in rows if "Taxes" in r.description]

    assert len(wages_rows) == 2
    assert wages_rows[0].amount == -7500.00  # Negative because expense
    assert tax_rows[0].amount == -618.75


def test_import_gusto_payroll(db, tmp_path):
    fixture = tmp_path / "gusto_sample.xlsx"
    _create_gusto_fixture(fixture)

    db.execute(
        "INSERT INTO accounts (name, account_type) VALUES (?, ?)",
        ("Gusto", "payroll"),
    )
    db.commit()

    result = import_file(db, fixture, "Gusto")
    assert result["imported"] >= 2  # At least wages entries
```

**Step 2: Run tests to verify they fail**

```bash
uv run pytest tests/test_importer.py::test_parse_gusto_payroll -v
```

Expected: FAIL — `ImportError: cannot import name 'parse_gusto_payroll'`

**Step 3: Implement Gusto parser**

Add to `src/bookkeeper/importer.py`:
```python
from openpyxl import load_workbook


def _excel_serial_to_date(serial: int | float) -> str:
    """Convert Excel serial date number to ISO 8601 string."""
    from datetime import datetime, timedelta
    base = datetime(1899, 12, 30)  # Excel epoch (accounting for the 1900 leap year bug)
    return (base + timedelta(days=int(serial))).strftime("%Y-%m-%d")


def parse_gusto_payroll(file_path: Path) -> list[ParsedRow]:
    """Parse a Gusto payroll XLSX. Aggregates per pay period, discards employee detail."""
    wb = load_workbook(file_path, read_only=True, data_only=True)

    # Aggregate gross pay per check date from payrolls sheet
    payrolls_ws = wb["payrolls"]
    payroll_rows = list(payrolls_ws.iter_rows(min_row=2, values_only=True))
    # Group by check date (column index 3)
    wages_by_date: dict[str, float] = {}
    for row in payroll_rows:
        if row[3] is None or row[7] is None:
            continue
        check_date = _excel_serial_to_date(row[3]) if isinstance(row[3], (int, float)) else str(row[3])
        gross = float(row[7])
        wages_by_date[check_date] = wages_by_date.get(check_date, 0.0) + gross

    # Aggregate employer taxes per check date from taxes sheet
    taxes_ws = wb["taxes"]
    tax_rows = list(taxes_ws.iter_rows(min_row=2, values_only=True))
    employer_taxes_by_date: dict[str, float] = {}
    for row in tax_rows:
        if row[6] != "Employer" or row[3] is None or row[7] is None:
            continue
        check_date = _excel_serial_to_date(row[3]) if isinstance(row[3], (int, float)) else str(row[3])
        amount = float(row[7])
        employer_taxes_by_date[check_date] = employer_taxes_by_date.get(check_date, 0.0) + amount

    wb.close()

    # Build parsed rows — expenses are negative
    result: list[ParsedRow] = []
    for date, total in sorted(wages_by_date.items()):
        result.append(ParsedRow(
            date=date,
            description=f"Payroll — Wages ({date})",
            amount=-abs(total),
        ))
    for date, total in sorted(employer_taxes_by_date.items()):
        result.append(ParsedRow(
            date=date,
            description=f"Payroll — Employer Taxes ({date})",
            amount=-abs(total),
        ))

    return result
```

Update `PARSER_MAP`:
```python
PARSER_MAP = {
    "checking": parse_bofa_checking,
    "credit_card": parse_bofa_credit_card,
    "line_of_credit": parse_bofa_line_of_credit,
    "payroll": parse_gusto_payroll,
}
```

**Step 4: Run tests**

```bash
uv run pytest tests/test_importer.py -v
```

Expected: All PASS

**Step 5: Commit**

```bash
git add src/bookkeeper/importer.py tests/test_importer.py
git commit -m "Add Gusto payroll XLSX parser with aggregate extraction"
```

---

### Task 9: Import CLI Command

**Files:**
- Modify: `src/bookkeeper/cli.py`
- Modify: `tests/test_cli.py`

**Step 1: Write failing test**

Append to `tests/test_cli.py`:
```python
from pathlib import Path

FIXTURES = Path(__file__).parent / "fixtures"


def test_import_command(tmp_path, monkeypatch):
    data_dir = tmp_path / "bookkeeper"
    monkeypatch.setenv("BOOKKEEPER_DATA_DIR", str(data_dir))
    runner.invoke(app, ["init"])
    runner.invoke(app, ["accounts", "add", "BofA Checking", "--type", "checking"])

    result = runner.invoke(
        app, ["import", str(FIXTURES / "bofa_checking_sample.csv"), "--account", "BofA Checking"]
    )
    assert result.exit_code == 0
    assert "5 imported" in result.output


def test_import_command_copies_file_to_imports(tmp_path, monkeypatch):
    data_dir = tmp_path / "bookkeeper"
    monkeypatch.setenv("BOOKKEEPER_DATA_DIR", str(data_dir))
    runner.invoke(app, ["init"])
    runner.invoke(app, ["accounts", "add", "BofA Checking", "--type", "checking"])

    runner.invoke(
        app, ["import", str(FIXTURES / "bofa_checking_sample.csv"), "--account", "BofA Checking"]
    )
    assert (data_dir / "imports" / "bofa_checking_sample.csv").exists()
```

**Step 2: Run test to verify it fails**

```bash
uv run pytest tests/test_cli.py::test_import_command -v
```

**Step 3: Implement import command**

Add to `src/bookkeeper/cli.py`:
```python
from bookkeeper.importer import import_file


@app.command("import")
def import_cmd(
    file: Path = typer.Argument(help="Path to CSV or XLSX file to import"),
    account: str = typer.Option(help="Account name to import into"),
):
    """Import a CSV/XLSX file and auto-categorize transactions."""
    conn = get_connection(get_db_path())
    result = import_file(conn, file, account)
    conn.close()

    if result.get("duplicate_file"):
        typer.echo("This file has already been imported (duplicate checksum).")
        return

    typer.echo(f"{result['imported']} imported, {result['skipped']} skipped (duplicates)")

    # Archive the import file
    dest = get_data_dir() / "imports" / file.name
    if not dest.exists():
        import shutil
        shutil.copy2(file, dest)
```

**Step 4: Run tests**

```bash
uv run pytest tests/test_cli.py -v
```

Expected: All PASS

**Step 5: Commit**

```bash
git add src/bookkeeper/cli.py tests/test_cli.py
git commit -m "Add import CLI command with file archiving"
```

---

### Task 10: Categorization Engine

**Files:**
- Create: `src/bookkeeper/categorizer.py`
- Create: `tests/test_categorizer.py`

**Step 1: Write failing tests**

`tests/test_categorizer.py`:
```python
from bookkeeper.categorizer import categorize_transactions


def test_categorize_by_contains_rule(db):
    # Setup: account, category, rule, and uncategorized transaction
    db.execute("INSERT INTO accounts (name, account_type) VALUES ('Test', 'checking')")
    cat_id = db.execute(
        "SELECT id FROM categories WHERE name = 'Software & Subscriptions'"
    ).fetchone()["id"]
    db.execute(
        "INSERT INTO rules (pattern, match_type, vendor, category_id, priority) VALUES (?, ?, ?, ?, ?)",
        ("ADOBE", "contains", "Adobe", cat_id, 10),
    )
    db.execute(
        "INSERT INTO transactions (account_id, date, description, amount, is_flagged, flag_reason) "
        "VALUES (1, '2025-03-10', 'ADOBE INC SUBSCRIPTION', -54.43, 1, 'No matching rule')",
    )
    db.commit()

    result = categorize_transactions(db)
    assert result["categorized"] == 1
    assert result["still_flagged"] == 0

    txn = db.execute("SELECT * FROM transactions WHERE id = 1").fetchone()
    assert txn["category_id"] == cat_id
    assert txn["vendor"] == "Adobe"
    assert txn["is_flagged"] == 0


def test_categorize_by_starts_with_rule(db):
    db.execute("INSERT INTO accounts (name, account_type) VALUES ('Test', 'checking')")
    cat_id = db.execute(
        "SELECT id FROM categories WHERE name = 'Travel'"
    ).fetchone()["id"]
    db.execute(
        "INSERT INTO rules (pattern, match_type, vendor, category_id, priority) VALUES (?, ?, ?, ?, ?)",
        ("UNITED AIR", "starts_with", "United Airlines", cat_id, 10),
    )
    db.execute(
        "INSERT INTO transactions (account_id, date, description, amount, is_flagged, flag_reason) "
        "VALUES (1, '2025-03-10', 'UNITED AIRLINES BOOKING', -350.00, 1, 'No matching rule')",
    )
    db.commit()

    result = categorize_transactions(db)
    assert result["categorized"] == 1


def test_categorize_by_regex_rule(db):
    db.execute("INSERT INTO accounts (name, account_type) VALUES ('Test', 'checking')")
    cat_id = db.execute(
        "SELECT id FROM categories WHERE name = 'Bank & Merchant Fees'"
    ).fetchone()["id"]
    db.execute(
        "INSERT INTO rules (pattern, match_type, vendor, category_id, priority) VALUES (?, ?, ?, ?, ?)",
        (r"STRIPE.*FEE", "regex", "Stripe", cat_id, 10),
    )
    db.execute(
        "INSERT INTO transactions (account_id, date, description, amount, is_flagged, flag_reason) "
        "VALUES (1, '2025-03-10', 'STRIPE PROCESSING FEE', -12.50, 1, 'No matching rule')",
    )
    db.commit()

    result = categorize_transactions(db)
    assert result["categorized"] == 1


def test_higher_priority_rule_wins(db):
    db.execute("INSERT INTO accounts (name, account_type) VALUES ('Test', 'checking')")
    cat_low = db.execute("SELECT id FROM categories WHERE name = 'Office Expense'").fetchone()["id"]
    cat_high = db.execute("SELECT id FROM categories WHERE name = 'Software & Subscriptions'").fetchone()["id"]
    db.execute(
        "INSERT INTO rules (pattern, match_type, vendor, category_id, priority) VALUES (?, ?, ?, ?, ?)",
        ("ADOBE", "contains", "Adobe Office", cat_low, 1),
    )
    db.execute(
        "INSERT INTO rules (pattern, match_type, vendor, category_id, priority) VALUES (?, ?, ?, ?, ?)",
        ("ADOBE", "contains", "Adobe Software", cat_high, 10),
    )
    db.execute(
        "INSERT INTO transactions (account_id, date, description, amount, is_flagged, flag_reason) "
        "VALUES (1, '2025-03-10', 'ADOBE CREATIVE CLOUD', -54.43, 1, 'No matching rule')",
    )
    db.commit()

    categorize_transactions(db)
    txn = db.execute("SELECT * FROM transactions WHERE id = 1").fetchone()
    assert txn["category_id"] == cat_high
    assert txn["vendor"] == "Adobe Software"


def test_unmatched_transactions_stay_flagged(db):
    db.execute("INSERT INTO accounts (name, account_type) VALUES ('Test', 'checking')")
    db.execute(
        "INSERT INTO transactions (account_id, date, description, amount, is_flagged, flag_reason) "
        "VALUES (1, '2025-03-10', 'MYSTERY VENDOR XYZ', -100.00, 1, 'No matching rule')",
    )
    db.commit()

    result = categorize_transactions(db)
    assert result["categorized"] == 0
    assert result["still_flagged"] == 1


def test_categorize_increments_hit_count(db):
    db.execute("INSERT INTO accounts (name, account_type) VALUES ('Test', 'checking')")
    cat_id = db.execute("SELECT id FROM categories WHERE name = 'Software & Subscriptions'").fetchone()["id"]
    db.execute(
        "INSERT INTO rules (pattern, match_type, vendor, category_id, priority) VALUES (?, ?, ?, ?, ?)",
        ("ADOBE", "contains", "Adobe", cat_id, 10),
    )
    db.execute(
        "INSERT INTO transactions (account_id, date, description, amount, is_flagged, flag_reason) "
        "VALUES (1, '2025-03-10', 'ADOBE INC', -54.43, 1, 'No matching rule')",
    )
    db.execute(
        "INSERT INTO transactions (account_id, date, description, amount, is_flagged, flag_reason) "
        "VALUES (1, '2025-04-10', 'ADOBE INC', -54.43, 1, 'No matching rule')",
    )
    db.commit()

    categorize_transactions(db)
    rule = db.execute("SELECT hit_count FROM rules WHERE id = 1").fetchone()
    assert rule["hit_count"] == 2
```

**Step 2: Run tests to verify they fail**

```bash
uv run pytest tests/test_categorizer.py -v
```

Expected: FAIL

**Step 3: Implement categorizer.py**

`src/bookkeeper/categorizer.py`:
```python
import re
import sqlite3


def _matches(description: str, pattern: str, match_type: str) -> bool:
    desc_upper = description.upper()
    pat_upper = pattern.upper()
    if match_type == "contains":
        return pat_upper in desc_upper
    elif match_type == "starts_with":
        return desc_upper.startswith(pat_upper)
    elif match_type == "regex":
        return bool(re.search(pattern, description, re.IGNORECASE))
    return False


def categorize_transactions(conn: sqlite3.Connection) -> dict:
    """Apply rules to all uncategorized transactions. Returns counts."""
    rules = conn.execute(
        "SELECT id, pattern, match_type, vendor, category_id FROM rules "
        "WHERE is_active = 1 ORDER BY priority DESC"
    ).fetchall()

    flagged = conn.execute(
        "SELECT id, description FROM transactions WHERE category_id IS NULL"
    ).fetchall()

    categorized = 0
    still_flagged = 0

    for txn in flagged:
        matched = False
        for rule in rules:
            if _matches(txn["description"], rule["pattern"], rule["match_type"]):
                conn.execute(
                    "UPDATE transactions SET category_id = ?, vendor = ?, is_flagged = 0, flag_reason = NULL "
                    "WHERE id = ?",
                    (rule["category_id"], rule["vendor"], txn["id"]),
                )
                conn.execute(
                    "UPDATE rules SET hit_count = hit_count + 1 WHERE id = ?",
                    (rule["id"],),
                )
                categorized += 1
                matched = True
                break
        if not matched:
            still_flagged += 1

    conn.commit()
    return {"categorized": categorized, "still_flagged": still_flagged}
```

**Step 4: Run tests**

```bash
uv run pytest tests/test_categorizer.py -v
```

Expected: All PASS

**Step 5: Commit**

```bash
git add src/bookkeeper/categorizer.py tests/test_categorizer.py
git commit -m "Add rules-based categorization engine"
```

---

### Task 11: Categorize CLI Command + Auto-categorize on Import

**Files:**
- Modify: `src/bookkeeper/cli.py`
- Modify: `tests/test_cli.py`

**Step 1: Write failing test**

Append to `tests/test_cli.py`:
```python
def test_categorize_command(tmp_path, monkeypatch):
    data_dir = tmp_path / "bookkeeper"
    monkeypatch.setenv("BOOKKEEPER_DATA_DIR", str(data_dir))
    runner.invoke(app, ["init"])
    runner.invoke(app, ["accounts", "add", "BofA Checking", "--type", "checking"])
    runner.invoke(app, ["import", str(FIXTURES / "bofa_checking_sample.csv"), "--account", "BofA Checking"])

    result = runner.invoke(app, ["categorize"])
    assert result.exit_code == 0
    assert "categorized" in result.output.lower() or "flagged" in result.output.lower()
```

**Step 2: Implement categorize command and hook into import**

Add to `src/bookkeeper/cli.py`:
```python
from bookkeeper.categorizer import categorize_transactions


@app.command()
def categorize():
    """Re-run categorization rules on uncategorized transactions."""
    conn = get_connection(get_db_path())
    result = categorize_transactions(conn)
    conn.close()
    typer.echo(f"{result['categorized']} categorized, {result['still_flagged']} still flagged")
```

Update the `import_cmd` to auto-categorize after import:
```python
@app.command("import")
def import_cmd(
    file: Path = typer.Argument(help="Path to CSV or XLSX file to import"),
    account: str = typer.Option(help="Account name to import into"),
):
    """Import a CSV/XLSX file and auto-categorize transactions."""
    conn = get_connection(get_db_path())
    result = import_file(conn, file, account)

    if result.get("duplicate_file"):
        conn.close()
        typer.echo("This file has already been imported (duplicate checksum).")
        return

    typer.echo(f"{result['imported']} imported, {result['skipped']} skipped (duplicates)")

    # Auto-categorize
    cat_result = categorize_transactions(conn)
    typer.echo(f"{cat_result['categorized']} categorized, {cat_result['still_flagged']} still flagged")
    conn.close()

    # Archive the import file
    dest = get_data_dir() / "imports" / file.name
    if not dest.exists():
        import shutil
        shutil.copy2(file, dest)
```

**Step 3: Run tests**

```bash
uv run pytest tests/test_cli.py -v
```

Expected: All PASS

**Step 4: Commit**

```bash
git add src/bookkeeper/cli.py tests/test_cli.py
git commit -m "Add categorize command, auto-categorize on import"
```

---

### Task 12: Rules Management CLI

**Files:**
- Modify: `src/bookkeeper/cli.py`
- Modify: `tests/test_cli.py`

**Step 1: Write failing tests**

Append to `tests/test_cli.py`:
```python
def test_rules_add_and_list(tmp_path, monkeypatch):
    data_dir = tmp_path / "bookkeeper"
    monkeypatch.setenv("BOOKKEEPER_DATA_DIR", str(data_dir))
    runner.invoke(app, ["init"])

    result = runner.invoke(
        app,
        ["rules", "add", "ADOBE", "--category", "Software & Subscriptions", "--vendor", "Adobe"],
    )
    assert result.exit_code == 0

    result = runner.invoke(app, ["rules", "list"])
    assert result.exit_code == 0
    assert "ADOBE" in result.output
```

**Step 2: Implement rules subcommands**

Add to `src/bookkeeper/cli.py`:
```python
rules_app = typer.Typer(help="Manage categorization rules.")
app.add_typer(rules_app, name="rules")


@rules_app.command("add")
def rules_add(
    pattern: str = typer.Argument(help="Pattern to match against transaction descriptions"),
    category: str = typer.Option(help="Category name to assign"),
    vendor: str = typer.Option(None, help="Normalized vendor name"),
    match_type: str = typer.Option("contains", help="Match type: contains, starts_with, regex"),
    priority: int = typer.Option(0, help="Rule priority (higher wins)"),
):
    """Add a categorization rule."""
    conn = get_connection(get_db_path())
    cat = conn.execute("SELECT id FROM categories WHERE name = ?", (category,)).fetchone()
    if cat is None:
        typer.echo(f"Unknown category: {category}")
        raise typer.Exit(1)
    conn.execute(
        "INSERT INTO rules (pattern, match_type, vendor, category_id, priority) VALUES (?, ?, ?, ?, ?)",
        (pattern, match_type, vendor, cat["id"], priority),
    )
    conn.commit()
    conn.close()
    typer.echo(f"Added rule: '{pattern}' → {category}")


@rules_app.command("list")
def rules_list():
    """List all categorization rules."""
    conn = get_connection(get_db_path())
    rows = conn.execute(
        "SELECT r.id, r.pattern, r.match_type, r.vendor, c.name as category, r.priority, r.hit_count "
        "FROM rules r JOIN categories c ON r.category_id = c.id "
        "WHERE r.is_active = 1 ORDER BY r.priority DESC"
    ).fetchall()
    conn.close()

    table = Table(title="Rules")
    table.add_column("ID", style="dim")
    table.add_column("Pattern")
    table.add_column("Type")
    table.add_column("Vendor")
    table.add_column("Category")
    table.add_column("Priority")
    table.add_column("Hits")
    for row in rows:
        table.add_row(
            str(row["id"]), row["pattern"], row["match_type"],
            row["vendor"] or "", row["category"],
            str(row["priority"]), str(row["hit_count"]),
        )
    console.print(table)
```

**Step 3: Run tests**

```bash
uv run pytest tests/test_cli.py -v
```

Expected: All PASS

**Step 4: Commit**

```bash
git add src/bookkeeper/cli.py tests/test_cli.py
git commit -m "Add rules add and list commands"
```

---

### Task 13: Interactive Review Flow

**Files:**
- Create: `src/bookkeeper/reviewer.py`
- Create: `tests/test_reviewer.py`

**Step 1: Write failing tests**

`tests/test_reviewer.py`:
```python
from unittest.mock import patch

from bookkeeper.reviewer import get_flagged_transactions, apply_review


def test_get_flagged_transactions(db):
    db.execute("INSERT INTO accounts (name, account_type) VALUES ('Test', 'checking')")
    db.execute(
        "INSERT INTO transactions (account_id, date, description, amount, is_flagged, flag_reason) "
        "VALUES (1, '2025-03-10', 'MYSTERY VENDOR', -100.00, 1, 'No matching rule')",
    )
    db.commit()

    flagged = get_flagged_transactions(db)
    assert len(flagged) == 1
    assert flagged[0]["description"] == "MYSTERY VENDOR"


def test_apply_review_categorizes_transaction(db):
    db.execute("INSERT INTO accounts (name, account_type) VALUES ('Test', 'checking')")
    cat_id = db.execute("SELECT id FROM categories WHERE name = 'Office Expense'").fetchone()["id"]
    db.execute(
        "INSERT INTO transactions (account_id, date, description, amount, is_flagged, flag_reason) "
        "VALUES (1, '2025-03-10', 'STAPLES STORE', -45.00, 1, 'No matching rule')",
    )
    db.commit()

    apply_review(db, transaction_id=1, category_id=cat_id, vendor="Staples")

    txn = db.execute("SELECT * FROM transactions WHERE id = 1").fetchone()
    assert txn["category_id"] == cat_id
    assert txn["vendor"] == "Staples"
    assert txn["is_flagged"] == 0


def test_apply_review_with_rule_creation(db):
    db.execute("INSERT INTO accounts (name, account_type) VALUES ('Test', 'checking')")
    cat_id = db.execute("SELECT id FROM categories WHERE name = 'Office Expense'").fetchone()["id"]
    db.execute(
        "INSERT INTO transactions (account_id, date, description, amount, is_flagged, flag_reason) "
        "VALUES (1, '2025-03-10', 'STAPLES STORE 1234', -45.00, 1, 'No matching rule')",
    )
    db.commit()

    apply_review(db, transaction_id=1, category_id=cat_id, vendor="Staples",
                 create_rule=True, rule_pattern="STAPLES")

    rule = db.execute("SELECT * FROM rules WHERE pattern = 'STAPLES'").fetchone()
    assert rule is not None
    assert rule["category_id"] == cat_id
    assert rule["vendor"] == "Staples"
```

**Step 2: Run tests to verify they fail**

```bash
uv run pytest tests/test_reviewer.py -v
```

**Step 3: Implement reviewer.py**

`src/bookkeeper/reviewer.py`:
```python
import sqlite3

from rich.console import Console
from rich.prompt import Confirm, Prompt
from rich.rule import Rule
from rich.table import Table

console = Console()


def get_flagged_transactions(conn: sqlite3.Connection) -> list:
    return conn.execute(
        "SELECT t.id, t.date, t.description, t.amount, a.name as account_name "
        "FROM transactions t JOIN accounts a ON t.account_id = a.id "
        "WHERE t.is_flagged = 1 ORDER BY t.date"
    ).fetchall()


def get_categories(conn: sqlite3.Connection) -> list:
    return conn.execute(
        "SELECT id, name, category_type FROM categories WHERE is_active = 1 ORDER BY category_type, name"
    ).fetchall()


def apply_review(
    conn: sqlite3.Connection,
    transaction_id: int,
    category_id: int,
    vendor: str | None = None,
    create_rule: bool = False,
    rule_pattern: str | None = None,
) -> None:
    """Apply a review decision to a transaction."""
    conn.execute(
        "UPDATE transactions SET category_id = ?, vendor = ?, is_flagged = 0, flag_reason = NULL WHERE id = ?",
        (category_id, vendor, transaction_id),
    )
    if create_rule and rule_pattern:
        conn.execute(
            "INSERT INTO rules (pattern, match_type, vendor, category_id) VALUES (?, 'contains', ?, ?)",
            (rule_pattern, vendor, category_id),
        )
    conn.commit()


def run_review(conn: sqlite3.Connection) -> None:
    """Interactive review loop for flagged transactions."""
    flagged = get_flagged_transactions(conn)
    if not flagged:
        console.print("[green]No flagged transactions to review.[/green]")
        return

    categories = get_categories(conn)
    console.print(f"\n[bold]{len(flagged)} transactions to review[/bold]\n")

    # Print category list for reference
    cat_table = Table(title="Categories", show_lines=False)
    cat_table.add_column("#", style="dim")
    cat_table.add_column("Name")
    cat_table.add_column("Type", style="dim")
    for i, cat in enumerate(categories, 1):
        cat_table.add_row(str(i), cat["name"], cat["category_type"])
    console.print(cat_table)
    console.print()

    for txn in flagged:
        console.print(Rule())
        console.print(f"  [bold]Date:[/bold]        {txn['date']}")
        console.print(f"  [bold]Description:[/bold] {txn['description']}")
        amount = txn["amount"]
        color = "red" if amount < 0 else "green"
        console.print(f"  [bold]Amount:[/bold]      [{color}]${abs(amount):,.2f}[/{color}]")
        console.print(f"  [bold]Account:[/bold]     {txn['account_name']}")
        console.print()

        choice = Prompt.ask(
            "Category # (or [bold]s[/bold]kip, [bold]q[/bold]uit)",
        )

        if choice.lower() == "q":
            console.print("[yellow]Review paused.[/yellow]")
            return
        if choice.lower() == "s":
            continue

        try:
            idx = int(choice) - 1
            cat = categories[idx]
        except (ValueError, IndexError):
            console.print("[red]Invalid choice, skipping.[/red]")
            continue

        vendor = Prompt.ask("Vendor name (or Enter to skip)", default="")
        vendor = vendor if vendor else None

        create_rule = Confirm.ask(f"Create rule for future matches?", default=False)
        rule_pattern = None
        if create_rule:
            # Suggest first two words of description as pattern
            words = txn["description"].split()
            suggested = " ".join(words[:2]) if len(words) >= 2 else words[0]
            rule_pattern = Prompt.ask("Rule pattern", default=suggested)

        apply_review(
            conn,
            transaction_id=txn["id"],
            category_id=cat["id"],
            vendor=vendor,
            create_rule=create_rule,
            rule_pattern=rule_pattern,
        )
        console.print(f"[green]→ Categorized as {cat['name']}[/green]\n")

    console.print("[green]Review complete![/green]")
```

**Step 4: Run tests**

```bash
uv run pytest tests/test_reviewer.py -v
```

Expected: All PASS

**Step 5: Add review CLI command**

Add to `src/bookkeeper/cli.py`:
```python
from bookkeeper.reviewer import run_review


@app.command()
def review():
    """Interactively review flagged transactions."""
    conn = get_connection(get_db_path())
    run_review(conn)
    conn.close()
```

**Step 6: Commit**

```bash
git add src/bookkeeper/reviewer.py tests/test_reviewer.py src/bookkeeper/cli.py
git commit -m "Add interactive review flow for flagged transactions"
```

---

### Task 14: Reports — P&L and Expense Breakdown

**Files:**
- Create: `src/bookkeeper/reports.py`
- Create: `tests/test_reports.py`

**Step 1: Write failing tests**

`tests/test_reports.py`:
```python
from datetime import date

from bookkeeper.reports import get_pnl, get_expense_breakdown


def _seed_transactions(db):
    db.execute("INSERT INTO accounts (name, account_type) VALUES ('Test', 'checking')")
    income_cat = db.execute("SELECT id FROM categories WHERE name = 'Client Services'").fetchone()["id"]
    software_cat = db.execute("SELECT id FROM categories WHERE name = 'Software & Subscriptions'").fetchone()["id"]
    meals_cat = db.execute("SELECT id FROM categories WHERE name = 'Meals'").fetchone()["id"]

    db.execute(
        "INSERT INTO transactions (account_id, date, description, amount, category_id, vendor) "
        "VALUES (1, '2025-03-01', 'Client payment', 5000.00, ?, 'Client A')",
        (income_cat,),
    )
    db.execute(
        "INSERT INTO transactions (account_id, date, description, amount, category_id, vendor) "
        "VALUES (1, '2025-03-10', 'Adobe', -54.43, ?, 'Adobe')",
        (software_cat,),
    )
    db.execute(
        "INSERT INTO transactions (account_id, date, description, amount, category_id, vendor) "
        "VALUES (1, '2025-03-15', 'Lunch meeting', -45.00, ?, 'Restaurant')",
        (meals_cat,),
    )
    db.execute(
        "INSERT INTO transactions (account_id, date, description, amount, category_id, vendor) "
        "VALUES (1, '2025-04-01', 'Client payment', 6000.00, ?, 'Client B')",
        (income_cat,),
    )
    db.commit()


def test_pnl_ytd(db):
    _seed_transactions(db)
    pnl = get_pnl(db, year=2025)
    assert pnl["total_income"] == 11000.00
    assert pnl["total_expenses"] == -99.43
    assert pnl["net"] == 11000.00 - 99.43


def test_pnl_by_month(db):
    _seed_transactions(db)
    pnl = get_pnl(db, year=2025, month=3)
    assert pnl["total_income"] == 5000.00
    assert pnl["total_expenses"] == -99.43


def test_expense_breakdown(db):
    _seed_transactions(db)
    breakdown = get_expense_breakdown(db, year=2025)
    # Should have 2 expense categories
    assert len(breakdown["categories"]) == 2
    # Software should be in there
    names = [c["name"] for c in breakdown["categories"]]
    assert "Software & Subscriptions" in names
    assert "Meals" in names
```

**Step 2: Run tests to verify they fail**

```bash
uv run pytest tests/test_reports.py -v
```

**Step 3: Implement reports.py**

`src/bookkeeper/reports.py`:
```python
import csv
import sqlite3
from pathlib import Path

from rich.console import Console
from rich.table import Table

console = Console()


def _date_filter(year: int | None = None, month: int | None = None,
                 from_date: str | None = None, to_date: str | None = None) -> tuple[str, list]:
    """Build a WHERE clause fragment for date filtering."""
    if from_date and to_date:
        return "t.date BETWEEN ? AND ?", [from_date, to_date]
    if year and month:
        prefix = f"{year}-{month:02d}"
        return "t.date LIKE ?", [f"{prefix}%"]
    if year:
        return "t.date LIKE ?", [f"{year}%"]
    # Default: current year
    from datetime import date
    return "t.date LIKE ?", [f"{date.today().year}%"]


def get_pnl(conn: sqlite3.Connection, year: int | None = None, month: int | None = None,
            from_date: str | None = None, to_date: str | None = None) -> dict:
    date_clause, params = _date_filter(year, month, from_date, to_date)

    income_rows = conn.execute(
        f"SELECT c.name, SUM(t.amount) as total "
        f"FROM transactions t JOIN categories c ON t.category_id = c.id "
        f"WHERE {date_clause} AND c.category_type = 'income' "
        f"GROUP BY c.name ORDER BY total DESC",
        params,
    ).fetchall()

    expense_rows = conn.execute(
        f"SELECT c.name, SUM(t.amount) as total "
        f"FROM transactions t JOIN categories c ON t.category_id = c.id "
        f"WHERE {date_clause} AND c.category_type = 'expense' "
        f"GROUP BY c.name ORDER BY total ASC",
        params,
    ).fetchall()

    total_income = sum(r["total"] for r in income_rows)
    total_expenses = sum(r["total"] for r in expense_rows)

    return {
        "income": [{"name": r["name"], "total": r["total"]} for r in income_rows],
        "expenses": [{"name": r["name"], "total": r["total"]} for r in expense_rows],
        "total_income": total_income,
        "total_expenses": total_expenses,
        "net": total_income + total_expenses,  # expenses are negative
    }


def get_expense_breakdown(conn: sqlite3.Connection, year: int | None = None, month: int | None = None,
                          from_date: str | None = None, to_date: str | None = None) -> dict:
    date_clause, params = _date_filter(year, month, from_date, to_date)

    rows = conn.execute(
        f"SELECT c.name, SUM(t.amount) as total, COUNT(*) as count "
        f"FROM transactions t JOIN categories c ON t.category_id = c.id "
        f"WHERE {date_clause} AND c.category_type = 'expense' "
        f"GROUP BY c.name ORDER BY total ASC",
        params,
    ).fetchall()

    total = sum(r["total"] for r in rows)

    categories = []
    for r in rows:
        pct = (r["total"] / total * 100) if total != 0 else 0
        categories.append({"name": r["name"], "total": r["total"], "count": r["count"], "pct": pct})

    # Top vendors
    vendors = conn.execute(
        f"SELECT t.vendor, SUM(t.amount) as total, COUNT(*) as count "
        f"FROM transactions t JOIN categories c ON t.category_id = c.id "
        f"WHERE {date_clause} AND c.category_type = 'expense' AND t.vendor IS NOT NULL "
        f"GROUP BY t.vendor ORDER BY total ASC LIMIT 10",
        params,
    ).fetchall()

    return {
        "categories": categories,
        "total": total,
        "top_vendors": [{"vendor": v["vendor"], "total": v["total"], "count": v["count"]} for v in vendors],
    }


def get_tax_summary(conn: sqlite3.Connection, year: int | None = None) -> dict:
    date_clause, params = _date_filter(year)

    rows = conn.execute(
        f"SELECT c.name, c.tax_line, c.category_type, SUM(t.amount) as total "
        f"FROM transactions t JOIN categories c ON t.category_id = c.id "
        f"WHERE {date_clause} "
        f"GROUP BY c.name, c.tax_line, c.category_type "
        f"ORDER BY c.category_type DESC, c.tax_line",
        params,
    ).fetchall()

    return {
        "line_items": [
            {"name": r["name"], "tax_line": r["tax_line"], "type": r["category_type"], "total": r["total"]}
            for r in rows
        ]
    }


def get_cashflow(conn: sqlite3.Connection, year: int | None = None) -> dict:
    date_clause, params = _date_filter(year)

    rows = conn.execute(
        f"SELECT substr(t.date, 1, 7) as month, "
        f"SUM(CASE WHEN t.amount > 0 THEN t.amount ELSE 0 END) as inflows, "
        f"SUM(CASE WHEN t.amount < 0 THEN t.amount ELSE 0 END) as outflows "
        f"FROM transactions t WHERE {date_clause} "
        f"GROUP BY substr(t.date, 1, 7) ORDER BY month",
        params,
    ).fetchall()

    months = []
    running = 0.0
    for r in rows:
        running += r["inflows"] + r["outflows"]
        months.append({
            "month": r["month"],
            "inflows": r["inflows"],
            "outflows": r["outflows"],
            "net": r["inflows"] + r["outflows"],
            "running_balance": running,
        })
    return {"months": months}


def get_flagged(conn: sqlite3.Connection) -> list:
    return conn.execute(
        "SELECT t.id, t.date, t.description, t.amount, a.name as account_name, t.flag_reason "
        "FROM transactions t JOIN accounts a ON t.account_id = a.id "
        "WHERE t.is_flagged = 1 ORDER BY t.date"
    ).fetchall()


def get_balance(conn: sqlite3.Connection) -> dict:
    accounts = conn.execute(
        "SELECT a.id, a.name, a.account_type, COALESCE(SUM(t.amount), 0) as balance "
        "FROM accounts a LEFT JOIN transactions t ON a.id = t.account_id "
        "GROUP BY a.id ORDER BY a.name"
    ).fetchall()

    total = sum(a["balance"] for a in accounts)

    # YTD net income
    from datetime import date as date_cls
    year = date_cls.today().year
    ytd = conn.execute(
        "SELECT COALESCE(SUM(amount), 0) as net FROM transactions WHERE date LIKE ?",
        (f"{year}%",),
    ).fetchone()

    return {
        "accounts": [{"name": a["name"], "type": a["account_type"], "balance": a["balance"]} for a in accounts],
        "total": total,
        "ytd_net_income": ytd["net"],
    }
```

**Step 4: Run tests**

```bash
uv run pytest tests/test_reports.py -v
```

Expected: All PASS

**Step 5: Commit**

```bash
git add src/bookkeeper/reports.py tests/test_reports.py
git commit -m "Add report functions: P&L, expense breakdown, tax, cashflow, flagged, balance"
```

---

### Task 15: Report CLI Commands

**Files:**
- Modify: `src/bookkeeper/cli.py`

**Step 1: Add report subcommands**

Add to `src/bookkeeper/cli.py`:
```python
from bookkeeper.reports import (
    get_pnl, get_expense_breakdown, get_tax_summary,
    get_cashflow, get_flagged, get_balance,
)

report_app = typer.Typer(help="Generate reports.")
app.add_typer(report_app, name="report")


def _parse_date_opts(month: str | None, year: int | None, from_date: str | None, to_date: str | None) -> dict:
    if from_date and to_date:
        return {"from_date": from_date, "to_date": to_date}
    if month:
        parts = month.split("-")
        return {"year": int(parts[0]), "month": int(parts[1])}
    if year:
        return {"year": year}
    return {}


@report_app.command("pnl")
def report_pnl(
    month: str = typer.Option(None, help="Month filter: YYYY-MM"),
    year: int = typer.Option(None, help="Year filter: YYYY"),
    from_date: str = typer.Option(None, "--from", help="Start date: YYYY-MM-DD"),
    to_date: str = typer.Option(None, "--to", help="End date: YYYY-MM-DD"),
):
    """Profit & Loss report."""
    conn = get_connection(get_db_path())
    pnl = get_pnl(conn, **_parse_date_opts(month, year, from_date, to_date))
    conn.close()

    table = Table(title="Profit & Loss")
    table.add_column("Category")
    table.add_column("Amount", justify="right")

    if pnl["income"]:
        table.add_row("[bold green]INCOME[/bold green]", "")
        for item in pnl["income"]:
            table.add_row(f"  {item['name']}", f"${item['total']:,.2f}")
        table.add_row("[bold]Total Income[/bold]", f"[bold]${pnl['total_income']:,.2f}[/bold]")
        table.add_row("", "")

    if pnl["expenses"]:
        table.add_row("[bold red]EXPENSES[/bold red]", "")
        for item in pnl["expenses"]:
            table.add_row(f"  {item['name']}", f"${abs(item['total']):,.2f}")
        table.add_row("[bold]Total Expenses[/bold]", f"[bold]${abs(pnl['total_expenses']):,.2f}[/bold]")
        table.add_row("", "")

    color = "green" if pnl["net"] >= 0 else "red"
    table.add_row(f"[bold {color}]NET[/bold {color}]", f"[bold {color}]${pnl['net']:,.2f}[/bold {color}]")
    console.print(table)


@report_app.command("expenses")
def report_expenses(
    month: str = typer.Option(None, help="Month filter: YYYY-MM"),
    year: int = typer.Option(None, help="Year filter: YYYY"),
):
    """Expense breakdown report."""
    conn = get_connection(get_db_path())
    data = get_expense_breakdown(conn, **_parse_date_opts(month, year, None, None))
    conn.close()

    table = Table(title="Expense Breakdown")
    table.add_column("Category")
    table.add_column("Amount", justify="right")
    table.add_column("%", justify="right")
    table.add_column("Count", justify="right")
    for item in data["categories"]:
        table.add_row(item["name"], f"${abs(item['total']):,.2f}", f"{item['pct']:.1f}%", str(item["count"]))
    table.add_row("[bold]Total[/bold]", f"[bold]${abs(data['total']):,.2f}[/bold]", "", "")
    console.print(table)

    if data["top_vendors"]:
        vtable = Table(title="Top Vendors")
        vtable.add_column("Vendor")
        vtable.add_column("Amount", justify="right")
        vtable.add_column("Count", justify="right")
        for v in data["top_vendors"]:
            vtable.add_row(v["vendor"], f"${abs(v['total']):,.2f}", str(v["count"]))
        console.print(vtable)


@report_app.command("tax")
def report_tax(year: int = typer.Option(None, help="Year filter: YYYY")):
    """Tax summary organized by IRS line items."""
    conn = get_connection(get_db_path())
    data = get_tax_summary(conn, year=year)
    conn.close()

    table = Table(title="Tax Summary")
    table.add_column("Category")
    table.add_column("Tax Line")
    table.add_column("Type")
    table.add_column("Amount", justify="right")
    for item in data["line_items"]:
        table.add_row(item["name"], item["tax_line"] or "", item["type"], f"${abs(item['total']):,.2f}")
    console.print(table)


@report_app.command("cashflow")
def report_cashflow(
    month: str = typer.Option(None, help="Month filter: YYYY-MM"),
    year: int = typer.Option(None, help="Year filter: YYYY"),
):
    """Cash flow report with monthly inflows/outflows."""
    conn = get_connection(get_db_path())
    data = get_cashflow(conn, **_parse_date_opts(month, year, None, None))
    conn.close()

    table = Table(title="Cash Flow")
    table.add_column("Month")
    table.add_column("Inflows", justify="right", style="green")
    table.add_column("Outflows", justify="right", style="red")
    table.add_column("Net", justify="right")
    table.add_column("Running", justify="right")
    for m in data["months"]:
        net_color = "green" if m["net"] >= 0 else "red"
        table.add_row(
            m["month"],
            f"${m['inflows']:,.2f}",
            f"${abs(m['outflows']):,.2f}",
            f"[{net_color}]${m['net']:,.2f}[/{net_color}]",
            f"${m['running_balance']:,.2f}",
        )
    console.print(table)


@report_app.command("flagged")
def report_flagged():
    """Show all flagged/uncategorized transactions."""
    conn = get_connection(get_db_path())
    rows = get_flagged(conn)
    conn.close()

    if not rows:
        typer.echo("No flagged transactions.")
        return

    table = Table(title=f"Flagged Transactions ({len(rows)})")
    table.add_column("ID", style="dim")
    table.add_column("Date")
    table.add_column("Description")
    table.add_column("Amount", justify="right")
    table.add_column("Account")
    for r in rows:
        color = "red" if r["amount"] < 0 else "green"
        table.add_row(str(r["id"]), r["date"], r["description"], f"[{color}]${abs(r['amount']):,.2f}[/{color}]", r["account_name"])
    console.print(table)


@report_app.command("balance")
def report_balance():
    """Cash position snapshot."""
    conn = get_connection(get_db_path())
    data = get_balance(conn)
    conn.close()

    table = Table(title="Cash Position")
    table.add_column("Account")
    table.add_column("Type", style="dim")
    table.add_column("Balance", justify="right")
    for a in data["accounts"]:
        color = "green" if a["balance"] >= 0 else "red"
        table.add_row(a["name"], a["type"], f"[{color}]${a['balance']:,.2f}[/{color}]")
    table.add_row("[bold]Total[/bold]", "", f"[bold]${data['total']:,.2f}[/bold]")
    console.print(table)
    console.print(f"\nYTD Net Income: ${data['ytd_net_income']:,.2f}")
```

**Step 2: Verify CLI help works**

```bash
uv run bookkeeper report --help
```

Expected: Shows all report subcommands

**Step 3: Commit**

```bash
git add src/bookkeeper/cli.py
git commit -m "Add all report CLI commands with rich table output"
```

---

### Task 16: Reconciliation

**Files:**
- Create: `src/bookkeeper/reconciler.py`
- Create: `tests/test_reconciler.py`
- Modify: `src/bookkeeper/cli.py`

**Step 1: Write failing tests**

`tests/test_reconciler.py`:
```python
from bookkeeper.reconciler import reconcile


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
```

**Step 2: Run tests to verify they fail**

```bash
uv run pytest tests/test_reconciler.py -v
```

**Step 3: Implement reconciler.py**

`src/bookkeeper/reconciler.py`:
```python
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
```

**Step 4: Run tests**

```bash
uv run pytest tests/test_reconciler.py -v
```

Expected: All PASS

**Step 5: Add reconcile CLI command**

Add to `src/bookkeeper/cli.py`:
```python
from bookkeeper.reconciler import reconcile


@app.command()
def reconcile_cmd(
    account: str = typer.Argument(help="Account name"),
    month: str = typer.Option(help="Month: YYYY-MM"),
    balance: float = typer.Option(help="Statement ending balance"),
):
    """Reconcile an account against a statement balance."""
    conn = get_connection(get_db_path())
    result = reconcile(conn, account_name=account, month=month, statement_balance=balance)
    conn.close()

    if result["is_reconciled"]:
        typer.echo(f"Reconciled! Calculated: ${result['calculated_balance']:,.2f}")
    else:
        typer.echo(
            f"DISCREPANCY: ${result['discrepancy']:,.2f}\n"
            f"  Statement:  ${result['statement_balance']:,.2f}\n"
            f"  Calculated: ${result['calculated_balance']:,.2f}"
        )
```

Note: Typer name must avoid collision with the imported `reconcile` function — name the command function `reconcile_cmd` and Typer will use the function name minus `_cmd` as the command name, or explicitly register it.

**Step 6: Run all tests**

```bash
uv run pytest -v
```

Expected: All PASS

**Step 7: Commit**

```bash
git add src/bookkeeper/reconciler.py tests/test_reconciler.py src/bookkeeper/cli.py
git commit -m "Add monthly reconciliation with discrepancy detection"
```

---

### Task 17: Gusto Payroll Auto-Categorization

**Files:**
- Modify: `src/bookkeeper/importer.py`

Payroll transactions from Gusto should be pre-categorized on import since we know exactly what they are.

**Step 1: Write failing test**

Append to `tests/test_importer.py`:
```python
def test_gusto_import_auto_categorizes(db, tmp_path):
    fixture = tmp_path / "gusto_sample.xlsx"
    _create_gusto_fixture(fixture)

    db.execute(
        "INSERT INTO accounts (name, account_type) VALUES (?, ?)",
        ("Gusto", "payroll"),
    )
    db.commit()

    import_file(db, fixture, "Gusto")

    # Wages should be auto-categorized
    wages_txn = db.execute(
        "SELECT t.*, c.name as cat_name FROM transactions t "
        "LEFT JOIN categories c ON t.category_id = c.id "
        "WHERE t.description LIKE '%Wages%'"
    ).fetchone()
    assert wages_txn["cat_name"] == "Payroll — Wages"
    assert wages_txn["is_flagged"] == 0

    # Employer taxes should be auto-categorized
    tax_txn = db.execute(
        "SELECT t.*, c.name as cat_name FROM transactions t "
        "LEFT JOIN categories c ON t.category_id = c.id "
        "WHERE t.description LIKE '%Taxes%'"
    ).fetchone()
    assert tax_txn["cat_name"] == "Payroll — Taxes"
    assert tax_txn["is_flagged"] == 0
```

**Step 2: Run test to verify it fails**

```bash
uv run pytest tests/test_importer.py::test_gusto_import_auto_categorizes -v
```

**Step 3: Update importer to pre-categorize payroll transactions**

Modify `import_file` in `src/bookkeeper/importer.py` to look up payroll categories and assign them during insert for payroll account types:

```python
def import_file(
    conn: sqlite3.Connection,
    file_path: Path,
    account_name: str,
) -> dict:
    # ... existing account lookup and checksum check ...

    # For payroll imports, look up categories for auto-assignment
    payroll_categories = {}
    if account_type == "payroll":
        for cat_name in ("Payroll — Wages", "Payroll — Taxes", "Payroll — Benefits"):
            row = conn.execute("SELECT id FROM categories WHERE name = ?", (cat_name,)).fetchone()
            if row:
                payroll_categories[cat_name] = row["id"]

    # ... parse rows ...

    imported = 0
    skipped = 0
    for row in parsed_rows:
        if _is_duplicate_row(conn, account_id, row):
            skipped += 1
            continue

        # Auto-categorize payroll transactions
        category_id = None
        is_flagged = 1
        flag_reason = "No matching rule"
        if account_type == "payroll":
            if "Wages" in row.description:
                category_id = payroll_categories.get("Payroll — Wages")
            elif "Taxes" in row.description:
                category_id = payroll_categories.get("Payroll — Taxes")
            elif "Benefits" in row.description:
                category_id = payroll_categories.get("Payroll — Benefits")
            if category_id:
                is_flagged = 0
                flag_reason = None

        conn.execute(
            "INSERT INTO transactions (account_id, date, description, amount, category_id, is_flagged, flag_reason) "
            "VALUES (?, ?, ?, ?, ?, ?, ?)",
            (account_id, row.date, row.description, row.amount, category_id, is_flagged, flag_reason),
        )
        imported += 1

    # ... rest of import_file stays the same ...
```

**Step 4: Run tests**

```bash
uv run pytest tests/test_importer.py -v
```

Expected: All PASS

**Step 5: Commit**

```bash
git add src/bookkeeper/importer.py tests/test_importer.py
git commit -m "Auto-categorize Gusto payroll transactions on import"
```

---

### Task 18: End-to-End Smoke Test with Real Data

**Files:**
- No new files — manual verification

**Step 1: Run all tests**

```bash
uv run pytest -v
```

Expected: All PASS

**Step 2: Initialize and set up accounts**

```bash
uv run bookkeeper init
uv run bookkeeper accounts add "BofA Checking" --type checking --institution "Bank of America"
uv run bookkeeper accounts add "BofA Credit Card" --type credit_card --institution "Bank of America"
uv run bookkeeper accounts add "BofA Line of Credit" --type line_of_credit --institution "Bank of America"
uv run bookkeeper accounts add "Gusto Payroll" --type payroll --institution "Gusto"
uv run bookkeeper accounts list
```

**Step 3: Import real files**

```bash
uv run bookkeeper import ~/Documents/bookkeeper/import/bofa\ checking.csv --account "BofA Checking"
uv run bookkeeper import ~/Documents/bookkeeper/import/bofa\ creditcard.csv --account "BofA Credit Card"
uv run bookkeeper import ~/Documents/bookkeeper/import/bofa\ lineofcredit.csv --account "BofA Line of Credit"
uv run bookkeeper import ~/Documents/bookkeeper/import/payroll_data_export_raygun-design-llc_2025_2026-02-26T15_00_33-08_00.xlsx.xlsx --account "Gusto Payroll"
uv run bookkeeper import ~/Documents/bookkeeper/import/payroll_data_export_raygun-design-llc_2026_2026-02-26T14_59_36-08_00.xlsx.xlsx --account "Gusto Payroll"
```

Verify: Each import reports counts, no crashes.

**Step 4: Check reports**

```bash
uv run bookkeeper report flagged
uv run bookkeeper report balance
```

**Step 5: Fix any parser issues found with real data**

Address edge cases that the sample fixtures didn't cover. This is expected — real BofA CSVs have quirks.

**Step 6: Commit any fixes**

```bash
git add -u
git commit -m "Fix parser edge cases from real data import"
```

---

### Task 19: Update CLAUDE.md

**Files:**
- Modify: `CLAUDE.md`

Update with actual commands now that the project is built:

```markdown
## Commands

```bash
uv sync                    # Install dependencies
uv run pytest -v           # Run all tests
uv run pytest tests/test_importer.py -v          # Run single test file
uv run pytest tests/test_importer.py::test_name  # Run single test
uv run bookkeeper --help   # CLI help
uv run bookkeeper init     # Initialize database
```
```

**Step 1: Commit**

```bash
git add CLAUDE.md
git commit -m "Update CLAUDE.md with development commands"
```
