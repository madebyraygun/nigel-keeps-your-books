# Raygun Bookkeeper — System Plan

A lightweight, Claude-powered bookkeeping system for Raygun, replacing QuickBooks with bank CSV imports, smart categorization, and SQLite storage.

---

## Overview

This system replaces QuickBooks Online for a small services firm (Raygun) with no inventory, no complex payroll, and cash-basis accounting. The core idea: every transaction that matters appears on a bank or credit card statement. Ingest those statements, categorize transactions intelligently, store everything in SQLite, and generate reports on demand.

### Design Principles

- **Cash-basis accounting.** Income is recorded when received, expenses when paid. No accruals, no accounts receivable/payable tracking.
- **Single-entry with tax-aligned categories.** Every transaction gets one category that maps cleanly to IRS Schedule C (or 1120-S) line items.
- **Rules-based categorization with human review.** Known vendors are auto-categorized. Unknown transactions are flagged. Corrections become new rules.
- **Local-first.** SQLite database on your machine. No third-party access to financial data. Back up however you back up everything else.
- **Tax-time ready.** At any point, generate a summary of income and deductions organized by tax category.

---

## Data Sources

### Bank of America — Checking Account
- **Format:** CSV download from BofA online banking
- **Frequency:** Monthly (or whenever you like)
- **Expected columns:** Date, Description, Amount, Balance
- **Download path:** BofA > Accounts > Download Transactions > CSV

### Bank of America — Credit Card (if applicable)
- **Format:** CSV download
- **Frequency:** Monthly
- **Expected columns:** Date, Description, Amount, Category (BofA's own, ignored)

### Gusto — Payroll
- **Format:** CSV or PDF payroll summary
- **Frequency:** Per pay period (semi-monthly or bi-weekly)
- **Data needed:** Gross payroll, employer taxes, benefits — just the totals, not individual employee detail
- **Note:** Gusto reports may need light manual formatting into a standard CSV. Define a simple template: Date, Description, Amount, Category.

### Manual Entries
- For anything that doesn't come through a bank statement — cash transactions, owner contributions, etc.
- Entered via a simple CSV or directly into the database.

---

## Database Schema

### `accounts`
Represents bank accounts and credit cards.

```sql
CREATE TABLE accounts (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,              -- "BofA Checking", "BofA Credit Card"
    account_type TEXT NOT NULL,      -- "checking", "credit_card", "payroll"
    institution TEXT,                -- "Bank of America", "Gusto"
    last_four TEXT,                  -- last 4 digits of account number
    created_at TEXT DEFAULT (datetime('now'))
);
```

### `categories`
Tax-aligned transaction categories. These map to IRS form line items.

```sql
CREATE TABLE categories (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,              -- "Advertising"
    parent_id INTEGER,              -- for subcategories
    category_type TEXT NOT NULL,     -- "income" or "expense"
    tax_line TEXT,                   -- "Schedule C, Line 8" or "1120-S, Line 19"
    description TEXT,               -- notes on what belongs here
    is_active INTEGER DEFAULT 1,
    FOREIGN KEY (parent_id) REFERENCES categories(id)
);
```

### `transactions`
The master ledger.

```sql
CREATE TABLE transactions (
    id INTEGER PRIMARY KEY,
    account_id INTEGER NOT NULL,
    date TEXT NOT NULL,              -- ISO 8601: "2025-03-15"
    description TEXT NOT NULL,       -- raw description from bank
    amount REAL NOT NULL,            -- negative for expenses, positive for income
    category_id INTEGER,            -- NULL if uncategorized
    vendor TEXT,                     -- cleaned/normalized vendor name
    notes TEXT,                      -- optional user notes
    is_flagged INTEGER DEFAULT 0,   -- 1 if needs human review
    flag_reason TEXT,                -- why it was flagged
    import_id INTEGER,              -- which import batch this came from
    created_at TEXT DEFAULT (datetime('now')),
    FOREIGN KEY (account_id) REFERENCES accounts(id),
    FOREIGN KEY (category_id) REFERENCES categories(id),
    FOREIGN KEY (import_id) REFERENCES imports(id)
);
```

### `rules`
Pattern-matching rules for auto-categorization. Built up over time as you confirm or correct Claude's suggestions.

```sql
CREATE TABLE rules (
    id INTEGER PRIMARY KEY,
    pattern TEXT NOT NULL,           -- substring or regex to match against description
    match_type TEXT DEFAULT 'contains', -- "contains", "starts_with", "regex"
    vendor TEXT,                     -- normalized vendor name to assign
    category_id INTEGER NOT NULL,
    priority INTEGER DEFAULT 0,     -- higher priority rules take precedence
    hit_count INTEGER DEFAULT 0,    -- how many times this rule has matched
    is_active INTEGER DEFAULT 1,
    created_at TEXT DEFAULT (datetime('now')),
    FOREIGN KEY (category_id) REFERENCES categories(id)
);
```

### `imports`
Tracks import batches to prevent duplicates.

```sql
CREATE TABLE imports (
    id INTEGER PRIMARY KEY,
    filename TEXT NOT NULL,
    account_id INTEGER NOT NULL,
    import_date TEXT DEFAULT (datetime('now')),
    record_count INTEGER,
    date_range_start TEXT,
    date_range_end TEXT,
    checksum TEXT,                   -- SHA-256 of file contents for dupe detection
    FOREIGN KEY (account_id) REFERENCES imports(id)
);
```

### `reconciliations`
Tracks monthly reconciliation status.

```sql
CREATE TABLE reconciliations (
    id INTEGER PRIMARY KEY,
    account_id INTEGER NOT NULL,
    month TEXT NOT NULL,             -- "2025-03"
    statement_balance REAL,
    calculated_balance REAL,
    is_reconciled INTEGER DEFAULT 0,
    reconciled_at TEXT,
    notes TEXT,
    FOREIGN KEY (account_id) REFERENCES accounts(id)
);
```

---

## Default Category Taxonomy

Based on IRS Schedule C expense categories, plus income categories relevant to a services firm. Adjust as needed.

### Income
| Category | Tax Line | Description |
|----------|----------|-------------|
| Client Services | Gross receipts | Project fees, retainer payments |
| Hosting & Maintenance | Gross receipts | Recurring client hosting/maintenance fees |
| Reimbursements | Gross receipts | Client reimbursements for expenses |
| Interest Income | Other income | Bank interest |
| Other Income | Other income | Anything else |

### Expenses
| Category | Tax Line (Sched C) | Description |
|----------|-------------------|-------------|
| Advertising & Marketing | Line 8 | Ads, sponsorships, marketing tools |
| Car & Truck | Line 9 | Mileage, fuel, parking (if applicable) |
| Commissions & Fees | Line 10 | Subcontractor commissions, platform fees |
| Contract Labor | Line 11 | Freelancers, subcontractors (1099 work) |
| Insurance | Line 15 | Business insurance, E&O |
| Legal & Professional | Line 17 | Accountant, lawyer, professional services |
| Office Expense | Line 18 | Office supplies, minor equipment |
| Rent / Lease | Line 20b | Office rent, coworking |
| Software & Subscriptions | Line 18/27a | SaaS tools, domain renewals, cloud services |
| Hosting & Infrastructure | Line 18/27a | AWS, server costs, CDN |
| Taxes & Licenses | Line 23 | Business licenses, state fees |
| Travel | Line 24a | Flights, hotels, conference travel |
| Meals | Line 24b | Business meals (50% deductible) |
| Utilities | Line 25 | Internet, phone (business portion) |
| Payroll — Wages | Line 26 | Employee salaries (from Gusto) |
| Payroll — Taxes | Line 23 | Employer payroll taxes (from Gusto) |
| Payroll — Benefits | Line 14 | Health insurance, retirement (from Gusto) |
| Bank & Merchant Fees | Line 27a | Stripe fees, bank charges, wire fees |
| Education & Training | Line 27a | Courses, books, conferences |
| Equipment | Line 13 (depreciation) | Hardware, major purchases |
| Home Office | Line 30 | Simplified method or actual expenses |
| Owner Draw / Distribution | Not deductible | Owner payments, distributions |
| Transfer | Not deductible | Transfers between own accounts |
| Uncategorized | — | Needs review |

---

## Import & Categorization Workflow

### Step 1: Import CSV

```
Input: CSV file + account identifier
Process:
  1. Compute file checksum, check against imports table for duplicates
  2. Parse CSV, normalize date formats and amounts
  3. For each transaction:
     a. Check for exact duplicates (same date, amount, description, account)
     b. Insert into transactions table with import_id
  4. Log import in imports table
Output: Count of new transactions imported, count of duplicates skipped
```

### Step 2: Auto-Categorize

```
Input: All uncategorized transactions (category_id IS NULL)
Process:
  For each transaction:
    1. Run description against rules table (ordered by priority DESC)
    2. If match found:
       - Assign category_id and vendor from rule
       - Increment rule hit_count
    3. If no match found:
       - Flag transaction (is_flagged = 1)
       - Set flag_reason = "No matching rule"
       - Optionally: Claude suggests a category based on description
Output: Count categorized, count flagged for review
```

### Step 3: Human Review

```
Input: All flagged transactions
Process:
  Present each flagged transaction to user with:
    - Date, description, amount
    - Claude's suggested category (if any)
    - Option to: confirm suggestion, choose different category, or skip
  For each confirmed categorization:
    1. Update transaction with category_id and vendor
    2. Ask: "Create a rule for future [vendor] transactions?"
    3. If yes, insert into rules table
Output: Updated transactions, new rules created
```

### Step 4: Reconciliation (Monthly)

```
Input: Account ID, month, statement ending balance
Process:
  1. Sum all transactions for account in that month
  2. Compare calculated balance to statement balance
  3. If they match: mark reconciled
  4. If they don't: flag discrepancy with difference amount
Output: Reconciliation status, discrepancy amount if any
```

---

## Reports

All reports generated from SQLite queries. Claude can produce these on demand in any format — terminal output, markdown, CSV, or formatted for print.

### Profit & Loss (Monthly / YTD / Custom Range)
- Total income by category
- Total expenses by category
- Net profit/loss
- Comparison to prior period (month-over-month or year-over-year)

### Expense Breakdown
- Expenses by category with percentages
- Top vendors by spend
- Month-over-month trend for each category

### Tax Summary (Year-to-Date)
- Income and expenses organized by tax line item
- Ready for Schedule C or 1120-S preparation
- Flags for items needing special treatment (meals at 50%, home office, etc.)

### Cash Flow
- Monthly inflows vs outflows
- Running balance over time
- Projected cash position based on recurring patterns

### Uncategorized / Flagged Transactions
- All transactions needing review
- Aging — how long they've been unflagged

### Reconciliation Status
- Which months are reconciled, which aren't
- Outstanding discrepancies

---

## Claude's Role

Claude operates as the categorization engine and reporting layer. It does NOT directly modify the database without user confirmation for anything financial.

### What Claude Does Automatically
- Parses and imports CSV files
- Applies existing rules to categorize transactions
- Generates reports from the database
- Suggests categories for unknown transactions
- Suggests new rules based on confirmed categorizations
- Detects potential duplicates and anomalies

### What Claude Asks You First
- Confirming category assignments for flagged transactions
- Creating new rules
- Any transaction modification or deletion
- Reconciliation sign-off

### What Claude Never Does
- Deletes transactions without explicit confirmation
- Changes previously confirmed categorizations silently
- Makes tax classification decisions without flagging the reasoning

---

## File Structure

```
raygun-books/
├── data/
│   ├── raygun.db              # SQLite database
│   └── imports/               # Archive of imported CSV files
│       ├── bofa-checking-2025-01.csv
│       ├── bofa-checking-2025-02.csv
│       └── gusto-payroll-2025-01.csv
├── scripts/
│   ├── init_db.py             # Create tables and seed categories
│   ├── import_csv.py          # Import a CSV into the database
│   ├── categorize.py          # Run auto-categorization pass
│   ├── review.py              # Interactive review of flagged items
│   └── reports.py             # Generate reports
├── exports/
│   └── (generated reports go here)
└── README.md
```

---

## Implementation Phases

### Phase 1: Foundation
- [ ] Create SQLite database with schema
- [ ] Seed default category taxonomy
- [ ] Build CSV import script with duplicate detection
- [ ] Support BofA checking CSV format
- [ ] Support BofA credit card CSV format (if applicable)

### Phase 2: Categorization Engine
- [ ] Build rules-based auto-categorization
- [ ] Build interactive review flow for flagged transactions
- [ ] Rule creation from confirmed categorizations
- [ ] Claude suggestion engine for unknown transactions

### Phase 3: Reports
- [ ] Profit & Loss report
- [ ] Expense breakdown report
- [ ] Tax summary report
- [ ] Cash flow report
- [ ] Flagged transaction report

### Phase 4: Reconciliation
- [ ] Monthly reconciliation workflow
- [ ] Statement balance comparison
- [ ] Discrepancy detection and flagging

### Phase 5: Payroll Integration
- [ ] Gusto CSV/report parsing
- [ ] Payroll transaction import with proper categorization
- [ ] Payroll tax and benefits breakdowns

### Phase 6: Quality of Life
- [ ] Recurring transaction detection
- [ ] Anomaly detection (unusual amounts, new vendors)
- [ ] Year-over-year comparison reports
- [ ] Export to accountant-friendly format (if ever needed)
- [ ] Backup and restore utilities

---

## Notes & Considerations

### On Accuracy
This system is only as good as the categorization rules. The first month or two will require more hands-on review as you build up the rules table. After that, most transactions should auto-categorize and you'll spend 10-15 minutes a month on review.

### On Tax Compliance
This system tracks income and expenses by tax category. It does NOT calculate tax liability, estimated quarterly payments, or handle depreciation schedules. For those, consult IRS publications or a tax professional. The goal is to have clean, categorized data ready for tax preparation.

### On the Bookkeeper Transition
Consider running this system in parallel with your current bookkeeper for one quarter. Compare results. Once you're confident, cut over.

### On Double-Entry
If you ever need to move to double-entry (taking on investors, applying for significant financing, or just wanting more rigorous books), the schema can be extended with a `journal_entries` table and debit/credit columns. The transaction data and categories remain the same — it's an additive change, not a rewrite.

### On BofA CSV Format
BofA's CSV format may vary slightly between account types. The import script should handle common variations and flag anything it can't parse. Keep a sample of each format in the imports directory for reference.

### On Gusto
Gusto's payroll reports contain sensitive employee data. The import should extract only aggregate totals (total wages, total employer taxes, total benefits) and discard individual employee details. Store the original reports securely outside the project directory.
