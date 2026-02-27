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
