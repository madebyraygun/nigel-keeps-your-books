# Nigel Invoicing + Static Web Publishing — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add first-class invoicing to Nigel — author invoices, publish PDF + static HTML to Cloudflare R2 under `billing.rygn.io/i/{token}/`, email clients via Mailgun, offer a Stripe Payment Link, and reconcile payments by pull-sync — so Nigel can replace InvoiceShelf.

**Architecture:** Nigel stays a local, outbound-only Rust TUI/CLI app and the system of record. Invoicing data lives in the existing encrypted SQLite via one new migration. External work (Stripe, R2, Mailgun) sits behind three traits (`PaymentGateway`, `AssetPublisher`, `Mailer`) so orchestration is unit-tested with fakes and the real clients are thin `reqwest` request builders tested offline. Confirmation is pull-based: `invoice sync` polls Stripe; nothing listens.

**Tech Stack:** Rust, `rusqlite` (sqlcipher), `clap`, `reqwest` (blocking), `printpdf` (feature `pdf`), `rand`, `serde`/`serde_json`, and one new dep `rusty-s3` for R2 signing.

## Global Constraints

- **Money is `f64`** everywhere, matching `transactions.amount REAL`. Store invoice amounts as `REAL`. Convert to integer cents only at the Stripe boundary: `(amount * 100.0).round() as i64`.
- **Migrations** are appended to the `MIGRATIONS` array in `src/migrations.rs` (one `Migration` with an `up: |conn| { conn.execute_batch(...) }`). Never edit `db::SCHEMA` for new tables; add a migration. Bump nothing else — `LATEST_VERSION` derives from the array.
- **Data-layer functions take `&Connection`** and are unit-tested with the `test_conn()` helper (tempdir + `get_connection` + `init_db` + `run_migrations`). CLI wrappers open the connection via `get_connection(&get_data_dir().join("nigel.db"))` and print.
- **Tests never touch the network.** External clients expose pure builder/parse functions asserted directly; orchestration uses trait fakes.
- **Commits are Conventional Commits** (`feat:`, `test:`, `fix:`, `chore:`), matching repo history.
- **PDF code is feature-gated** behind `#[cfg(feature = "pdf")]`, as `src/pdf.rs` already is.
- **Currency:** invoices carry a `currency` string (e.g. `USD`); Stripe currency is `currency.to_lowercase()`.
- **New crate:** add `rusty-s3 = "0.5"` to `[dependencies]` (pure request signing; no async runtime).

---

## File Structure

- `src/migrations.rs` — **modify**: append the invoicing migration.
- `src/models.rs` — **modify**: add `Client`, `Invoice`, `InvoiceLineItem`, `InvoicePayment`, `InvoiceStatus`.
- `src/invoicing/mod.rs` — **create**: module root re-exporting the submodules below.
- `src/invoicing/clients.rs` — **create**: client data layer.
- `src/invoicing/invoices.rs` — **create**: invoice create/query, numbering, token, totals, status derivation, payments, AR aging.
- `src/invoicing/render_html.rs` — **create**: static HTML page renderer.
- `src/invoicing/gateway.rs` — **create**: `PaymentGateway`/`AssetPublisher`/`Mailer` traits + shared value types.
- `src/invoicing/stripe.rs` — **create**: Stripe `PaymentGateway` impl + pure builders/parsers.
- `src/invoicing/r2.rs` — **create**: R2 `AssetPublisher` impl + object-key helper.
- `src/invoicing/mailgun.rs` — **create**: Mailgun `Mailer` impl + request builder.
- `src/invoicing/send.rs` — **create**: `send_invoice` orchestration over the three traits.
- `src/invoicing/import_invoiceshelf.rs` — **create**: one-time InvoiceShelf SQLite importer.
- `src/pdf.rs` — **modify**: add `render_invoice_pdf` (feature-gated).
- `src/settings.rs` — **modify**: add secret/config fields + env-override accessors.
- `src/cli/invoice.rs`, `src/cli/client.rs` — **create**: thin CLI wrappers.
- `src/cli/mod.rs`, `src/main.rs` — **modify**: register `Invoice`/`Client` command groups + dispatch.
- `Cargo.toml` — **modify**: add `rusty-s3`.

---

## Task 1: Schema migration + model structs

**Files:**
- Modify: `src/migrations.rs` (append to `MIGRATIONS`)
- Modify: `src/models.rs`
- Test: inline `#[cfg(test)]` in `src/migrations.rs`

**Interfaces:**
- Produces: tables `clients`, `invoices`, `invoice_line_items`, `invoice_payments`; structs `Client`, `Invoice`, `InvoiceLineItem`, `InvoicePayment`, `InvoiceStatus`.

- [ ] **Step 1: Write the failing test**

Add to `src/migrations.rs` tests:

```rust
#[cfg(test)]
mod invoicing_migration_tests {
    use crate::db::{get_connection, init_db};
    use crate::migrations::run_migrations;

    #[test]
    fn invoicing_tables_exist_after_migration() {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("t.db")).unwrap();
        init_db(&conn).unwrap();
        run_migrations(&conn).unwrap();
        for table in ["clients", "invoices", "invoice_line_items", "invoice_payments"] {
            let n: i64 = conn
                .query_row(
                    "SELECT count(*) FROM sqlite_master WHERE type='table' AND name=?1",
                    [table],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(n, 1, "missing table {table}");
        }
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test invoicing_tables_exist_after_migration`
Expected: FAIL (tables not found, `assert_eq!` panics).

- [ ] **Step 3: Append the migration**

Add a new element to the `MIGRATIONS` array in `src/migrations.rs` (version = current highest + 1):

```rust
Migration {
    version: 3,
    description: "add invoicing tables (clients, invoices, line items, payments)",
    up: |conn| {
        conn.execute_batch(
            "CREATE TABLE clients (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                email TEXT,
                billing_address TEXT,
                notes TEXT,
                created_at TEXT DEFAULT (datetime('now'))
            );
            CREATE TABLE invoices (
                id INTEGER PRIMARY KEY,
                number INTEGER NOT NULL UNIQUE,
                client_id INTEGER NOT NULL,
                issue_date TEXT NOT NULL,
                due_date TEXT,
                status TEXT NOT NULL DEFAULT 'draft',
                currency TEXT NOT NULL DEFAULT 'USD',
                subtotal REAL NOT NULL DEFAULT 0,
                tax REAL NOT NULL DEFAULT 0,
                total REAL NOT NULL DEFAULT 0,
                notes TEXT,
                terms TEXT,
                token TEXT NOT NULL UNIQUE,
                stripe_payment_link_id TEXT,
                stripe_payment_link_url TEXT,
                published_at TEXT,
                created_at TEXT DEFAULT (datetime('now')),
                FOREIGN KEY (client_id) REFERENCES clients(id)
            );
            CREATE TABLE invoice_line_items (
                id INTEGER PRIMARY KEY,
                invoice_id INTEGER NOT NULL,
                description TEXT NOT NULL,
                quantity REAL NOT NULL DEFAULT 1,
                unit_amount REAL NOT NULL DEFAULT 0,
                line_total REAL NOT NULL DEFAULT 0,
                position INTEGER NOT NULL DEFAULT 0,
                FOREIGN KEY (invoice_id) REFERENCES invoices(id)
            );
            CREATE TABLE invoice_payments (
                id INTEGER PRIMARY KEY,
                invoice_id INTEGER NOT NULL,
                amount REAL NOT NULL,
                paid_date TEXT NOT NULL,
                method TEXT NOT NULL CHECK (method IN ('stripe','ach','direct_deposit','other')),
                stripe_checkout_session_id TEXT UNIQUE,
                recorded_at TEXT DEFAULT (datetime('now')),
                FOREIGN KEY (invoice_id) REFERENCES invoices(id)
            );",
        )?;
        Ok(())
    },
},
```

> If the current highest `version` is not 2, use the next integer instead of 3.

- [ ] **Step 4: Add model structs**

Append to `src/models.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvoiceStatus {
    Draft,
    Sent,
    Partial,
    Paid,
    Overdue,
    Void,
}

impl InvoiceStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Sent => "sent",
            Self::Partial => "partial",
            Self::Paid => "paid",
            Self::Overdue => "overdue",
            Self::Void => "void",
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Client {
    pub id: i64,
    pub name: String,
    pub email: Option<String>,
    pub billing_address: Option<String>,
    pub notes: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct InvoiceLineItem {
    pub id: Option<i64>,
    pub invoice_id: Option<i64>,
    pub description: String,
    pub quantity: f64,
    pub unit_amount: f64,
    pub line_total: f64,
    pub position: i64,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Invoice {
    pub id: i64,
    pub number: i64,
    pub client_id: i64,
    pub issue_date: String,
    pub due_date: Option<String>,
    pub status: String,
    pub currency: String,
    pub subtotal: f64,
    pub tax: f64,
    pub total: f64,
    pub notes: Option<String>,
    pub terms: Option<String>,
    pub token: String,
    pub stripe_payment_link_id: Option<String>,
    pub stripe_payment_link_url: Option<String>,
    pub published_at: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct InvoicePayment {
    pub id: Option<i64>,
    pub invoice_id: i64,
    pub amount: f64,
    pub paid_date: String,
    pub method: String,
    pub stripe_checkout_session_id: Option<String>,
}
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test invoicing_tables_exist_after_migration`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/migrations.rs src/models.rs
git commit -m "feat(invoicing): add schema migration and model structs"
```

---

## Task 2: Module scaffolding + client data layer

**Files:**
- Create: `src/invoicing/mod.rs`, `src/invoicing/clients.rs`
- Modify: `src/main.rs` (add `mod invoicing;`)
- Test: inline in `src/invoicing/clients.rs`

**Interfaces:**
- Consumes: `Client` (Task 1), `test_conn` pattern.
- Produces: `clients::add_client(&Connection, name, email, address, notes) -> Result<i64>`, `clients::list_clients(&Connection) -> Result<Vec<Client>>`, `clients::get_client(&Connection, id) -> Result<Client>`.

- [ ] **Step 1: Write the failing test**

Create `src/invoicing/clients.rs` with only the test module first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{get_connection, init_db};
    use crate::migrations::run_migrations;

    fn test_conn() -> (tempfile::TempDir, rusqlite::Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("t.db")).unwrap();
        init_db(&conn).unwrap();
        run_migrations(&conn).unwrap();
        (dir, conn)
    }

    #[test]
    fn add_and_get_client() {
        let (_d, conn) = test_conn();
        let id = add_client(&conn, "Acme Co", Some("ap@acme.test"), None, None).unwrap();
        let c = get_client(&conn, id).unwrap();
        assert_eq!(c.name, "Acme Co");
        assert_eq!(c.email.as_deref(), Some("ap@acme.test"));
        assert_eq!(list_clients(&conn).unwrap().len(), 1);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p nigel add_and_get_client 2>&1 | head`
Expected: FAIL to compile (`add_client` undefined) — add `pub mod clients;` to `src/invoicing/mod.rs` and `mod invoicing;` to `src/main.rs` first; then it fails on the missing functions.

Create `src/invoicing/mod.rs`:

```rust
pub mod clients;
```

Add to `src/main.rs` alongside the other `mod` lines:

```rust
mod invoicing;
```

- [ ] **Step 3: Write minimal implementation**

Prepend to `src/invoicing/clients.rs` (above the test module):

```rust
use rusqlite::Connection;

use crate::error::Result;
use crate::models::Client;

pub fn add_client(
    conn: &Connection,
    name: &str,
    email: Option<&str>,
    billing_address: Option<&str>,
    notes: Option<&str>,
) -> Result<i64> {
    conn.execute(
        "INSERT INTO clients (name, email, billing_address, notes) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![name, email, billing_address, notes],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn get_client(conn: &Connection, id: i64) -> Result<Client> {
    let c = conn.query_row(
        "SELECT id, name, email, billing_address, notes FROM clients WHERE id = ?1",
        [id],
        |r| {
            Ok(Client {
                id: r.get(0)?,
                name: r.get(1)?,
                email: r.get(2)?,
                billing_address: r.get(3)?,
                notes: r.get(4)?,
            })
        },
    )?;
    Ok(c)
}

pub fn list_clients(conn: &Connection) -> Result<Vec<Client>> {
    let mut stmt =
        conn.prepare("SELECT id, name, email, billing_address, notes FROM clients ORDER BY name")?;
    let rows = stmt
        .query_map([], |r| {
            Ok(Client {
                id: r.get(0)?,
                name: r.get(1)?,
                email: r.get(2)?,
                billing_address: r.get(3)?,
                notes: r.get(4)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test add_and_get_client`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/invoicing src/main.rs
git commit -m "feat(invoicing): add client data layer and module scaffolding"
```

---

## Task 3: Invoice numbering, token, and creation

**Files:**
- Create/modify: `src/invoicing/invoices.rs`
- Modify: `src/invoicing/mod.rs` (`pub mod invoices;`)
- Test: inline in `src/invoicing/invoices.rs`

**Interfaces:**
- Consumes: `Client` (via `clients::add_client`), `db::{get_metadata, set_metadata}`.
- Produces:
  - `invoices::next_number(&Connection) -> Result<i64>` — reads metadata `next_invoice_number` (default 1248), does not mutate.
  - `invoices::gen_token() -> String` — 16-char base62.
  - `invoices::NewLineItem { description: String, quantity: f64, unit_amount: f64 }`.
  - `invoices::create_invoice(&Connection, client_id: i64, issue_date: &str, due_date: Option<&str>, currency: &str, items: &[NewLineItem], notes: Option<&str>, terms: Option<&str>) -> Result<i64>` — allocates number, bumps metadata, computes `line_total`/`subtotal`/`total`, inserts invoice + items, status `draft`.
  - `invoices::get_invoice(&Connection, id) -> Result<Invoice>` and `get_invoice_by_number(&Connection, number) -> Result<Invoice>`.

- [ ] **Step 1: Write the failing test**

Create `src/invoicing/invoices.rs` test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{get_connection, init_db};
    use crate::invoicing::clients::add_client;
    use crate::migrations::run_migrations;

    fn test_conn() -> (tempfile::TempDir, rusqlite::Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("t.db")).unwrap();
        init_db(&conn).unwrap();
        run_migrations(&conn).unwrap();
        (dir, conn)
    }

    #[test]
    fn token_is_16_alphanumeric() {
        let t = gen_token();
        assert_eq!(t.len(), 16);
        assert!(t.chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[test]
    fn first_number_defaults_to_1248_and_increments() {
        let (_d, conn) = test_conn();
        let cid = add_client(&conn, "Acme", Some("a@b.test"), None, None).unwrap();
        let items = vec![
            NewLineItem { description: "Design".into(), quantity: 2.0, unit_amount: 100.0 },
            NewLineItem { description: "Dev".into(), quantity: 1.0, unit_amount: 50.0 },
        ];
        let id1 = create_invoice(&conn, cid, "2026-08-04", Some("2026-09-03"), "USD", &items, None, None).unwrap();
        let inv1 = get_invoice(&conn, id1).unwrap();
        assert_eq!(inv1.number, 1248);
        assert_eq!(inv1.subtotal, 250.0);
        assert_eq!(inv1.total, 250.0);
        assert_eq!(inv1.status, "draft");

        let id2 = create_invoice(&conn, cid, "2026-08-05", None, "USD", &items, None, None).unwrap();
        assert_eq!(get_invoice(&conn, id2).unwrap().number, 1249);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Add `pub mod invoices;` to `src/invoicing/mod.rs`.
Run: `cargo test -p nigel invoices:: 2>&1 | head`
Expected: FAIL to compile (undefined items).

- [ ] **Step 3: Write minimal implementation**

Prepend to `src/invoicing/invoices.rs`:

```rust
use rand::distributions::Alphanumeric;
use rand::Rng;
use rusqlite::Connection;

use crate::db::{get_metadata, set_metadata};
use crate::error::Result;
use crate::models::Invoice;

const NEXT_NUMBER_KEY: &str = "next_invoice_number";
const NEXT_NUMBER_DEFAULT: i64 = 1248;

pub struct NewLineItem {
    pub description: String,
    pub quantity: f64,
    pub unit_amount: f64,
}

pub fn gen_token() -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(16)
        .map(char::from)
        .collect()
}

pub fn next_number(conn: &Connection) -> Result<i64> {
    let n = get_metadata(conn, NEXT_NUMBER_KEY)
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(NEXT_NUMBER_DEFAULT);
    Ok(n)
}

pub fn create_invoice(
    conn: &Connection,
    client_id: i64,
    issue_date: &str,
    due_date: Option<&str>,
    currency: &str,
    items: &[NewLineItem],
    notes: Option<&str>,
    terms: Option<&str>,
) -> Result<i64> {
    let number = next_number(conn)?;
    let subtotal: f64 = items.iter().map(|i| i.quantity * i.unit_amount).sum();
    let tax = 0.0;
    let total = subtotal + tax;
    let token = gen_token();

    conn.execute(
        "INSERT INTO invoices
            (number, client_id, issue_date, due_date, status, currency, subtotal, tax, total, notes, terms, token)
         VALUES (?1, ?2, ?3, ?4, 'draft', ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        rusqlite::params![
            number, client_id, issue_date, due_date, currency, subtotal, tax, total, notes, terms, token
        ],
    )?;
    let invoice_id = conn.last_insert_rowid();

    for (idx, item) in items.iter().enumerate() {
        let line_total = item.quantity * item.unit_amount;
        conn.execute(
            "INSERT INTO invoice_line_items
                (invoice_id, description, quantity, unit_amount, line_total, position)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![invoice_id, item.description, item.quantity, item.unit_amount, line_total, idx as i64],
        )?;
    }

    set_metadata(conn, NEXT_NUMBER_KEY, &(number + 1).to_string())?;
    Ok(invoice_id)
}

fn row_to_invoice(r: &rusqlite::Row) -> rusqlite::Result<Invoice> {
    Ok(Invoice {
        id: r.get(0)?,
        number: r.get(1)?,
        client_id: r.get(2)?,
        issue_date: r.get(3)?,
        due_date: r.get(4)?,
        status: r.get(5)?,
        currency: r.get(6)?,
        subtotal: r.get(7)?,
        tax: r.get(8)?,
        total: r.get(9)?,
        notes: r.get(10)?,
        terms: r.get(11)?,
        token: r.get(12)?,
        stripe_payment_link_id: r.get(13)?,
        stripe_payment_link_url: r.get(14)?,
        published_at: r.get(15)?,
    })
}

const INVOICE_COLS: &str = "id, number, client_id, issue_date, due_date, status, currency,
    subtotal, tax, total, notes, terms, token, stripe_payment_link_id,
    stripe_payment_link_url, published_at";

pub fn get_invoice(conn: &Connection, id: i64) -> Result<Invoice> {
    Ok(conn.query_row(
        &format!("SELECT {INVOICE_COLS} FROM invoices WHERE id = ?1"),
        [id],
        row_to_invoice,
    )?)
}

pub fn get_invoice_by_number(conn: &Connection, number: i64) -> Result<Invoice> {
    Ok(conn.query_row(
        &format!("SELECT {INVOICE_COLS} FROM invoices WHERE number = ?1"),
        [number],
        row_to_invoice,
    )?)
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p nigel invoices::tests`
Expected: PASS (both tests).

- [ ] **Step 5: Commit**

```bash
git add src/invoicing/invoices.rs src/invoicing/mod.rs
git commit -m "feat(invoicing): invoice creation with numbering and token"
```

---

## Task 4: Payments and derived status

**Files:**
- Modify: `src/invoicing/invoices.rs`
- Test: inline

**Interfaces:**
- Consumes: `create_invoice`, `get_invoice` (Task 3).
- Produces:
  - `invoices::paid_amount(&Connection, invoice_id) -> Result<f64>`.
  - `invoices::record_payment(&Connection, invoice_id, amount, paid_date, method, stripe_session: Option<&str>) -> Result<bool>` — inserts a payment (idempotent when `stripe_session` already recorded → returns `false`; new → `true`), then calls `refresh_status`.
  - `invoices::refresh_status(&Connection, invoice_id, today: &str) -> Result<String>` — recomputes and persists status from paid sum + due date, returns new status. Never downgrades `void`.
  - `invoices::line_items(&Connection, invoice_id) -> Result<Vec<InvoiceLineItem>>`.

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `src/invoicing/invoices.rs`:

```rust
    #[test]
    fn recording_full_payment_marks_paid() {
        let (_d, conn) = test_conn();
        let cid = add_client(&conn, "Acme", Some("a@b.test"), None, None).unwrap();
        let items = vec![NewLineItem { description: "Work".into(), quantity: 1.0, unit_amount: 200.0 }];
        let id = create_invoice(&conn, cid, "2026-08-04", Some("2026-09-03"), "USD", &items, None, None).unwrap();

        assert_eq!(record_payment(&conn, id, 200.0, "2026-08-10", "direct_deposit", None).unwrap(), true);
        assert_eq!(paid_amount(&conn, id).unwrap(), 200.0);
        assert_eq!(refresh_status(&conn, id, "2026-08-11").unwrap(), "paid");
    }

    #[test]
    fn partial_then_overdue_is_derived() {
        let (_d, conn) = test_conn();
        let cid = add_client(&conn, "Acme", Some("a@b.test"), None, None).unwrap();
        let items = vec![NewLineItem { description: "Work".into(), quantity: 1.0, unit_amount: 200.0 }];
        let id = create_invoice(&conn, cid, "2026-08-04", Some("2026-08-20"), "USD", &items, None, None).unwrap();
        // simulate published
        conn.execute("UPDATE invoices SET status='sent', published_at='2026-08-04' WHERE id=?1", [id]).unwrap();

        record_payment(&conn, id, 50.0, "2026-08-10", "ach", None).unwrap();
        assert_eq!(refresh_status(&conn, id, "2026-08-15").unwrap(), "partial");
        // past due, still owing
        assert_eq!(refresh_status(&conn, id, "2026-08-25").unwrap(), "overdue");
    }

    #[test]
    fn stripe_session_is_idempotent() {
        let (_d, conn) = test_conn();
        let cid = add_client(&conn, "Acme", Some("a@b.test"), None, None).unwrap();
        let items = vec![NewLineItem { description: "Work".into(), quantity: 1.0, unit_amount: 100.0 }];
        let id = create_invoice(&conn, cid, "2026-08-04", None, "USD", &items, None, None).unwrap();

        assert_eq!(record_payment(&conn, id, 100.0, "2026-08-10", "stripe", Some("cs_1")).unwrap(), true);
        assert_eq!(record_payment(&conn, id, 100.0, "2026-08-10", "stripe", Some("cs_1")).unwrap(), false);
        assert_eq!(paid_amount(&conn, id).unwrap(), 100.0);
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p nigel invoices:: 2>&1 | head`
Expected: FAIL to compile (undefined functions).

- [ ] **Step 3: Write minimal implementation**

Add to `src/invoicing/invoices.rs` (above the test module):

```rust
use crate::models::{InvoiceLineItem, InvoiceStatus};

pub fn paid_amount(conn: &Connection, invoice_id: i64) -> Result<f64> {
    let sum: Option<f64> = conn.query_row(
        "SELECT SUM(amount) FROM invoice_payments WHERE invoice_id = ?1",
        [invoice_id],
        |r| r.get(0),
    )?;
    Ok(sum.unwrap_or(0.0))
}

pub fn record_payment(
    conn: &Connection,
    invoice_id: i64,
    amount: f64,
    paid_date: &str,
    method: &str,
    stripe_session: Option<&str>,
) -> Result<bool> {
    if let Some(sid) = stripe_session {
        let seen: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM invoice_payments WHERE stripe_checkout_session_id = ?1)",
            [sid],
            |r| r.get(0),
        )?;
        if seen {
            return Ok(false);
        }
    }
    conn.execute(
        "INSERT INTO invoice_payments (invoice_id, amount, paid_date, method, stripe_checkout_session_id)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![invoice_id, amount, paid_date, method, stripe_session],
    )?;
    // Recompute status using paid_date as the reference day (deterministic, testable).
    refresh_status(conn, invoice_id, paid_date)?;
    Ok(true)
}

pub fn refresh_status(conn: &Connection, invoice_id: i64, today: &str) -> Result<String> {
    let inv = get_invoice(conn, invoice_id)?;
    if inv.status == InvoiceStatus::Void.as_str() {
        return Ok(inv.status);
    }
    let paid = paid_amount(conn, invoice_id)?;
    let published = inv.published_at.is_some();
    let owing = inv.total - paid;

    let status = if paid + f64::EPSILON >= inv.total && inv.total > 0.0 {
        InvoiceStatus::Paid
    } else if !published {
        InvoiceStatus::Draft
    } else if is_overdue(inv.due_date.as_deref(), today) && owing > 0.0 {
        InvoiceStatus::Overdue
    } else if paid > 0.0 {
        InvoiceStatus::Partial
    } else {
        InvoiceStatus::Sent
    };

    conn.execute(
        "UPDATE invoices SET status = ?1 WHERE id = ?2",
        rusqlite::params![status.as_str(), invoice_id],
    )?;
    Ok(status.as_str().to_string())
}

fn is_overdue(due_date: Option<&str>, today: &str) -> bool {
    // ISO YYYY-MM-DD compares lexicographically.
    match due_date {
        Some(d) => today > d,
        None => false,
    }
}

pub fn line_items(conn: &Connection, invoice_id: i64) -> Result<Vec<InvoiceLineItem>> {
    let mut stmt = conn.prepare(
        "SELECT id, invoice_id, description, quantity, unit_amount, line_total, position
         FROM invoice_line_items WHERE invoice_id = ?1 ORDER BY position",
    )?;
    let rows = stmt
        .query_map([invoice_id], |r| {
            Ok(InvoiceLineItem {
                id: r.get(0)?,
                invoice_id: r.get(1)?,
                description: r.get(2)?,
                quantity: r.get(3)?,
                unit_amount: r.get(4)?,
                line_total: r.get(5)?,
                position: r.get(6)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p nigel invoices::tests`
Expected: PASS (all five tests in the module).

- [ ] **Step 5: Commit**

```bash
git add src/invoicing/invoices.rs
git commit -m "feat(invoicing): payments and derived invoice status"
```

---

## Task 5: AR aging report

**Files:**
- Modify: `src/invoicing/invoices.rs`
- Test: inline

**Interfaces:**
- Produces: `invoices::AgingBucket { label: &'static str, total: f64 }` and `invoices::ar_aging(&Connection, today: &str) -> Result<Vec<AgingBucket>>` returning buckets `current`, `1-30`, `31-60`, `61-90`, `90+` computed from each open invoice's owing balance and days past `due_date` (or `issue_date` when `due_date` is null).

- [ ] **Step 1: Write the failing test**

```rust
    #[test]
    fn aging_buckets_split_by_days_past_due() {
        let (_d, conn) = test_conn();
        let cid = add_client(&conn, "Acme", Some("a@b.test"), None, None).unwrap();
        let items = vec![NewLineItem { description: "W".into(), quantity: 1.0, unit_amount: 100.0 }];
        // due 5 days ago -> "1-30"
        let a = create_invoice(&conn, cid, "2026-07-01", Some("2026-07-30"), "USD", &items, None, None).unwrap();
        // due 45 days ago -> "31-60"
        let b = create_invoice(&conn, cid, "2026-06-01", Some("2026-06-20"), "USD", &items, None, None).unwrap();
        conn.execute("UPDATE invoices SET status='sent', published_at='x' WHERE id IN (?1,?2)", [a, b]).unwrap();

        let buckets = ar_aging(&conn, "2026-08-04").unwrap();
        let get = |label: &str| buckets.iter().find(|x| x.label == label).unwrap().total;
        assert_eq!(get("1-30"), 100.0);
        assert_eq!(get("31-60"), 100.0);
        assert_eq!(get("90+"), 0.0);
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p nigel aging_buckets_split_by_days_past_due 2>&1 | head`
Expected: FAIL to compile.

- [ ] **Step 3: Write minimal implementation**

Add to `src/invoicing/invoices.rs`:

```rust
use chrono::NaiveDate;

pub struct AgingBucket {
    pub label: &'static str,
    pub total: f64,
}

pub fn ar_aging(conn: &Connection, today: &str) -> Result<Vec<AgingBucket>> {
    let today = NaiveDate::parse_from_str(today, "%Y-%m-%d")
        .map_err(|e| crate::error::NigelError::Other(format!("bad date {today}: {e}")))?;

    let mut buckets = [
        AgingBucket { label: "current", total: 0.0 },
        AgingBucket { label: "1-30", total: 0.0 },
        AgingBucket { label: "31-60", total: 0.0 },
        AgingBucket { label: "61-90", total: 0.0 },
        AgingBucket { label: "90+", total: 0.0 },
    ];

    let mut stmt = conn.prepare(
        "SELECT id, total, COALESCE(due_date, issue_date) FROM invoices
         WHERE status IN ('sent','partial','overdue')",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, f64>(1)?, r.get::<_, String>(2)?))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    for (id, total, due) in rows {
        let owing = total - paid_amount(conn, id)?;
        if owing <= 0.0 {
            continue;
        }
        let due = NaiveDate::parse_from_str(&due, "%Y-%m-%d").unwrap_or(today);
        let days = (today - due).num_days();
        let idx = if days <= 0 {
            0
        } else if days <= 30 {
            1
        } else if days <= 60 {
            2
        } else if days <= 90 {
            3
        } else {
            4
        };
        buckets[idx].total += owing;
    }
    Ok(buckets.into_iter().collect())
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p nigel aging_buckets_split_by_days_past_due`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/invoicing/invoices.rs
git commit -m "feat(invoicing): AR aging report calculation"
```

---

## Task 6: Settings — secrets and config

**Files:**
- Modify: `src/settings.rs`
- Test: inline in `src/settings.rs`

**Interfaces:**
- Produces: new optional `Settings` fields and accessor `settings::invoicing_config() -> InvoicingConfig` reading each value from env first, then settings file. `InvoicingConfig { stripe_secret_key, mailgun_api_key, mailgun_domain, from_email, r2_account_id, r2_access_key, r2_secret_key, r2_bucket, public_base_url }` (all `Option<String>` except `public_base_url` defaulting to `https://billing.rygn.io/i`).

- [ ] **Step 1: Write the failing test**

Add to `src/settings.rs` tests:

```rust
    #[test]
    fn invoicing_config_prefers_env_over_settings() {
        // Uses a real env var name; set/remove around the assertion.
        std::env::set_var("NIGEL_STRIPE_SECRET_KEY", "rk_env");
        let cfg = invoicing_config_from(
            &Settings { data_dir: "/x".into(), user_name: String::new(), update_check: true, last_update_check: None,
                stripe_secret_key: Some("rk_file".into()), ..Settings::default() },
        );
        assert_eq!(cfg.stripe_secret_key.as_deref(), Some("rk_env"));
        std::env::remove_var("NIGEL_STRIPE_SECRET_KEY");

        let cfg2 = invoicing_config_from(
            &Settings { stripe_secret_key: Some("rk_file".into()), ..Settings::default() },
        );
        assert_eq!(cfg2.stripe_secret_key.as_deref(), Some("rk_file"));
        assert_eq!(cfg2.public_base_url, "https://billing.rygn.io/i");
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p nigel invoicing_config_prefers_env 2>&1 | head`
Expected: FAIL to compile.

- [ ] **Step 3: Write minimal implementation**

Add fields to the `Settings` struct (each `#[serde(default)]`):

```rust
    #[serde(default)]
    pub stripe_secret_key: Option<String>,
    #[serde(default)]
    pub mailgun_api_key: Option<String>,
    #[serde(default)]
    pub mailgun_domain: Option<String>,
    #[serde(default)]
    pub from_email: Option<String>,
    #[serde(default)]
    pub r2_account_id: Option<String>,
    #[serde(default)]
    pub r2_access_key: Option<String>,
    #[serde(default)]
    pub r2_secret_key: Option<String>,
    #[serde(default)]
    pub r2_bucket: Option<String>,
    #[serde(default)]
    pub public_base_url: Option<String>,
```

Add the corresponding `None` initializers to the `Default` impl. Then add:

```rust
pub struct InvoicingConfig {
    pub stripe_secret_key: Option<String>,
    pub mailgun_api_key: Option<String>,
    pub mailgun_domain: String,
    pub from_email: String,
    pub r2_account_id: Option<String>,
    pub r2_access_key: Option<String>,
    pub r2_secret_key: Option<String>,
    pub r2_bucket: Option<String>,
    pub public_base_url: String,
}

fn env_or(name: &str, file_val: &Option<String>) -> Option<String> {
    std::env::var(name).ok().or_else(|| file_val.clone())
}

pub fn invoicing_config_from(s: &Settings) -> InvoicingConfig {
    InvoicingConfig {
        stripe_secret_key: env_or("NIGEL_STRIPE_SECRET_KEY", &s.stripe_secret_key),
        mailgun_api_key: env_or("NIGEL_MAILGUN_API_KEY", &s.mailgun_api_key),
        mailgun_domain: env_or("NIGEL_MAILGUN_DOMAIN", &s.mailgun_domain).unwrap_or_else(|| "rygn.io".into()),
        from_email: env_or("NIGEL_FROM_EMAIL", &s.from_email).unwrap_or_else(|| "billing@rygn.io".into()),
        r2_account_id: env_or("NIGEL_R2_ACCOUNT_ID", &s.r2_account_id),
        r2_access_key: env_or("NIGEL_R2_ACCESS_KEY", &s.r2_access_key),
        r2_secret_key: env_or("NIGEL_R2_SECRET_KEY", &s.r2_secret_key),
        r2_bucket: env_or("NIGEL_R2_BUCKET", &s.r2_bucket),
        public_base_url: env_or("NIGEL_PUBLIC_BASE_URL", &s.public_base_url)
            .unwrap_or_else(|| "https://billing.rygn.io/i".into()),
    }
}

pub fn invoicing_config() -> InvoicingConfig {
    invoicing_config_from(&load_settings())
}
```

> Update the existing `Settings { ... }` literals in `settings.rs` tests to include `..Settings::default()` so they keep compiling.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p nigel settings`
Expected: PASS (new test + existing settings tests).

- [ ] **Step 5: Commit**

```bash
git add src/settings.rs
git commit -m "feat(invoicing): settings for stripe, mailgun, and r2 with env overrides"
```

---

## Task 7: Static HTML invoice page renderer

**Files:**
- Create: `src/invoicing/render_html.rs`, `src/invoicing/templates/invoice.html`
- Modify: `src/invoicing/mod.rs`
- Test: inline

**Interfaces:**
- Consumes: `Invoice`, `Client`, `Vec<InvoiceLineItem>`.
- Produces: `render_html::render_invoice_html(&Invoice, &Client, &[InvoiceLineItem], pay_url: Option<&str>) -> String`. Includes each line item, the total, a **Pay online** anchor when `pay_url` is `Some`, and a direct-deposit instructions block. HTML-escapes client/description text.

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Client, Invoice, InvoiceLineItem};

    fn sample() -> (Invoice, Client, Vec<InvoiceLineItem>) {
        let inv = Invoice {
            id: 1, number: 1248, client_id: 1, issue_date: "2026-08-04".into(),
            due_date: Some("2026-09-03".into()), status: "sent".into(), currency: "USD".into(),
            subtotal: 250.0, tax: 0.0, total: 250.0, notes: None, terms: None,
            token: "abc123".into(), stripe_payment_link_id: None,
            stripe_payment_link_url: None, published_at: Some("2026-08-04".into()),
        };
        let client = Client { id: 1, name: "Acme <Co>".into(), email: Some("a@b.test".into()), billing_address: None, notes: None };
        let items = vec![InvoiceLineItem { id: None, invoice_id: Some(1), description: "Design".into(), quantity: 2.0, unit_amount: 100.0, line_total: 200.0, position: 0 }];
        (inv, client, items)
    }

    #[test]
    fn renders_number_total_items_and_pay_button() {
        let (inv, client, items) = sample();
        let html = render_invoice_html(&inv, &client, &items, Some("https://pay.stripe.test/x"));
        assert!(html.contains("1248"));
        assert!(html.contains("Design"));
        assert!(html.contains("250.00"));
        assert!(html.contains("https://pay.stripe.test/x"));
        assert!(html.contains("Direct deposit"));
        assert!(html.contains("Acme &lt;Co&gt;")); // escaped
    }

    #[test]
    fn omits_pay_button_when_no_url() {
        let (inv, client, items) = sample();
        let html = render_invoice_html(&inv, &client, &items, None);
        assert!(!html.contains("Pay online"));
        assert!(html.contains("Direct deposit"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Add `pub mod render_html;` to `src/invoicing/mod.rs`.
Run: `cargo test -p nigel render_html 2>&1 | head`
Expected: FAIL to compile.

- [ ] **Step 3: Write minimal implementation**

Create `src/invoicing/templates/invoice.html`:

```html
<!doctype html>
<html><head><meta charset="utf-8"><title>Invoice {{NUMBER}}</title>
<style>body{font-family:system-ui,sans-serif;max-width:40rem;margin:2rem auto;padding:0 1rem}
table{width:100%;border-collapse:collapse}td,th{text-align:left;padding:.4rem;border-bottom:1px solid #ddd}
.total{font-weight:700}.pay{display:inline-block;margin:1rem 0;padding:.6rem 1rem;background:#111;color:#fff;text-decoration:none;border-radius:.4rem}</style>
</head><body>
<h1>Invoice #{{NUMBER}}</h1>
<p>Billed to: {{CLIENT}}<br>Issued: {{ISSUE}}{{DUE}}</p>
<table><thead><tr><th>Description</th><th>Qty</th><th>Unit</th><th>Amount</th></tr></thead>
<tbody>{{ROWS}}</tbody></table>
<p class="total">Total: {{CURRENCY}} {{TOTAL}}</p>
{{PAY}}
<h3>Direct deposit</h3>
<p>To pay by bank transfer, reference invoice <strong>#{{NUMBER}}</strong>. Contact billing@rygn.io for account details.</p>
</body></html>
```

Create `src/invoicing/render_html.rs`:

```rust
use crate::models::{Client, Invoice, InvoiceLineItem};

const TEMPLATE: &str = include_str!("templates/invoice.html");

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

pub fn render_invoice_html(
    invoice: &Invoice,
    client: &Client,
    items: &[InvoiceLineItem],
    pay_url: Option<&str>,
) -> String {
    let rows: String = items
        .iter()
        .map(|i| {
            format!(
                "<tr><td>{}</td><td>{}</td><td>{:.2}</td><td>{:.2}</td></tr>",
                esc(&i.description), i.quantity, i.unit_amount, i.line_total
            )
        })
        .collect();

    let pay = match pay_url {
        Some(url) => format!("<a class=\"pay\" href=\"{}\">Pay online</a>", esc(url)),
        None => String::new(),
    };
    let due = invoice
        .due_date
        .as_deref()
        .map(|d| format!("<br>Due: {d}"))
        .unwrap_or_default();

    TEMPLATE
        .replace("{{NUMBER}}", &invoice.number.to_string())
        .replace("{{CLIENT}}", &esc(&client.name))
        .replace("{{ISSUE}}", &invoice.issue_date)
        .replace("{{DUE}}", &due)
        .replace("{{ROWS}}", &rows)
        .replace("{{CURRENCY}}", &invoice.currency)
        .replace("{{TOTAL}}", &format!("{:.2}", invoice.total))
        .replace("{{PAY}}", &pay)
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p nigel render_html`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/invoicing/render_html.rs src/invoicing/templates/invoice.html src/invoicing/mod.rs
git commit -m "feat(invoicing): static HTML invoice page renderer"
```

---

## Task 8: PDF invoice renderer (feature-gated)

**Files:**
- Modify: `src/pdf.rs`
- Test: inline in `src/pdf.rs` (gated `#[cfg(all(test, feature = "pdf"))]`)

**Interfaces:**
- Produces: `pdf::render_invoice_pdf(&Invoice, &Client, &[InvoiceLineItem]) -> Result<Vec<u8>>` returning PDF bytes.

- [ ] **Step 1: Write the failing test**

Add to `src/pdf.rs`:

```rust
#[cfg(all(test, feature = "pdf"))]
mod invoice_pdf_tests {
    use super::*;
    use crate::models::{Client, Invoice, InvoiceLineItem};

    #[test]
    fn produces_nonempty_pdf() {
        let inv = Invoice {
            id: 1, number: 1248, client_id: 1, issue_date: "2026-08-04".into(),
            due_date: None, status: "draft".into(), currency: "USD".into(),
            subtotal: 100.0, tax: 0.0, total: 100.0, notes: None, terms: None,
            token: "t".into(), stripe_payment_link_id: None, stripe_payment_link_url: None,
            published_at: None,
        };
        let client = Client { id: 1, name: "Acme".into(), email: None, billing_address: None, notes: None };
        let items = vec![InvoiceLineItem { id: None, invoice_id: Some(1), description: "Work".into(), quantity: 1.0, unit_amount: 100.0, line_total: 100.0, position: 0 }];
        let bytes = render_invoice_pdf(&inv, &client, &items).unwrap();
        assert!(bytes.len() > 100);
        assert_eq!(&bytes[0..4], b"%PDF");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p nigel --features pdf produces_nonempty_pdf 2>&1 | head`
Expected: FAIL to compile (`render_invoice_pdf` undefined).

- [ ] **Step 3: Write minimal implementation**

Add a `render_invoice_pdf` function to `src/pdf.rs` following the existing report-PDF construction in that file (reuse its `printpdf` document setup, font loading, and line-writing helpers). Minimal shape:

```rust
use crate::models::{Client, Invoice, InvoiceLineItem};

pub fn render_invoice_pdf(
    invoice: &Invoice,
    client: &Client,
    items: &[InvoiceLineItem],
) -> crate::error::Result<Vec<u8>> {
    use printpdf::*;
    let (doc, page, layer) =
        PdfDocument::new(format!("Invoice {}", invoice.number), Mm(210.0), Mm(297.0), "layer");
    let font = doc
        .add_builtin_font(BuiltinFont::Helvetica)
        .map_err(|e| crate::error::NigelError::Pdf(e.to_string()))?;
    let current = doc.get_page(page).get_layer(layer);

    let mut y = 270.0;
    let mut line = |text: String, size: f64, yy: &mut f64| {
        current.use_text(text, size as f32, Mm(20.0), Mm(*yy as f32), &font);
        *yy -= 8.0;
    };

    line(format!("Invoice #{}", invoice.number), 18.0, &mut y);
    line(format!("Billed to: {}", client.name), 11.0, &mut y);
    line(format!("Issued: {}", invoice.issue_date), 11.0, &mut y);
    if let Some(d) = &invoice.due_date {
        line(format!("Due: {d}"), 11.0, &mut y);
    }
    y -= 4.0;
    for it in items {
        line(
            format!("{}  x{}  @{:.2}  = {:.2}", it.description, it.quantity, it.unit_amount, it.line_total),
            11.0, &mut y,
        );
    }
    y -= 4.0;
    line(format!("Total: {} {:.2}", invoice.currency, invoice.total), 13.0, &mut y);

    let bytes = doc
        .save_to_bytes()
        .map_err(|e| crate::error::NigelError::Pdf(e.to_string()))?;
    Ok(bytes)
}
```

> Match the exact `printpdf` 0.7 API already used elsewhere in `pdf.rs`; if the file wraps document creation in a helper, call that helper instead of re-creating the document inline.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p nigel --features pdf produces_nonempty_pdf`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/pdf.rs
git commit -m "feat(invoicing): PDF invoice renderer"
```

---

## Task 9: Integration traits and value types

**Files:**
- Create: `src/invoicing/gateway.rs`
- Modify: `src/invoicing/mod.rs`
- Test: inline (a fake implementing all three traits, used by later tasks)

**Interfaces:**
- Produces:
  - `gateway::PaymentLink { id: String, url: String }`
  - `gateway::PaidSession { session_id: String, amount: f64 }`
  - `trait PaymentGateway { fn create_payment_link(&self, invoice: &Invoice, client: &Client) -> Result<PaymentLink>; fn paid_sessions(&self, payment_link_id: &str) -> Result<Vec<PaidSession>>; }`
  - `trait AssetPublisher { fn publish(&self, token: &str, html: &[u8], pdf: &[u8]) -> Result<String>; }`
  - `trait Mailer { fn send_invoice(&self, to: &str, subject: &str, html: &str, pdf: &[u8]) -> Result<()>; }`

- [ ] **Step 1: Write the failing test**

Create `src/invoicing/gateway.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    struct Ok1;
    impl AssetPublisher for Ok1 {
        fn publish(&self, token: &str, _h: &[u8], _p: &[u8]) -> crate::error::Result<String> {
            Ok(format!("https://billing.rygn.io/i/{token}/"))
        }
    }

    #[test]
    fn publisher_trait_returns_url() {
        let url = Ok1.publish("tok", b"<html>", b"%PDF").unwrap();
        assert_eq!(url, "https://billing.rygn.io/i/tok/");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Add `pub mod gateway;` to `src/invoicing/mod.rs`.
Run: `cargo test -p nigel gateway:: 2>&1 | head`
Expected: FAIL to compile.

- [ ] **Step 3: Write minimal implementation**

Prepend to `src/invoicing/gateway.rs`:

```rust
use crate::error::Result;
use crate::models::{Client, Invoice};

#[derive(Debug, Clone)]
pub struct PaymentLink {
    pub id: String,
    pub url: String,
}

#[derive(Debug, Clone)]
pub struct PaidSession {
    pub session_id: String,
    pub amount: f64,
}

pub trait PaymentGateway {
    fn create_payment_link(&self, invoice: &Invoice, client: &Client) -> Result<PaymentLink>;
    fn paid_sessions(&self, payment_link_id: &str) -> Result<Vec<PaidSession>>;
}

pub trait AssetPublisher {
    fn publish(&self, token: &str, html: &[u8], pdf: &[u8]) -> Result<String>;
}

pub trait Mailer {
    fn send_invoice(&self, to: &str, subject: &str, html: &str, pdf: &[u8]) -> Result<()>;
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p nigel gateway::`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/invoicing/gateway.rs src/invoicing/mod.rs
git commit -m "feat(invoicing): integration traits and value types"
```

---

## Task 10: Stripe client — payment-link builder and session parser

**Files:**
- Create: `src/invoicing/stripe.rs`
- Modify: `src/invoicing/mod.rs`
- Test: inline (pure builder/parser tests, no network)

**Interfaces:**
- Consumes: `PaymentGateway`, `PaymentLink`, `PaidSession` (Task 9); `Invoice`, `Client`.
- Produces:
  - `stripe::price_params(&Invoice) -> Vec<(String, String)>` — Stripe `/v1/prices` form fields (currency lowercased, integer cents).
  - `stripe::payment_link_params(price_id: &str, invoice: &Invoice) -> Vec<(String, String)>` — `/v1/payment_links` fields incl. `metadata[invoice_id]`.
  - `stripe::parse_paid_sessions(json: &str) -> Result<Vec<PaidSession>>` — from a `/v1/checkout/sessions` list response, keeping only `status=="complete" && payment_status=="paid"`, amount = `amount_total/100.0`.
  - `stripe::StripeClient { secret_key: String }` implementing `PaymentGateway` via `reqwest::blocking` (thin; not unit-tested against network).

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Invoice;

    fn inv() -> Invoice {
        Invoice { id: 1, number: 1248, client_id: 1, issue_date: "2026-08-04".into(), due_date: None,
            status: "draft".into(), currency: "USD".into(), subtotal: 250.0, tax: 0.0, total: 250.0,
            notes: None, terms: None, token: "t".into(), stripe_payment_link_id: None,
            stripe_payment_link_url: None, published_at: None }
    }

    #[test]
    fn price_params_are_lowercase_currency_and_cents() {
        let p = price_params(&inv());
        assert!(p.contains(&("currency".into(), "usd".into())));
        assert!(p.contains(&("unit_amount".into(), "25000".into())));
        assert!(p.iter().any(|(k, _)| k == "product_data[name]"));
    }

    #[test]
    fn payment_link_params_carry_invoice_metadata() {
        let p = payment_link_params("price_123", &inv());
        assert!(p.contains(&("line_items[0][price]".into(), "price_123".into())));
        assert!(p.contains(&("line_items[0][quantity]".into(), "1".into())));
        assert!(p.contains(&("metadata[invoice_id]".into(), "1248".into())));
    }

    #[test]
    fn parse_paid_sessions_filters_unpaid() {
        let json = r#"{"object":"list","data":[
            {"id":"cs_1","status":"complete","payment_status":"paid","amount_total":25000},
            {"id":"cs_2","status":"open","payment_status":"unpaid","amount_total":25000},
            {"id":"cs_3","status":"complete","payment_status":"no_payment_required","amount_total":0}
        ]}"#;
        let sessions = parse_paid_sessions(json).unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, "cs_1");
        assert_eq!(sessions[0].amount, 250.0);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Add `pub mod stripe;` to `src/invoicing/mod.rs`.
Run: `cargo test -p nigel stripe:: 2>&1 | head`
Expected: FAIL to compile.

- [ ] **Step 3: Write minimal implementation**

Prepend to `src/invoicing/stripe.rs`:

```rust
use serde::Deserialize;

use crate::error::{NigelError, Result};
use crate::invoicing::gateway::{PaidSession, PaymentGateway, PaymentLink};
use crate::models::{Client, Invoice};

fn to_cents(amount: f64) -> i64 {
    (amount * 100.0).round() as i64
}

pub fn price_params(invoice: &Invoice) -> Vec<(String, String)> {
    vec![
        ("currency".into(), invoice.currency.to_lowercase()),
        ("unit_amount".into(), to_cents(invoice.total).to_string()),
        ("product_data[name]".into(), format!("Invoice #{}", invoice.number)),
    ]
}

pub fn payment_link_params(price_id: &str, invoice: &Invoice) -> Vec<(String, String)> {
    vec![
        ("line_items[0][price]".into(), price_id.to_string()),
        ("line_items[0][quantity]".into(), "1".into()),
        ("metadata[invoice_id]".into(), invoice.number.to_string()),
    ]
}

#[derive(Deserialize)]
struct SessionList {
    data: Vec<Session>,
}
#[derive(Deserialize)]
struct Session {
    id: String,
    status: String,
    payment_status: String,
    amount_total: i64,
}

pub fn parse_paid_sessions(json: &str) -> Result<Vec<PaidSession>> {
    let list: SessionList =
        serde_json::from_str(json).map_err(|e| NigelError::Other(format!("stripe parse: {e}")))?;
    Ok(list
        .data
        .into_iter()
        .filter(|s| s.status == "complete" && s.payment_status == "paid")
        .map(|s| PaidSession { session_id: s.id, amount: s.amount_total as f64 / 100.0 })
        .collect())
}

pub struct StripeClient {
    pub secret_key: String,
}

impl StripeClient {
    fn post_form(&self, url: &str, form: &[(String, String)]) -> Result<serde_json::Value> {
        let resp = reqwest::blocking::Client::new()
            .post(url)
            .bearer_auth(&self.secret_key)
            .form(form)
            .send()
            .map_err(|e| NigelError::Other(format!("stripe request: {e}")))?;
        let status = resp.status();
        let body = resp.text().map_err(|e| NigelError::Other(e.to_string()))?;
        if !status.is_success() {
            return Err(NigelError::Other(format!("stripe {status}: {body}")));
        }
        serde_json::from_str(&body).map_err(|e| NigelError::Other(e.to_string()))
    }
}

impl PaymentGateway for StripeClient {
    fn create_payment_link(&self, invoice: &Invoice, _client: &Client) -> Result<PaymentLink> {
        let price = self.post_form("https://api.stripe.com/v1/prices", &price_params(invoice))?;
        let price_id = price["id"].as_str().ok_or_else(|| NigelError::Other("no price id".into()))?;
        let link = self.post_form(
            "https://api.stripe.com/v1/payment_links",
            &payment_link_params(price_id, invoice),
        )?;
        Ok(PaymentLink {
            id: link["id"].as_str().unwrap_or_default().to_string(),
            url: link["url"].as_str().unwrap_or_default().to_string(),
        })
    }

    fn paid_sessions(&self, payment_link_id: &str) -> Result<Vec<PaidSession>> {
        let url = format!(
            "https://api.stripe.com/v1/checkout/sessions?payment_link={payment_link_id}&limit=100"
        );
        let resp = reqwest::blocking::Client::new()
            .get(&url)
            .bearer_auth(&self.secret_key)
            .send()
            .map_err(|e| NigelError::Other(format!("stripe request: {e}")))?;
        let body = resp.text().map_err(|e| NigelError::Other(e.to_string()))?;
        parse_paid_sessions(&body)
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p nigel stripe::`
Expected: PASS (three tests).

- [ ] **Step 5: Commit**

```bash
git add src/invoicing/stripe.rs src/invoicing/mod.rs
git commit -m "feat(invoicing): stripe payment-link builder and session parser"
```

---

## Task 11: R2 asset publisher

**Files:**
- Modify: `Cargo.toml` (add `rusty-s3 = "0.5"`)
- Create: `src/invoicing/r2.rs`
- Modify: `src/invoicing/mod.rs`
- Test: inline (object-key layout; signing is exercised by the real path only)

**Interfaces:**
- Consumes: `AssetPublisher` (Task 9); `settings::InvoicingConfig`.
- Produces:
  - `r2::object_key(token: &str, filename: &str) -> String` → `"i/{token}/{filename}"`.
  - `r2::R2Publisher { account_id, access_key, secret_key, bucket, public_base_url }` implementing `AssetPublisher` (uploads `index.html` + `invoice.pdf`, returns `"{public_base_url}/{token}/"`).

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_key_layout() {
        assert_eq!(object_key("abc", "index.html"), "i/abc/index.html");
        assert_eq!(object_key("abc", "invoice.pdf"), "i/abc/invoice.pdf");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Add `rusty-s3 = "0.5"` under `[dependencies]` in `Cargo.toml`. Add `pub mod r2;` to `src/invoicing/mod.rs`.
Run: `cargo test -p nigel r2:: 2>&1 | head`
Expected: FAIL to compile.

- [ ] **Step 3: Write minimal implementation**

Prepend to `src/invoicing/r2.rs`:

```rust
use std::time::Duration;

use rusty_s3::{Bucket, Credentials, S3Action, UrlStyle};

use crate::error::{NigelError, Result};
use crate::invoicing::gateway::AssetPublisher;

pub fn object_key(token: &str, filename: &str) -> String {
    format!("i/{token}/{filename}")
}

pub struct R2Publisher {
    pub account_id: String,
    pub access_key: String,
    pub secret_key: String,
    pub bucket: String,
    pub public_base_url: String,
}

impl R2Publisher {
    fn put(&self, key: &str, body: &[u8], content_type: &str) -> Result<()> {
        let endpoint = format!("https://{}.r2.cloudflarestorage.com", self.account_id)
            .parse()
            .map_err(|e| NigelError::Other(format!("r2 endpoint: {e}")))?;
        let bucket = Bucket::new(endpoint, UrlStyle::Path, self.bucket.clone(), "auto")
            .map_err(|e| NigelError::Other(format!("r2 bucket: {e}")))?;
        let creds = Credentials::new(self.access_key.clone(), self.secret_key.clone());

        let action = bucket.put_object(Some(&creds), key);
        let signed = action.sign(Duration::from_secs(300));

        reqwest::blocking::Client::new()
            .put(signed)
            .header("content-type", content_type)
            .body(body.to_vec())
            .send()
            .map_err(|e| NigelError::Other(format!("r2 put: {e}")))?
            .error_for_status()
            .map_err(|e| NigelError::Other(format!("r2 status: {e}")))?;
        Ok(())
    }
}

impl AssetPublisher for R2Publisher {
    fn publish(&self, token: &str, html: &[u8], pdf: &[u8]) -> Result<String> {
        self.put(&object_key(token, "index.html"), html, "text/html; charset=utf-8")?;
        self.put(&object_key(token, "invoice.pdf"), pdf, "application/pdf")?;
        Ok(format!("{}/{}/", self.public_base_url.trim_end_matches('/'), token))
    }
}
```

> Verify the exact `rusty-s3` 0.5 API names (`put_object`, `sign`, `UrlStyle`) at implementation time and adjust to the installed version; the object-key test is the invariant that must hold.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p nigel r2::`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock src/invoicing/r2.rs src/invoicing/mod.rs
git commit -m "feat(invoicing): R2 asset publisher"
```

---

## Task 12: Mailgun mailer

**Files:**
- Create: `src/invoicing/mailgun.rs`
- Modify: `src/invoicing/mod.rs`
- Test: inline (form-field builder)

**Interfaces:**
- Consumes: `Mailer` (Task 9).
- Produces:
  - `mailgun::message_fields(from: &str, to: &str, subject: &str, html: &str) -> Vec<(String, String)>`.
  - `mailgun::MailgunClient { api_key, domain, from }` implementing `Mailer` (multipart with a PDF attachment, `POST https://api.mailgun.net/v3/{domain}/messages`, basic auth `api:{key}`).

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_fields_include_from_to_subject_html() {
        let f = message_fields("billing@rygn.io", "a@b.test", "Invoice #1248", "<p>hi</p>");
        assert!(f.contains(&("from".into(), "billing@rygn.io".into())));
        assert!(f.contains(&("to".into(), "a@b.test".into())));
        assert!(f.contains(&("subject".into(), "Invoice #1248".into())));
        assert!(f.contains(&("html".into(), "<p>hi</p>".into())));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Add `pub mod mailgun;` to `src/invoicing/mod.rs`.
Run: `cargo test -p nigel mailgun:: 2>&1 | head`
Expected: FAIL to compile.

- [ ] **Step 3: Write minimal implementation**

Prepend to `src/invoicing/mailgun.rs`:

```rust
use crate::error::{NigelError, Result};
use crate::invoicing::gateway::Mailer;

pub fn message_fields(from: &str, to: &str, subject: &str, html: &str) -> Vec<(String, String)> {
    vec![
        ("from".into(), from.to_string()),
        ("to".into(), to.to_string()),
        ("subject".into(), subject.to_string()),
        ("html".into(), html.to_string()),
    ]
}

pub struct MailgunClient {
    pub api_key: String,
    pub domain: String,
    pub from: String,
}

impl Mailer for MailgunClient {
    fn send_invoice(&self, to: &str, subject: &str, html: &str, pdf: &[u8]) -> Result<()> {
        let url = format!("https://api.mailgun.net/v3/{}/messages", self.domain);
        let mut form = reqwest::blocking::multipart::Form::new()
            .text("from", self.from.clone())
            .text("to", to.to_string())
            .text("subject", subject.to_string())
            .text("html", html.to_string());
        let part = reqwest::blocking::multipart::Part::bytes(pdf.to_vec())
            .file_name("invoice.pdf")
            .mime_str("application/pdf")
            .map_err(|e| NigelError::Other(e.to_string()))?;
        form = form.part("attachment", part);

        reqwest::blocking::Client::new()
            .post(&url)
            .basic_auth("api", Some(&self.api_key))
            .multipart(form)
            .send()
            .map_err(|e| NigelError::Other(format!("mailgun request: {e}")))?
            .error_for_status()
            .map_err(|e| NigelError::Other(format!("mailgun status: {e}")))?;
        Ok(())
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p nigel mailgun::`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/invoicing/mailgun.rs src/invoicing/mod.rs
git commit -m "feat(invoicing): mailgun mailer"
```

---

## Task 13: `send_invoice` orchestration (abort-leaves-draft)

**Files:**
- Create: `src/invoicing/send.rs`
- Modify: `src/invoicing/mod.rs`, `src/invoicing/invoices.rs` (add small persistence helpers)
- Test: inline with fakes

**Interfaces:**
- Consumes: `PaymentGateway`, `AssetPublisher`, `Mailer`; `invoices::*`, `clients::get_client`, `render_html::render_invoice_html`, `pdf::render_invoice_pdf`.
- Produces:
  - `invoices::set_payment_link(&Connection, id, link_id, url) -> Result<()>`.
  - `invoices::mark_published(&Connection, id, published_at) -> Result<()>` (sets `published_at`, then `refresh_status`).
  - `send::send_invoice<G: PaymentGateway, P: AssetPublisher, M: Mailer>(&Connection, invoice_id, today, &G, &P, &M) -> Result<String>` — returns the public URL. Order: ensure/create+persist payment link → render HTML+PDF → publish → email → `mark_published`. Any error returns early, leaving status `draft`.

- [ ] **Step 1: Write the failing test**

Create `src/invoicing/send.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{get_connection, init_db};
    use crate::error::{NigelError, Result};
    use crate::invoicing::clients::add_client;
    use crate::invoicing::gateway::{AssetPublisher, Mailer, PaidSession, PaymentGateway, PaymentLink};
    use crate::invoicing::invoices::{create_invoice, get_invoice, NewLineItem};
    use crate::migrations::run_migrations;
    use crate::models::{Client, Invoice};
    use std::cell::RefCell;

    fn test_conn() -> (tempfile::TempDir, rusqlite::Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("t.db")).unwrap();
        init_db(&conn).unwrap();
        run_migrations(&conn).unwrap();
        (dir, conn)
    }

    struct FakeGw { create_calls: RefCell<u32> }
    impl PaymentGateway for FakeGw {
        fn create_payment_link(&self, _i: &Invoice, _c: &Client) -> Result<PaymentLink> {
            *self.create_calls.borrow_mut() += 1;
            Ok(PaymentLink { id: "pl_1".into(), url: "https://pay/x".into() })
        }
        fn paid_sessions(&self, _id: &str) -> Result<Vec<PaidSession>> { Ok(vec![]) }
    }
    struct FakePub;
    impl AssetPublisher for FakePub {
        fn publish(&self, token: &str, _h: &[u8], _p: &[u8]) -> Result<String> {
            Ok(format!("https://billing.rygn.io/i/{token}/"))
        }
    }
    struct FailPub;
    impl AssetPublisher for FailPub {
        fn publish(&self, _t: &str, _h: &[u8], _p: &[u8]) -> Result<String> {
            Err(NigelError::Other("upload down".into()))
        }
    }
    struct FakeMail { sent: RefCell<u32> }
    impl Mailer for FakeMail {
        fn send_invoice(&self, _to: &str, _s: &str, _h: &str, _p: &[u8]) -> Result<()> {
            *self.sent.borrow_mut() += 1; Ok(())
        }
    }

    fn seed(conn: &rusqlite::Connection) -> i64 {
        let cid = add_client(conn, "Acme", Some("a@b.test"), None, None).unwrap();
        let items = vec![NewLineItem { description: "W".into(), quantity: 1.0, unit_amount: 100.0 }];
        create_invoice(conn, cid, "2026-08-04", None, "USD", &items, None, None).unwrap()
    }

    #[test]
    fn happy_path_publishes_emails_and_marks_sent() {
        let (_d, conn) = test_conn();
        let id = seed(&conn);
        let gw = FakeGw { create_calls: RefCell::new(0) };
        let mail = FakeMail { sent: RefCell::new(0) };
        let url = send_invoice(&conn, id, "2026-08-04", &gw, &FakePub, &mail).unwrap();
        assert!(url.starts_with("https://billing.rygn.io/i/"));
        let inv = get_invoice(&conn, id).unwrap();
        assert_eq!(inv.status, "sent");
        assert_eq!(inv.stripe_payment_link_id.as_deref(), Some("pl_1"));
        assert_eq!(*mail.sent.borrow(), 1);
    }

    #[test]
    fn publish_failure_leaves_draft_and_sends_no_email() {
        let (_d, conn) = test_conn();
        let id = seed(&conn);
        let gw = FakeGw { create_calls: RefCell::new(0) };
        let mail = FakeMail { sent: RefCell::new(0) };
        let err = send_invoice(&conn, id, "2026-08-04", &gw, &FailPub, &mail);
        assert!(err.is_err());
        assert_eq!(get_invoice(&conn, id).unwrap().status, "draft");
        assert_eq!(*mail.sent.borrow(), 0);
    }

    #[test]
    fn resend_reuses_existing_payment_link() {
        let (_d, conn) = test_conn();
        let id = seed(&conn);
        let gw = FakeGw { create_calls: RefCell::new(0) };
        let mail = FakeMail { sent: RefCell::new(0) };
        send_invoice(&conn, id, "2026-08-04", &gw, &FakePub, &mail).unwrap();
        send_invoice(&conn, id, "2026-08-05", &gw, &FakePub, &mail).unwrap();
        assert_eq!(*gw.create_calls.borrow(), 1); // created once, reused second time
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Add `pub mod send;` to `src/invoicing/mod.rs`.
Run: `cargo test -p nigel --features pdf send:: 2>&1 | head`
Expected: FAIL to compile.

- [ ] **Step 3: Write minimal implementation**

Add helpers to `src/invoicing/invoices.rs`:

```rust
pub fn set_payment_link(conn: &Connection, id: i64, link_id: &str, url: &str) -> Result<()> {
    conn.execute(
        "UPDATE invoices SET stripe_payment_link_id = ?1, stripe_payment_link_url = ?2 WHERE id = ?3",
        rusqlite::params![link_id, url, id],
    )?;
    Ok(())
}

pub fn mark_published(conn: &Connection, id: i64, published_at: &str) -> Result<()> {
    conn.execute(
        "UPDATE invoices SET published_at = ?1 WHERE id = ?2",
        rusqlite::params![published_at, id],
    )?;
    refresh_status(conn, id, published_at)?;
    Ok(())
}
```

Prepend to `src/invoicing/send.rs`:

```rust
use rusqlite::Connection;

use crate::error::{NigelError, Result};
use crate::invoicing::clients::get_client;
use crate::invoicing::gateway::{AssetPublisher, Mailer, PaymentGateway};
use crate::invoicing::invoices::{
    get_invoice, line_items, mark_published, set_payment_link,
};
use crate::invoicing::render_html::render_invoice_html;

pub fn send_invoice<G: PaymentGateway, P: AssetPublisher, M: Mailer>(
    conn: &Connection,
    invoice_id: i64,
    today: &str,
    gateway: &G,
    publisher: &P,
    mailer: &M,
) -> Result<String> {
    let mut invoice = get_invoice(conn, invoice_id)?;
    let client = get_client(conn, invoice.client_id)?;
    let email = client
        .email
        .clone()
        .ok_or_else(|| NigelError::Other(format!("client '{}' has no email", client.name)))?;

    // Create the Stripe link once; reuse on resend.
    if invoice.stripe_payment_link_url.is_none() {
        let link = gateway.create_payment_link(&invoice, &client)?;
        set_payment_link(conn, invoice_id, &link.id, &link.url)?;
        invoice = get_invoice(conn, invoice_id)?;
    }
    let pay_url = invoice.stripe_payment_link_url.clone();

    let items = line_items(conn, invoice_id)?;
    let html = render_invoice_html(&invoice, &client, &items, pay_url.as_deref());
    let pdf = render_pdf(&invoice, &client, &items)?;

    let public_url = publisher.publish(&invoice.token, html.as_bytes(), &pdf)?;

    let subject = format!("Invoice #{} from Raygun", invoice.number);
    mailer.send_invoice(&email, &subject, &html, &pdf)?;

    mark_published(conn, invoice_id, today)?;
    Ok(public_url)
}

#[cfg(feature = "pdf")]
fn render_pdf(
    invoice: &crate::models::Invoice,
    client: &crate::models::Client,
    items: &[crate::models::InvoiceLineItem],
) -> Result<Vec<u8>> {
    crate::pdf::render_invoice_pdf(invoice, client, items)
}

#[cfg(not(feature = "pdf"))]
fn render_pdf(
    _invoice: &crate::models::Invoice,
    _client: &crate::models::Client,
    _items: &[crate::models::InvoiceLineItem],
) -> Result<Vec<u8>> {
    Err(NigelError::Other(
        "PDF support not compiled in (build with --features pdf)".into(),
    ))
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p nigel --features pdf send::`
Expected: PASS (three tests).

- [ ] **Step 5: Commit**

```bash
git add src/invoicing/send.rs src/invoicing/invoices.rs src/invoicing/mod.rs
git commit -m "feat(invoicing): send orchestration with abort-leaves-draft"
```

---

## Task 14: Sync — reconcile Stripe payments

**Files:**
- Create: `src/invoicing/sync.rs`
- Modify: `src/invoicing/mod.rs`
- Test: inline with a fake gateway

**Interfaces:**
- Consumes: `PaymentGateway::paid_sessions`, `invoices::record_payment`.
- Produces: `sync::sync_invoice<G: PaymentGateway>(&Connection, invoice_id, today, &G) -> Result<u32>` returning the count of newly recorded payments; and `sync::sync_all<G>(&Connection, today, &G) -> Result<u32>` iterating invoices with a payment link that are still owing.

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{get_connection, init_db};
    use crate::error::Result;
    use crate::invoicing::clients::add_client;
    use crate::invoicing::gateway::{PaidSession, PaymentGateway, PaymentLink};
    use crate::invoicing::invoices::{create_invoice, get_invoice, paid_amount, set_payment_link, NewLineItem};
    use crate::migrations::run_migrations;
    use crate::models::{Client, Invoice};

    fn test_conn() -> (tempfile::TempDir, rusqlite::Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("t.db")).unwrap();
        init_db(&conn).unwrap();
        run_migrations(&conn).unwrap();
        (dir, conn)
    }

    struct Gw(Vec<PaidSession>);
    impl PaymentGateway for Gw {
        fn create_payment_link(&self, _i: &Invoice, _c: &Client) -> Result<PaymentLink> { unreachable!() }
        fn paid_sessions(&self, _id: &str) -> Result<Vec<PaidSession>> { Ok(self.0.clone()) }
    }

    #[test]
    fn sync_records_once_and_marks_paid() {
        let (_d, conn) = test_conn();
        let cid = add_client(&conn, "Acme", Some("a@b.test"), None, None).unwrap();
        let items = vec![NewLineItem { description: "W".into(), quantity: 1.0, unit_amount: 100.0 }];
        let id = create_invoice(&conn, cid, "2026-08-04", None, "USD", &items, None, None).unwrap();
        set_payment_link(&conn, id, "pl_1", "https://pay/x").unwrap();
        conn.execute("UPDATE invoices SET status='sent', published_at='2026-08-04' WHERE id=?1", [id]).unwrap();

        let gw = Gw(vec![PaidSession { session_id: "cs_1".into(), amount: 100.0 }]);
        assert_eq!(sync_invoice(&conn, id, "2026-08-10", &gw).unwrap(), 1);
        assert_eq!(sync_invoice(&conn, id, "2026-08-11", &gw).unwrap(), 0); // idempotent
        assert_eq!(paid_amount(&conn, id).unwrap(), 100.0);
        assert_eq!(get_invoice(&conn, id).unwrap().status, "paid");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Add `pub mod sync;` to `src/invoicing/mod.rs`.
Run: `cargo test -p nigel sync:: 2>&1 | head`
Expected: FAIL to compile.

- [ ] **Step 3: Write minimal implementation**

Prepend to `src/invoicing/sync.rs`:

```rust
use rusqlite::Connection;

use crate::error::Result;
use crate::invoicing::gateway::PaymentGateway;
use crate::invoicing::invoices::{get_invoice, record_payment};

pub fn sync_invoice<G: PaymentGateway>(
    conn: &Connection,
    invoice_id: i64,
    today: &str,
    gateway: &G,
) -> Result<u32> {
    let invoice = get_invoice(conn, invoice_id)?;
    let link_id = match invoice.stripe_payment_link_id.as_deref() {
        Some(id) => id,
        None => return Ok(0),
    };
    let mut recorded = 0;
    for session in gateway.paid_sessions(link_id)? {
        let is_new = record_payment(
            conn,
            invoice_id,
            session.amount,
            today,
            "stripe",
            Some(&session.session_id),
        )?;
        if is_new {
            recorded += 1;
        }
    }
    Ok(recorded)
}

pub fn sync_all<G: PaymentGateway>(conn: &Connection, today: &str, gateway: &G) -> Result<u32> {
    let mut stmt = conn.prepare(
        "SELECT id FROM invoices
         WHERE stripe_payment_link_id IS NOT NULL AND status IN ('sent','partial','overdue')",
    )?;
    let ids = stmt
        .query_map([], |r| r.get::<_, i64>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let mut total = 0;
    for id in ids {
        total += sync_invoice(conn, id, today, gateway)?;
    }
    Ok(total)
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p nigel sync::`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/invoicing/sync.rs src/invoicing/mod.rs
git commit -m "feat(invoicing): pull-based stripe payment sync"
```

---

## Task 15: InvoiceShelf SQLite importer

**Files:**
- Create: `src/invoicing/import_invoiceshelf.rs`
- Modify: `src/invoicing/mod.rs`
- Test: inline (build a fixture InvoiceShelf-shaped SQLite in a tempdir)

**Interfaces:**
- Produces: `import_invoiceshelf::import(dest: &Connection, invoiceshelf_db: &Path) -> Result<ImportSummary>` where `ImportSummary { clients: u32, invoices: u32, payments: u32, next_number: i64 }`. Reads InvoiceShelf `customers`, `invoices`, `invoice_items`, `payments`; converts integer cents → REAL dollars; sets metadata `next_invoice_number = max(invoice number)+1`.

> The exact InvoiceShelf column names must be confirmed against the live DB during implementation. The test below encodes the mapping this task assumes; adjust both the query and the fixture together if the live schema differs.

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{get_connection, get_metadata, init_db};
    use crate::migrations::run_migrations;

    fn dest_conn() -> (tempfile::TempDir, rusqlite::Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("nigel.db")).unwrap();
        init_db(&conn).unwrap();
        run_migrations(&conn).unwrap();
        (dir, conn)
    }

    fn fixture_invoiceshelf(path: &std::path::Path) {
        let c = rusqlite::Connection::open(path).unwrap();
        c.execute_batch(
            "CREATE TABLE customers (id INTEGER PRIMARY KEY, name TEXT, email TEXT);
             CREATE TABLE invoices (id INTEGER PRIMARY KEY, invoice_number TEXT, customer_id INTEGER,
                invoice_date TEXT, due_date TEXT, total INTEGER, paid_status TEXT, currency_code TEXT);
             CREATE TABLE invoice_items (id INTEGER PRIMARY KEY, invoice_id INTEGER, name TEXT,
                quantity REAL, price INTEGER, total INTEGER);
             CREATE TABLE payments (id INTEGER PRIMARY KEY, invoice_id INTEGER, amount INTEGER, payment_date TEXT);
             INSERT INTO customers VALUES (1,'Acme','a@b.test');
             INSERT INTO invoices VALUES (1,'1247',1,'2026-07-01','2026-07-31',66000,'PAID','USD');
             INSERT INTO invoice_items VALUES (1,1,'Consulting',1,66000,66000);
             INSERT INTO payments VALUES (1,1,66000,'2026-07-15');",
        ).unwrap();
    }

    #[test]
    fn imports_customers_invoices_items_payments_and_sets_next_number() {
        let (_d, dest) = dest_conn();
        let src_dir = tempfile::tempdir().unwrap();
        let src_path = src_dir.path().join("invoiceshelf.sqlite");
        fixture_invoiceshelf(&src_path);

        let summary = import(&dest, &src_path).unwrap();
        assert_eq!(summary.clients, 1);
        assert_eq!(summary.invoices, 1);
        assert_eq!(summary.payments, 1);
        assert_eq!(summary.next_number, 1248);

        // cents -> dollars
        let total: f64 = dest.query_row("SELECT total FROM invoices WHERE number=1247", [], |r| r.get(0)).unwrap();
        assert_eq!(total, 660.0);
        let paid: f64 = dest.query_row("SELECT amount FROM invoice_payments", [], |r| r.get(0)).unwrap();
        assert_eq!(paid, 660.0);
        assert_eq!(get_metadata(&dest, "next_invoice_number").unwrap(), "1248");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Add `pub mod import_invoiceshelf;` to `src/invoicing/mod.rs`.
Run: `cargo test -p nigel import_invoiceshelf 2>&1 | head`
Expected: FAIL to compile.

- [ ] **Step 3: Write minimal implementation**

Prepend to `src/invoicing/import_invoiceshelf.rs`:

```rust
use std::path::Path;

use rusqlite::Connection;

use crate::db::set_metadata;
use crate::error::{NigelError, Result};
use crate::invoicing::invoices::gen_token;

pub struct ImportSummary {
    pub clients: u32,
    pub invoices: u32,
    pub payments: u32,
    pub next_number: i64,
}

fn cents_to_dollars(cents: i64) -> f64 {
    cents as f64 / 100.0
}

pub fn import(dest: &Connection, invoiceshelf_db: &Path) -> Result<ImportSummary> {
    let src = Connection::open(invoiceshelf_db)?;
    let mut summary = ImportSummary { clients: 0, invoices: 0, payments: 0, next_number: 1248 };
    let mut max_number: i64 = 1247;

    // Customers -> clients. Keep a source_id -> dest_id map.
    let mut customer_map = std::collections::HashMap::new();
    {
        let mut stmt = src.prepare("SELECT id, name, email FROM customers")?;
        let rows = stmt
            .query_map([], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, Option<String>>(2)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        for (src_id, name, email) in rows {
            dest.execute(
                "INSERT INTO clients (name, email) VALUES (?1, ?2)",
                rusqlite::params![name, email],
            )?;
            customer_map.insert(src_id, dest.last_insert_rowid());
            summary.clients += 1;
        }
    }

    // Invoices + their items.
    let mut invoice_map = std::collections::HashMap::new();
    {
        let mut stmt = src.prepare(
            "SELECT id, invoice_number, customer_id, invoice_date, due_date, total, paid_status, currency_code FROM invoices",
        )?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, Option<String>>(4)?,
                    r.get::<_, i64>(5)?,
                    r.get::<_, String>(6)?,
                    r.get::<_, Option<String>>(7)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        for (src_id, number_str, cust, issue, due, total_cents, paid_status, currency) in rows {
            let number: i64 = number_str
                .trim()
                .parse()
                .map_err(|_| NigelError::Other(format!("non-numeric invoice number '{number_str}'")))?;
            max_number = max_number.max(number);
            let client_id = *customer_map
                .get(&cust)
                .ok_or_else(|| NigelError::Other(format!("invoice {number} references missing customer {cust}")))?;
            let total = cents_to_dollars(total_cents);
            let status = if paid_status.eq_ignore_ascii_case("PAID") { "paid" } else { "sent" };
            dest.execute(
                "INSERT INTO invoices (number, client_id, issue_date, due_date, status, currency,
                    subtotal, tax, total, token, published_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, ?8, ?9, ?3)",
                rusqlite::params![
                    number, client_id, issue, due, status,
                    currency.unwrap_or_else(|| "USD".into()), total, total, gen_token()
                ],
            )?;
            invoice_map.insert(src_id, dest.last_insert_rowid());
            summary.invoices += 1;

            let mut istmt = src.prepare(
                "SELECT name, quantity, price, total FROM invoice_items WHERE invoice_id = ?1",
            )?;
            let items = istmt
                .query_map([src_id], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)?, r.get::<_, i64>(2)?, r.get::<_, i64>(3)?))
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            let dest_invoice_id = invoice_map[&src_id];
            for (pos, (name, qty, price, line_total)) in items.into_iter().enumerate() {
                dest.execute(
                    "INSERT INTO invoice_line_items (invoice_id, description, quantity, unit_amount, line_total, position)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    rusqlite::params![dest_invoice_id, name, qty, cents_to_dollars(price), cents_to_dollars(line_total), pos as i64],
                )?;
            }
        }
    }

    // Payments.
    {
        let mut stmt = src.prepare("SELECT invoice_id, amount, payment_date FROM payments")?;
        let rows = stmt
            .query_map([], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, String>(2)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        for (src_invoice, amount_cents, date) in rows {
            if let Some(dest_id) = invoice_map.get(&src_invoice) {
                dest.execute(
                    "INSERT INTO invoice_payments (invoice_id, amount, paid_date, method)
                     VALUES (?1, ?2, ?3, 'other')",
                    rusqlite::params![dest_id, cents_to_dollars(amount_cents), date],
                )?;
                summary.payments += 1;
            }
        }
    }

    summary.next_number = max_number + 1;
    set_metadata(dest, "next_invoice_number", &summary.next_number.to_string())?;
    Ok(summary)
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p nigel import_invoiceshelf`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/invoicing/import_invoiceshelf.rs src/invoicing/mod.rs
git commit -m "feat(invoicing): one-time InvoiceShelf SQLite importer"
```

---

## Task 16: CLI surface + dispatch wiring

**Files:**
- Create: `src/cli/client.rs`, `src/cli/invoice.rs`
- Modify: `src/cli/mod.rs` (module decls + `Commands::Client`/`Commands::Invoice` + subcommand enums), `src/main.rs` (dispatch arms)
- Test: `tests/cli_dispatch.rs` (assert_cmd end-to-end on an initialized temp DB)

**Interfaces:**
- Consumes: everything above. Real clients are constructed here from `settings::invoicing_config()`. Commands: `client add|list`, `invoice new|list|show|send|sync|pay|aging|import`.
- Produces: user-facing CLI. `send`/`sync` build `StripeClient`, `R2Publisher`, `MailgunClient` from config; error clearly if required secrets are missing.

- [ ] **Step 1: Write the failing test**

Add to `tests/cli_dispatch.rs`:

```rust
#[test]
fn client_add_and_list_roundtrip() {
    use assert_cmd::Command;
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");

    Command::cargo_bin("nigel").unwrap()
        .args(["init", "--data-dir", data.to_str().unwrap()])
        .assert().success();

    Command::cargo_bin("nigel").unwrap()
        .args(["load", data.to_str().unwrap()])
        .assert().success();

    Command::cargo_bin("nigel").unwrap()
        .args(["client", "add", "Acme Co", "--email", "a@b.test"])
        .assert().success();

    Command::cargo_bin("nigel").unwrap()
        .args(["client", "list"])
        .assert().success()
        .stdout(predicates::str::contains("Acme Co"));
}
```

> If `init`/`load` in this repo require a non-interactive password flag or a specific data-dir convention, mirror what the existing tests in `tests/cli_dispatch.rs` already do for setup.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test cli_dispatch client_add_and_list_roundtrip 2>&1 | head`
Expected: FAIL (unknown subcommand `client`).

- [ ] **Step 3: Write minimal implementation**

In `src/cli/mod.rs`: add `pub mod client;` and `pub mod invoice;`; add two variants to `Commands`:

```rust
    /// Manage clients.
    Client {
        #[command(subcommand)]
        command: ClientCommands,
    },
    /// Create, publish, and track invoices.
    Invoice {
        #[command(subcommand)]
        command: InvoiceCommands,
    },
```

Add the subcommand enums:

```rust
#[derive(Subcommand)]
pub enum ClientCommands {
    /// Add a client.
    Add {
        name: String,
        #[arg(long)]
        email: Option<String>,
        #[arg(long)]
        address: Option<String>,
    },
    /// List clients.
    List,
}

#[derive(Subcommand)]
pub enum InvoiceCommands {
    /// Create a draft invoice. Line items as "desc:qty:unit", repeatable.
    New {
        #[arg(long)]
        client: i64,
        #[arg(long = "issue")]
        issue_date: String,
        #[arg(long = "due")]
        due_date: Option<String>,
        #[arg(long, default_value = "USD")]
        currency: String,
        #[arg(long = "item")]
        items: Vec<String>,
    },
    /// List invoices.
    List,
    /// Show one invoice by number.
    Show { number: i64 },
    /// Render, publish to R2, and email an invoice.
    Send { number: i64 },
    /// Poll Stripe and record any new payments.
    Sync,
    /// Manually record a payment (direct deposit, etc.).
    Pay {
        number: i64,
        #[arg(long)]
        amount: Option<f64>,
        #[arg(long)]
        date: String,
        #[arg(long, default_value = "direct_deposit")]
        method: String,
    },
    /// AR aging report.
    Aging,
    /// One-time import from an InvoiceShelf SQLite file.
    Import {
        #[arg(long = "from-invoiceshelf")]
        db: String,
    },
}
```

In `src/main.rs`, add dispatch arms that parse args and call the CLI wrappers (e.g. `Commands::Client { command } => match command { ... cli::client::add(...) }`, and similarly for `Invoice`). Follow the exact wrapper signatures you define in `src/cli/client.rs` / `src/cli/invoice.rs`.

Create `src/cli/client.rs` (thin wrappers over the data layer, mirroring `src/cli/accounts.rs`):

```rust
use comfy_table::{Cell, Table};

use crate::db::get_connection;
use crate::error::Result;
use crate::invoicing::clients::{add_client, list_clients};
use crate::settings::get_data_dir;

pub fn add(name: &str, email: Option<&str>, address: Option<&str>) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let id = add_client(&conn, name, email, address, None)?;
    println!("Added client {id}: {name}");
    Ok(())
}

pub fn list() -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let mut table = Table::new();
    table.set_header(vec!["ID", "Name", "Email"]);
    for c in list_clients(&conn)? {
        table.add_row(vec![
            Cell::new(c.id),
            Cell::new(c.name),
            Cell::new(c.email.unwrap_or_default()),
        ]);
    }
    println!("Clients\n{table}");
    Ok(())
}
```

Create `src/cli/invoice.rs` with wrappers for `new`, `list`, `show`, `send`, `sync`, `pay`, `aging`, `import`. `new` parses each `desc:qty:unit` item string into `NewLineItem`. `send`/`sync` construct the real clients from `settings::invoicing_config()`, returning a clear error when a required secret is `None`:

```rust
use crate::db::get_connection;
use crate::error::{NigelError, Result};
use crate::invoicing::gateway::{AssetPublisher, Mailer, PaymentGateway};
use crate::invoicing::invoices::{
    ar_aging, create_invoice, get_invoice_by_number, record_payment, NewLineItem,
};
use crate::invoicing::mailgun::MailgunClient;
use crate::invoicing::r2::R2Publisher;
use crate::invoicing::stripe::StripeClient;
use crate::invoicing::{send, sync};
use crate::settings::{get_data_dir, invoicing_config, InvoicingConfig};

fn parse_item(s: &str) -> Result<NewLineItem> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 3 {
        return Err(NigelError::Other(format!("bad --item '{s}', want desc:qty:unit")));
    }
    Ok(NewLineItem {
        description: parts[0].to_string(),
        quantity: parts[1].parse().map_err(|_| NigelError::Other("bad qty".into()))?,
        unit_amount: parts[2].parse().map_err(|_| NigelError::Other("bad unit".into()))?,
    })
}

fn require<T>(v: Option<T>, what: &str) -> Result<T> {
    v.ok_or_else(|| NigelError::Other(format!("missing config: {what}")))
}

fn build_clients(cfg: InvoicingConfig) -> Result<(StripeClient, R2Publisher, MailgunClient)> {
    let stripe = StripeClient { secret_key: require(cfg.stripe_secret_key, "stripe_secret_key")? };
    let r2 = R2Publisher {
        account_id: require(cfg.r2_account_id, "r2_account_id")?,
        access_key: require(cfg.r2_access_key, "r2_access_key")?,
        secret_key: require(cfg.r2_secret_key, "r2_secret_key")?,
        bucket: require(cfg.r2_bucket, "r2_bucket")?,
        public_base_url: cfg.public_base_url,
    };
    let mail = MailgunClient {
        api_key: require(cfg.mailgun_api_key, "mailgun_api_key")?,
        domain: cfg.mailgun_domain,
        from: cfg.from_email,
    };
    Ok((stripe, r2, mail))
}

pub fn new(client: i64, issue: &str, due: Option<&str>, currency: &str, items: &[String]) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let parsed: Result<Vec<_>> = items.iter().map(|s| parse_item(s)).collect();
    let id = create_invoice(&conn, client, issue, due, currency, &parsed?, None, None)?;
    let number = get_invoice_by_number_from_id(&conn, id)?;
    println!("Created draft invoice #{number}");
    Ok(())
}

fn get_invoice_by_number_from_id(conn: &rusqlite::Connection, id: i64) -> Result<i64> {
    Ok(crate::invoicing::invoices::get_invoice(conn, id)?.number)
}

pub fn send(number: i64, today: &str) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let invoice = get_invoice_by_number(&conn, number)?;
    let (stripe, r2, mail) = build_clients(invoicing_config())?;
    let url = send::send_invoice(&conn, invoice.id, today, &stripe, &r2, &mail)?;
    println!("Sent invoice #{number}: {url}");
    Ok(())
}

pub fn sync(today: &str) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let (stripe, _r2, _mail) = build_clients(invoicing_config())?;
    let n = sync::sync_all(&conn, today, &stripe)?;
    println!("Recorded {n} new payment(s)");
    Ok(())
}

pub fn pay(number: i64, amount: Option<f64>, date: &str, method: &str) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let invoice = get_invoice_by_number(&conn, number)?;
    let paid = crate::invoicing::invoices::paid_amount(&conn, invoice.id)?;
    let amt = amount.unwrap_or(invoice.total - paid);
    record_payment(&conn, invoice.id, amt, date, method, None)?;
    println!("Recorded {amt:.2} against invoice #{number}");
    Ok(())
}

pub fn aging(today: &str) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    for b in ar_aging(&conn, today)? {
        println!("{:>8}: {:.2}", b.label, b.total);
    }
    Ok(())
}

pub fn import(db: &str) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let s = crate::invoicing::import_invoiceshelf::import(&conn, std::path::Path::new(db))?;
    println!("Imported {} clients, {} invoices, {} payments. Next number: {}", s.clients, s.invoices, s.payments, s.next_number);
    Ok(())
}

pub fn list() -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let mut stmt = conn.prepare("SELECT number, status, total FROM invoices ORDER BY number DESC")?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, f64>(2)?)))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    for (num, status, total) in rows {
        println!("#{num}  {status:<8} {total:.2}");
    }
    Ok(())
}

pub fn show(number: i64) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let inv = get_invoice_by_number(&conn, number)?;
    println!("Invoice #{}  [{}]  {} {:.2}", inv.number, inv.status, inv.currency, inv.total);
    if let Some(url) = inv.stripe_payment_link_url { println!("Pay: {url}"); }
    Ok(())
}
```

Wire the dispatch arms in `src/main.rs`. For `today`, use `chrono::Local::now().format("%Y-%m-%d").to_string()`.

> `_r2`/`_mail` are intentionally unused in `sync`; keep them or split `build_clients` if the compiler's unused-variable lint is denied in this repo.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test cli_dispatch client_add_and_list_roundtrip`
Expected: PASS.

- [ ] **Step 5: Full build + suite**

Run: `cargo build --features pdf && cargo test --features pdf`
Expected: PASS (whole suite).

- [ ] **Step 6: Commit**

```bash
git add src/cli/mod.rs src/cli/client.rs src/cli/invoice.rs src/main.rs tests/cli_dispatch.rs
git commit -m "feat(invoicing): CLI commands for clients and invoices"
```

---

## Task 17: On-launch sync hook + docs

**Files:**
- Modify: `src/main.rs` (best-effort sync notice, mirroring the existing update-check pattern)
- Modify: `docs/` (add an invoicing usage doc), `CLAUDE.md` if it lists commands
- Test: manual (documented below)

**Interfaces:**
- Consumes: `sync::sync_all`. Non-fatal: a sync error prints a notice and never blocks the command.

- [ ] **Step 1: Add a best-effort launch sync**

In `src/main.rs`, near the existing `cli::update::check_and_notify()` call, add a guarded sync for data-bearing commands only (skip `Init`/`Demo`/`Load`/`Update`/`Completions`). It must never abort the real command:

```rust
// Best-effort: reconcile Stripe payments in the background of interactive use.
if let Ok(cfg) = std::panic::catch_unwind(crate::settings::invoicing_config) {
    if cfg.stripe_secret_key.is_some() {
        if let Ok(conn) = crate::db::get_connection(&crate::settings::get_data_dir().join("nigel.db")) {
            let today = chrono::Local::now().format("%Y-%m-%d").to_string();
            let client = crate::invoicing::stripe::StripeClient { secret_key: cfg.stripe_secret_key.unwrap() };
            match crate::invoicing::sync::sync_all(&conn, &today, &client) {
                Ok(n) if n > 0 => eprintln!("notice: recorded {n} new invoice payment(s)"),
                Ok(_) => {}
                Err(e) => eprintln!("notice: invoice sync skipped: {e}"),
            }
        }
    }
}
```

> Place this so it only runs for subcommands that require an initialized DB (reuse the same guard set the password prompt uses). Keep it out of the hot path for `completions`.

- [ ] **Step 2: Verify build**

Run: `cargo build --features pdf`
Expected: builds clean.

- [ ] **Step 3: Write the usage doc**

Create `docs/invoicing.md` describing: configuring secrets (env vars / settings.json keys), `client add`, `invoice new --item desc:qty:unit`, `invoice send`, `invoice sync`, `invoice pay`, `invoice aging`, `invoice import --from-invoiceshelf`, and the Cloudflare `billing.rygn.io/i/*` → R2 routing. Describe current behavior only (no migration history).

- [ ] **Step 4: Manual smoke (documented, not automated)**

```bash
# with test-mode secrets exported:
cargo run --features pdf -- client add "Test Client" --email you@example.com
cargo run --features pdf -- invoice new --client 1 --issue 2026-08-04 --item "Consulting:1:100"
cargo run --features pdf -- invoice send 1248     # publishes to R2, emails, creates Stripe link
# pay via the link with card 4242 4242 4242 4242, then:
cargo run --features pdf -- invoice sync          # records the payment, flips to paid
cargo run --features pdf -- invoice aging
```

- [ ] **Step 5: Commit**

```bash
git add src/main.rs docs/invoicing.md CLAUDE.md
git commit -m "feat(invoicing): launch-time sync notice and usage docs"
```

---

## Self-Review

**Spec coverage:**
- R2 static publishing → Tasks 7 (HTML), 8 (PDF), 11 (R2), 13 (send). ✓
- `billing.rygn.io/i/{token}/` routing → `public_base_url` default (Task 6), object-key `i/{token}/…` (Task 11), doc (Task 17). ✓
- Stripe Payment Link always attached + pull sync → Tasks 10, 13, 14. ✓
- AR-lite, journal deferred → Tasks 1–5 (no `journal_entries` table). ✓
- Manual mark-paid for direct deposit → Task 4 (`record_payment`), Task 16 (`invoice pay`). ✓
- Email via Mailgun → Tasks 12, 13, 16. ✓
- Numbering `max+1`, seeded 1248 → Task 3 (`next_number`), Task 15 (import sets it). ✓
- Full-fidelity InvoiceShelf import → Task 15. ✓
- Money as `f64`, cents only at Stripe → Global Constraints, Task 10 `to_cents`, Task 15 `cents_to_dollars`. ✓
- Derived status incl. overdue → Task 4. ✓
- Abort-leaves-draft send → Task 13. ✓
- Offline tests → every task's tests avoid network; external clients tested via builders/parsers + trait fakes. ✓

**Placeholder scan:** No `TODO`/`TBD`/"handle errors appropriately". The three "verify at implementation time" notes (migration version number, `rusty-s3` 0.5 API, InvoiceShelf column names) are genuine external-fact confirmations, each with the invariant its test pins — not deferred work.

**Type consistency:** `NewLineItem`, `PaymentLink`, `PaidSession`, `InvoicingConfig`, `ImportSummary`, and the three traits are defined once and referenced with matching signatures across tasks. `record_payment` returns `bool` (new vs duplicate) consistently in Tasks 4, 14. `send_invoice`/`sync_*` generic bounds match the trait names in Task 9.

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-08-04-nigel-invoicing.md`.
