use std::path::Path;

use rusqlite::Connection;
use sha2::{Digest, Sha256};

use crate::error::{NigelError, Result};
use crate::models::ParsedRow;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

pub fn parse_amount(raw: &str) -> f64 {
    let s = raw.replace(',', "").replace('"', "").replace('$', "");
    let s = s.trim();
    if let Some(inner) = s.strip_prefix('(').and_then(|v| v.strip_suffix(')')) {
        return -inner.trim().parse::<f64>().unwrap_or(0.0);
    }
    s.parse().unwrap_or(0.0)
}

pub fn parse_date_mdy(raw: &str) -> Option<String> {
    let raw = raw.trim();
    let parts: Vec<&str> = raw.split('/').collect();
    if parts.len() != 3 {
        return None;
    }
    let m: u32 = parts[0].parse().ok()?;
    let d: u32 = parts[1].parse().ok()?;
    let y: i32 = parts[2].parse().ok()?;
    chrono::NaiveDate::from_ymd_opt(y, m as u32, d as u32)
        .map(|dt| dt.format("%Y-%m-%d").to_string())
}

#[cfg(any(feature = "gusto", test))]
pub fn excel_serial_to_date(serial: f64) -> String {
    // Excel epoch is 1899-12-30 (accounting for the 1900 leap year bug)
    let base = chrono::NaiveDate::from_ymd_opt(1899, 12, 30).unwrap();
    let date = base + chrono::Duration::days(serial as i64);
    date.format("%Y-%m-%d").to_string()
}

fn compute_checksum(file_path: &Path) -> Result<String> {
    let data = std::fs::read(file_path)?;
    let mut hasher = Sha256::new();
    hasher.update(&data);
    Ok(hex::encode(hasher.finalize()))
}

fn is_duplicate_row(conn: &Connection, account_id: i64, row: &ParsedRow) -> bool {
    let mut stmt = conn
        .prepare_cached(
            "SELECT 1 FROM transactions WHERE account_id = ?1 AND date = ?2 AND amount = ?3 AND description = ?4",
        )
        .unwrap();
    stmt.exists(rusqlite::params![account_id, row.date, row.amount, row.description])
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Importer kinds — enum dispatch instead of trait objects
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ImporterKind {
    BofaChecking,
    BofaCreditCard,
    BofaLineOfCredit,
    #[cfg(feature = "gusto")]
    GustoPayroll,
}

impl ImporterKind {
    pub fn key(&self) -> &'static str {
        match self {
            Self::BofaChecking => "bofa_checking",
            Self::BofaCreditCard => "bofa_credit_card",
            Self::BofaLineOfCredit => "bofa_line_of_credit",
            #[cfg(feature = "gusto")]
            Self::GustoPayroll => "gusto_payroll",
        }
    }

    #[allow(dead_code)]
    pub fn name(&self) -> &'static str {
        match self {
            Self::BofaChecking => "Bank of America Checking",
            Self::BofaCreditCard => "Bank of America Credit Card",
            Self::BofaLineOfCredit => "Bank of America Line of Credit",
            #[cfg(feature = "gusto")]
            Self::GustoPayroll => "Gusto Payroll",
        }
    }

    pub fn account_types(&self) -> &[&str] {
        match self {
            Self::BofaChecking => &["checking"],
            Self::BofaCreditCard => &["credit_card"],
            Self::BofaLineOfCredit => &["line_of_credit"],
            #[cfg(feature = "gusto")]
            Self::GustoPayroll => &["payroll"],
        }
    }

    pub fn detect(&self, file_path: &Path) -> bool {
        match self {
            Self::BofaChecking => detect_bofa_checking(file_path),
            Self::BofaCreditCard => detect_bofa_credit_card(file_path),
            Self::BofaLineOfCredit => false, // differentiated by account_type
            #[cfg(feature = "gusto")]
            Self::GustoPayroll => detect_gusto_payroll(file_path),
        }
    }

    pub fn parse(&self, file_path: &Path) -> Result<Vec<ParsedRow>> {
        match self {
            Self::BofaChecking => parse_bofa_checking(file_path),
            Self::BofaCreditCard => parse_bofa_credit_card(file_path),
            Self::BofaLineOfCredit => parse_bofa_line_of_credit(file_path),
            #[cfg(feature = "gusto")]
            Self::GustoPayroll => parse_gusto_payroll(file_path),
        }
    }

    pub fn has_post_import(&self) -> bool {
        match self {
            #[cfg(feature = "gusto")]
            Self::GustoPayroll => true,
            _ => false,
        }
    }

    #[allow(unused_variables)]
    pub fn post_import(&self, conn: &Connection, account_id: i64, rows: &[ParsedRow]) -> Result<()> {
        match self {
            #[cfg(feature = "gusto")]
            Self::GustoPayroll => auto_categorize_payroll(conn, account_id, rows),
            _ => Ok(()),
        }
    }
}

const ALL_IMPORTERS: &[ImporterKind] = &[
    ImporterKind::BofaChecking,
    ImporterKind::BofaCreditCard,
    ImporterKind::BofaLineOfCredit,
    #[cfg(feature = "gusto")]
    ImporterKind::GustoPayroll,
];

pub fn get_by_key(key: &str) -> Option<ImporterKind> {
    ALL_IMPORTERS.iter().find(|i| i.key() == key).copied()
}

pub fn get_for_file(account_type: &str, file_path: &Path) -> Option<ImporterKind> {
    let candidates: Vec<_> = ALL_IMPORTERS
        .iter()
        .filter(|i| i.account_types().contains(&account_type))
        .collect();
    // Try detect first
    for imp in &candidates {
        if imp.detect(file_path) {
            return Some(**imp);
        }
    }
    // Fallback to first match
    candidates.first().map(|i| **i)
}

// ---------------------------------------------------------------------------
// import_file
// ---------------------------------------------------------------------------

pub struct ImportResult {
    pub imported: usize,
    pub skipped: usize,
    pub duplicate_file: bool,
}

pub fn import_file(
    conn: &Connection,
    file_path: &Path,
    account_name: &str,
    format_key: Option<&str>,
) -> Result<ImportResult> {
    let (account_id, account_type) = {
        let mut stmt = conn.prepare("SELECT id, account_type FROM accounts WHERE name = ?1")?;
        let row = stmt
            .query_row([account_name], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|_| NigelError::UnknownAccount(account_name.to_string()))?;
        row
    };

    let checksum = compute_checksum(file_path)?;
    {
        let mut stmt = conn.prepare("SELECT 1 FROM imports WHERE checksum = ?1 AND account_id = ?2")?;
        if stmt.exists(rusqlite::params![checksum, account_id])? {
            return Ok(ImportResult {
                imported: 0,
                skipped: 0,
                duplicate_file: true,
            });
        }
    }

    let importer = if let Some(key) = format_key {
        get_by_key(key).ok_or_else(|| NigelError::UnknownFormat(key.to_string()))?
    } else {
        get_for_file(&account_type, file_path)
            .ok_or_else(|| NigelError::NoImporter(account_type.clone()))?
    };

    let parsed_rows = importer.parse(file_path)?;

    let mut imported = 0usize;
    let mut skipped = 0usize;
    for row in &parsed_rows {
        if is_duplicate_row(conn, account_id, row) {
            skipped += 1;
            continue;
        }
        conn.execute(
            "INSERT INTO transactions (account_id, date, description, amount, is_flagged, flag_reason) VALUES (?1, ?2, ?3, ?4, 1, 'No matching rule')",
            rusqlite::params![account_id, row.date, row.description, row.amount],
        )?;
        imported += 1;
    }

    let dates: Vec<&str> = parsed_rows.iter().map(|r| r.date.as_str()).collect();
    let min_date = dates.iter().min().copied();
    let max_date = dates.iter().max().copied();
    conn.execute(
        "INSERT INTO imports (filename, account_id, record_count, date_range_start, date_range_end, checksum) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![
            file_path.file_name().and_then(|n| n.to_str()).unwrap_or(""),
            account_id,
            parsed_rows.len() as i64,
            min_date,
            max_date,
            checksum,
        ],
    )?;

    if importer.has_post_import() {
        importer.post_import(conn, account_id, &parsed_rows)?;
    }

    Ok(ImportResult {
        imported,
        skipped,
        duplicate_file: false,
    })
}

// ---------------------------------------------------------------------------
// BofA Checking parser
// ---------------------------------------------------------------------------

fn detect_bofa_checking(file_path: &Path) -> bool {
    let Ok(file) = std::fs::File::open(file_path) else {
        return false;
    };
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(std::io::BufReader::new(file));
    for result in rdr.records() {
        let Ok(record) = result else { continue };
        if record.len() >= 4
            && record[0].trim() == "Date"
            && record[1].contains("Description")
        {
            return true;
        }
    }
    false
}

fn parse_bofa_checking(file_path: &Path) -> Result<Vec<ParsedRow>> {
    let file = std::fs::File::open(file_path)?;
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(std::io::BufReader::new(file));
    let mut rows = Vec::new();
    let mut found_header = false;

    for result in rdr.records() {
        let Ok(record) = result else { continue };
        if !found_header {
            if record.len() >= 4
                && record[0].trim() == "Date"
                && record[1].contains("Description")
            {
                found_header = true;
            }
            continue;
        }
        if record.len() < 3 || record[0].trim().is_empty() {
            continue;
        }
        let Some(date) = parse_date_mdy(&record[0]) else {
            continue;
        };
        let description = record[1].trim().to_string();
        if description.is_empty() || description.contains("Beginning balance") {
            continue;
        }
        let amount = parse_amount(&record[2]);
        rows.push(ParsedRow {
            date,
            description,
            amount,
        });
    }
    Ok(rows)
}

// ---------------------------------------------------------------------------
// BofA Credit Card parser
// ---------------------------------------------------------------------------

fn detect_bofa_credit_card(file_path: &Path) -> bool {
    let Ok(content) = std::fs::read_to_string(file_path) else {
        return false;
    };
    content.contains("CardHolder Name")
}

fn parse_bofa_credit_card(file_path: &Path) -> Result<Vec<ParsedRow>> {
    let file = std::fs::File::open(file_path)?;
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(std::io::BufReader::new(file));
    let mut rows = Vec::new();
    let mut found_header = false;
    let (mut idx_date, mut idx_desc, mut idx_amount, mut idx_type) = (3, 5, 6, 9);

    for result in rdr.records() {
        let Ok(record) = result else { continue };
        if !found_header {
            if record.iter().any(|f| f.contains("Posting Date")) {
                // Validate expected column positions from header
                for (i, field) in record.iter().enumerate() {
                    let f = field.trim();
                    if f == "Posting Date" { idx_date = i; }
                    if f == "Payee" { idx_desc = i; }
                    if f == "Amount" { idx_amount = i; }
                    if f == "Type" { idx_type = i; }
                }
                found_header = true;
            }
            continue;
        }
        let min_cols = [idx_date, idx_desc, idx_amount, idx_type].into_iter().max().unwrap_or(0) + 1;
        if record.len() < min_cols || record[2].trim().is_empty() {
            continue;
        }
        let Some(date) = parse_date_mdy(&record[idx_date]) else {
            continue;
        };
        let description = record[idx_desc].trim().to_string();
        let mut amount = parse_amount(&record[idx_amount]);
        let txn_type = record[idx_type].trim();
        if txn_type == "D" {
            amount = -amount.abs();
        } else {
            amount = amount.abs();
        }
        rows.push(ParsedRow {
            date,
            description,
            amount,
        });
    }
    Ok(rows)
}

// ---------------------------------------------------------------------------
// BofA Line of Credit parser
// ---------------------------------------------------------------------------

fn parse_bofa_line_of_credit(file_path: &Path) -> Result<Vec<ParsedRow>> {
    let file = std::fs::File::open(file_path)?;
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(std::io::BufReader::new(file));
    let mut rows = Vec::new();
    let mut found_header = false;
    let (mut idx_date, mut idx_desc, mut idx_amount) = (3, 5, 6);

    for result in rdr.records() {
        let Ok(record) = result else { continue };
        if !found_header {
            if record.iter().any(|f| f.contains("Posting Date")) {
                for (i, field) in record.iter().enumerate() {
                    let f = field.trim();
                    if f == "Posting Date" { idx_date = i; }
                    if f == "Payee" { idx_desc = i; }
                    if f == "Amount" { idx_amount = i; }
                }
                found_header = true;
            }
            continue;
        }
        let min_cols = [idx_date, idx_desc, idx_amount].into_iter().max().unwrap_or(0) + 1;
        if record.len() < min_cols || record[2].trim().is_empty() {
            continue;
        }
        let Some(date) = parse_date_mdy(&record[idx_date]) else {
            continue;
        };
        let description = record[idx_desc].trim().to_string();
        let amount = -parse_amount(&record[idx_amount]);
        rows.push(ParsedRow {
            date,
            description,
            amount,
        });
    }
    Ok(rows)
}

// ---------------------------------------------------------------------------
// Gusto Payroll parser (feature-gated)
// ---------------------------------------------------------------------------

#[cfg(feature = "gusto")]
fn detect_gusto_payroll(file_path: &Path) -> bool {
    use calamine::Reader;
    if !file_path
        .extension()
        .map_or(false, |e| e.eq_ignore_ascii_case("xlsx"))
    {
        return false;
    }
    let Ok(workbook) = calamine::open_workbook_auto(file_path) else {
        return false;
    };
    workbook.sheet_names().iter().any(|name| name == "payrolls")
}

#[cfg(feature = "gusto")]
fn parse_gusto_payroll(file_path: &Path) -> Result<Vec<ParsedRow>> {
    use calamine::{Data, Reader};
    use std::collections::BTreeMap;

    let mut workbook = calamine::open_workbook_auto(file_path)
        .map_err(|e| NigelError::Other(format!("Failed to open XLSX: {e}")))?;

    // Parse payrolls sheet — aggregate wages by check date
    let mut wages_by_date: BTreeMap<String, f64> = BTreeMap::new();
    if let Ok(range) = workbook.worksheet_range("payrolls") {
        for row in range.rows().skip(1) {
            // col 3 = check_date, col 7 = gross
            if row.len() < 8 {
                continue;
            }
            let check_date = match &row[3] {
                Data::Float(f) => excel_serial_to_date(*f),
                Data::Int(i) => excel_serial_to_date(*i as f64),
                Data::String(s) => s.clone(),
                _ => continue,
            };
            let gross = match &row[7] {
                Data::Float(f) => *f,
                Data::Int(i) => *i as f64,
                _ => continue,
            };
            *wages_by_date.entry(check_date).or_default() += gross;
        }
    }

    // Parse taxes sheet — aggregate employer taxes by check date
    let mut taxes_by_date: BTreeMap<String, f64> = BTreeMap::new();
    if let Ok(range) = workbook.worksheet_range("taxes") {
        for row in range.rows().skip(1) {
            if row.len() < 8 {
                continue;
            }
            // col 6 = type (Employer), col 3 = check_date, col 7 = amount
            let tax_type = match &row[6] {
                Data::String(s) => s.as_str(),
                _ => continue,
            };
            if tax_type != "Employer" {
                continue;
            }
            let check_date = match &row[3] {
                Data::Float(f) => excel_serial_to_date(*f),
                Data::Int(i) => excel_serial_to_date(*i as f64),
                Data::String(s) => s.clone(),
                _ => continue,
            };
            let amount = match &row[7] {
                Data::Float(f) => *f,
                Data::Int(i) => *i as f64,
                _ => continue,
            };
            *taxes_by_date.entry(check_date).or_default() += amount;
        }
    }

    let mut result = Vec::new();
    for (date, total) in &wages_by_date {
        result.push(ParsedRow {
            date: date.clone(),
            description: format!("Payroll \u{2014} Wages ({date})"),
            amount: -total.abs(),
        });
    }
    for (date, total) in &taxes_by_date {
        result.push(ParsedRow {
            date: date.clone(),
            description: format!("Payroll \u{2014} Employer Taxes ({date})"),
            amount: -total.abs(),
        });
    }
    Ok(result)
}

#[cfg(feature = "gusto")]
fn auto_categorize_payroll(conn: &Connection, account_id: i64, rows: &[ParsedRow]) -> Result<()> {
    let mut payroll_categories = std::collections::HashMap::new();
    for cat_name in &[
        "Payroll \u{2014} Wages",
        "Payroll \u{2014} Taxes",
        "Payroll \u{2014} Benefits",
    ] {
        let id: std::result::Result<i64, _> =
            conn.query_row("SELECT id FROM categories WHERE name = ?1", [cat_name], |r| {
                r.get(0)
            });
        if let Ok(id) = id {
            payroll_categories.insert(*cat_name, id);
        }
    }

    for row in rows {
        let category_id = if row.description.contains("Wages") {
            payroll_categories.get("Payroll \u{2014} Wages")
        } else if row.description.contains("Taxes") {
            payroll_categories.get("Payroll \u{2014} Taxes")
        } else if row.description.contains("Benefits") {
            payroll_categories.get("Payroll \u{2014} Benefits")
        } else {
            None
        };

        if let Some(&cat_id) = category_id {
            conn.execute(
                "UPDATE transactions SET category_id = ?1, is_flagged = 0, flag_reason = NULL \
                 WHERE account_id = ?2 AND date = ?3 AND amount = ?4 AND description = ?5",
                rusqlite::params![cat_id, account_id, row.date, row.amount, row.description],
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{get_connection, init_db};

    fn test_db() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("test.db")).unwrap();
        init_db(&conn).unwrap();
        (dir, conn)
    }

    fn add_test_account(conn: &Connection) {
        conn.execute(
            "INSERT INTO accounts (name, account_type) VALUES ('Test Checking', 'checking')", [],
        ).unwrap();
    }

    fn write_bofa_csv(dir: &Path, name: &str, rows: &[(&str, &str, &str)]) -> std::path::PathBuf {
        let path = dir.join(name);
        let mut content = String::from("Date,Description,Amount,Running Bal.\n");
        for (date, desc, amt) in rows {
            content.push_str(&format!("{date},{desc},{amt},0.00\n"));
        }
        std::fs::write(&path, &content).unwrap();
        path
    }

    #[test]
    fn test_parse_amount() {
        assert_eq!(parse_amount("1,234.56"), 1234.56);
        assert_eq!(parse_amount("\"500.00\""), 500.0);
        assert_eq!(parse_amount("  -42.50  "), -42.5);
        assert_eq!(parse_amount("0"), 0.0);
        assert_eq!(parse_amount("not_a_number"), 0.0);
    }

    #[test]
    fn test_parse_amount_parenthesized_negatives() {
        assert_eq!(parse_amount("(500.00)"), -500.0);
        assert_eq!(parse_amount("(1,234.56)"), -1234.56);
        assert_eq!(parse_amount("\"(50.00)\""), -50.0);
    }

    #[test]
    fn test_parse_amount_currency_symbol() {
        assert_eq!(parse_amount("$1,234.56"), 1234.56);
        assert_eq!(parse_amount("-$50.00"), -50.0);
    }

    #[test]
    fn test_parse_date_mdy() {
        assert_eq!(parse_date_mdy("01/15/2025"), Some("2025-01-15".to_string()));
        assert_eq!(parse_date_mdy("12/01/2024"), Some("2024-12-01".to_string()));
        assert_eq!(parse_date_mdy("invalid"), None);
        assert_eq!(parse_date_mdy("2025-01-15"), None);
    }

    #[test]
    fn test_parse_date_mdy_rejects_invalid_dates() {
        assert_eq!(parse_date_mdy("13/01/2025"), None); // month 13
        assert_eq!(parse_date_mdy("02/30/2025"), None); // Feb 30
        assert_eq!(parse_date_mdy("00/15/2025"), None); // month 0
    }

    #[test]
    fn test_excel_serial_to_date() {
        assert_eq!(excel_serial_to_date(45667.0), "2025-01-10");
    }

    #[test]
    fn test_import_file_inserts_transactions() {
        let (dir, conn) = test_db();
        add_test_account(&conn);
        let csv_path = write_bofa_csv(dir.path(), "stmt.csv", &[
            ("01/15/2025", "PAYMENT ONE", "-100.00"),
            ("01/16/2025", "PAYMENT TWO", "-250.00"),
            ("01/17/2025", "DEPOSIT", "500.00"),
        ]);
        let result = import_file(&conn, &csv_path, "Test Checking", Some("bofa_checking")).unwrap();
        assert_eq!(result.imported, 3);
        assert!(!result.duplicate_file);
        let count: i64 = conn.query_row("SELECT count(*) FROM transactions", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 3);
    }

    #[test]
    fn test_import_file_detects_file_duplicate() {
        let (dir, conn) = test_db();
        add_test_account(&conn);
        let csv_path = write_bofa_csv(dir.path(), "stmt.csv", &[
            ("01/15/2025", "PAYMENT ONE", "-100.00"),
        ]);
        let r1 = import_file(&conn, &csv_path, "Test Checking", Some("bofa_checking")).unwrap();
        assert_eq!(r1.imported, 1);
        let r2 = import_file(&conn, &csv_path, "Test Checking", Some("bofa_checking")).unwrap();
        assert!(r2.duplicate_file);
        assert_eq!(r2.imported, 0);
    }

    #[test]
    fn test_import_file_detects_row_duplicates() {
        let (dir, conn) = test_db();
        add_test_account(&conn);
        let csv1 = write_bofa_csv(dir.path(), "stmt1.csv", &[
            ("01/15/2025", "PAYMENT ONE", "-100.00"),
            ("01/16/2025", "PAYMENT TWO", "-200.00"),
        ]);
        import_file(&conn, &csv1, "Test Checking", Some("bofa_checking")).unwrap();
        let csv2 = write_bofa_csv(dir.path(), "stmt2.csv", &[
            ("01/16/2025", "PAYMENT TWO", "-200.00"),
            ("01/18/2025", "PAYMENT THREE", "-300.00"),
        ]);
        let r2 = import_file(&conn, &csv2, "Test Checking", Some("bofa_checking")).unwrap();
        assert_eq!(r2.imported, 1);
        assert_eq!(r2.skipped, 1);
    }

    #[test]
    fn test_import_file_records_batch() {
        let (dir, conn) = test_db();
        add_test_account(&conn);
        let csv_path = write_bofa_csv(dir.path(), "stmt.csv", &[
            ("01/15/2025", "PAYMENT ONE", "-100.00"),
        ]);
        import_file(&conn, &csv_path, "Test Checking", Some("bofa_checking")).unwrap();
        let count: i64 = conn.query_row("SELECT count(*) FROM imports", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 1);
        let record_count: i64 = conn.query_row(
            "SELECT record_count FROM imports LIMIT 1", [], |r| r.get(0),
        ).unwrap();
        assert_eq!(record_count, 1);
    }

    #[test]
    fn test_bofa_checking_parse_quoted_amounts() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bofa.csv");
        let content = "\
Description,,Summary Amt.

Date,Description,Amount,Running Bal.
01/31/2025,BKOFAMERICA MOBILE DEPOSIT,\"2,000.00\",\"32,742.87\"
";
        std::fs::write(&path, content).unwrap();
        let rows = ImporterKind::BofaChecking.parse(&path).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].amount, 2000.0);
    }

    #[test]
    fn test_bofa_checking_parse() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bofa.csv");
        let content = "\
Account Name: Test Checking
Account Number: ****1234

Date,Description,Amount,Running Bal.
01/15/2025,ADOBE CREATIVE,-50.00,950.00
01/16/2025,Beginning balance,1000.00,1000.00
01/17/2025,STRIPE PAYOUT,2500.00,3450.00
";
        std::fs::write(&path, content).unwrap();
        let rows = ImporterKind::BofaChecking.parse(&path).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].description, "ADOBE CREATIVE");
        assert_eq!(rows[0].amount, -50.0);
        assert_eq!(rows[1].description, "STRIPE PAYOUT");
        assert_eq!(rows[1].amount, 2500.0);
    }
}
