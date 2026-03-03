use std::io::BufRead;
use std::path::Path;

use rusqlite::Connection;
use sha2::{Digest, Sha256};

use crate::error::{NigelError, Result};
use crate::models::ParsedRow;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

pub fn parse_amount(raw: &str) -> Option<f64> {
    let s = raw.replace([',', '"', '$'], "");
    let s = s.trim();
    if let Some(inner) = s.strip_prefix('(').and_then(|v| v.strip_suffix(')')) {
        return Some(-inner.trim().parse::<f64>().ok()?);
    }
    s.parse().ok()
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
    chrono::NaiveDate::from_ymd_opt(y, m, d).map(|dt| dt.format("%Y-%m-%d").to_string())
}

#[cfg(any(feature = "gusto", test))]
pub fn excel_serial_to_date(serial: f64) -> String {
    // Excel epoch is 1899-12-30 (accounting for the 1900 leap year bug)
    // unwrap safe: 1899-12-30 is a valid date constant
    let base = chrono::NaiveDate::from_ymd_opt(1899, 12, 30).unwrap();
    let date = base + chrono::Duration::days(serial as i64);
    date.format("%Y-%m-%d").to_string()
}

fn create_csv_reader(file_path: &Path) -> Result<csv::Reader<std::io::BufReader<std::fs::File>>> {
    let file = std::fs::File::open(file_path)?;
    Ok(csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(std::io::BufReader::new(file)))
}

fn compute_checksum(file_path: &Path) -> Result<String> {
    let data = std::fs::read(file_path)?;
    let mut hasher = Sha256::new();
    hasher.update(&data);
    Ok(hex::encode(hasher.finalize()))
}

fn is_duplicate_row(conn: &Connection, account_id: i64, row: &ParsedRow) -> Result<bool> {
    let mut stmt = conn
        .prepare_cached(
            "SELECT 1 FROM transactions WHERE account_id = ?1 AND date = ?2 AND amount = ?3 AND description = ?4",
        )?;
    Ok(stmt.exists(rusqlite::params![
        account_id,
        row.date,
        row.amount,
        row.description
    ])?)
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

    pub fn parse(&self, file_path: &Path) -> Result<(Vec<ParsedRow>, usize)> {
        match self {
            Self::BofaChecking => parse_bofa_checking(file_path),
            Self::BofaCreditCard => parse_bofa_credit_card(file_path),
            Self::BofaLineOfCredit => parse_bofa_line_of_credit(file_path),
            #[cfg(feature = "gusto")]
            // Gusto extracts aggregate totals only; per-row malformed tracking is not applicable.
            Self::GustoPayroll => parse_gusto_payroll(file_path).map(|rows| (rows, 0)),
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
    pub fn post_import(
        &self,
        conn: &Connection,
        account_id: i64,
        rows: &[ParsedRow],
    ) -> Result<()> {
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
// Generic CSV config + helpers
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct GenericCsvConfig {
    pub date_col: usize,
    pub desc_col: usize,
    pub amount_col: usize,
    pub date_format: String,
}

pub fn save_csv_profile(conn: &Connection, name: &str, config: &GenericCsvConfig) -> Result<()> {
    if get_by_key(name).is_some() {
        return Err(NigelError::Other(format!(
            "'{name}' conflicts with a built-in importer; choose a different profile name"
        )));
    }
    conn.execute(
        "INSERT INTO csv_profiles (name, date_col, desc_col, amount_col, date_format)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(name) DO UPDATE SET
            date_col = excluded.date_col,
            desc_col = excluded.desc_col,
            amount_col = excluded.amount_col,
            date_format = excluded.date_format",
        rusqlite::params![
            name,
            config.date_col as i64,
            config.desc_col as i64,
            config.amount_col as i64,
            config.date_format,
        ],
    )?;
    Ok(())
}

pub fn load_csv_profile(conn: &Connection, name: &str) -> Result<Option<GenericCsvConfig>> {
    let mut stmt = conn.prepare(
        "SELECT date_col, desc_col, amount_col, date_format FROM csv_profiles WHERE name = ?1",
    )?;
    let result = stmt.query_row([name], |row| {
        Ok(GenericCsvConfig {
            date_col: row.get::<_, i64>(0)? as usize,
            desc_col: row.get::<_, i64>(1)? as usize,
            amount_col: row.get::<_, i64>(2)? as usize,
            date_format: row.get(3)?,
        })
    });
    match result {
        Ok(config) => Ok(Some(config)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn parse_generic_csv(
    file_path: &Path,
    config: &GenericCsvConfig,
) -> Result<(Vec<ParsedRow>, usize)> {
    let mut rdr = create_csv_reader(file_path)?;
    let mut rows = Vec::new();
    let mut malformed = 0usize;
    let mut first = true;
    let min_cols = [config.date_col, config.desc_col, config.amount_col]
        .into_iter()
        .max()
        .unwrap_or(0)
        + 1;

    for result in rdr.records() {
        let Ok(record) = result else {
            malformed += 1;
            continue;
        };
        // Skip header row
        if first {
            first = false;
            continue;
        }
        if record.len() < min_cols {
            malformed += 1;
            continue;
        }
        let raw_date = record[config.date_col].trim();
        let date = match chrono::NaiveDate::parse_from_str(raw_date, &config.date_format) {
            Ok(d) => d.format("%Y-%m-%d").to_string(),
            Err(_) => {
                malformed += 1;
                continue;
            }
        };
        let description = record[config.desc_col].trim().to_string();
        if description.is_empty() {
            malformed += 1;
            continue;
        }
        let Some(amount) = parse_amount(&record[config.amount_col]) else {
            malformed += 1;
            continue;
        };
        rows.push(ParsedRow {
            date,
            description,
            amount,
        });
    }
    Ok((rows, malformed))
}

// ---------------------------------------------------------------------------
// import_file
// ---------------------------------------------------------------------------

pub struct ImportResult {
    pub imported: usize,
    pub skipped: usize,
    pub malformed: usize,
    pub duplicate_file: bool,
    pub sample: Vec<ParsedRow>,
}

pub fn import_file(
    conn: &Connection,
    file_path: &Path,
    account_name: &str,
    format_key: Option<&str>,
    dry_run: bool,
    inline_config: Option<&GenericCsvConfig>,
) -> Result<ImportResult> {
    let (account_id, account_type) = {
        let mut stmt = conn.prepare("SELECT id, account_type FROM accounts WHERE name = ?1")?;

        stmt.query_row([account_name], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|_| NigelError::UnknownAccount(account_name.to_string()))?
    };

    let checksum = compute_checksum(file_path)?;
    {
        let mut stmt = conn.prepare("SELECT 1 FROM imports WHERE checksum = ?1")?;
        if stmt.exists(rusqlite::params![checksum])? {
            return Ok(ImportResult {
                imported: 0,
                skipped: 0,
                malformed: 0,
                duplicate_file: true,
                sample: Vec::new(),
            });
        }
    }

    enum ResolvedImporter {
        BuiltIn(ImporterKind),
        Generic(GenericCsvConfig),
    }

    let resolved = if let Some(config) = inline_config {
        ResolvedImporter::Generic(config.clone())
    } else if let Some(key) = format_key {
        if let Some(kind) = get_by_key(key) {
            ResolvedImporter::BuiltIn(kind)
        } else if let Some(config) = load_csv_profile(conn, key)? {
            ResolvedImporter::Generic(config)
        } else {
            return Err(NigelError::UnknownFormat(key.to_string()));
        }
    } else {
        ResolvedImporter::BuiltIn(
            get_for_file(&account_type, file_path)
                .ok_or_else(|| NigelError::NoImporter(account_type.clone()))?,
        )
    };

    let (parsed_rows, malformed) = match &resolved {
        ResolvedImporter::BuiltIn(kind) => kind.parse(file_path)?,
        ResolvedImporter::Generic(config) => parse_generic_csv(file_path, config)?,
    };
    let sample: Vec<ParsedRow> = parsed_rows.iter().take(5).cloned().collect();

    let mut imported = 0usize;
    let mut skipped = 0usize;

    if !dry_run {
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
        let import_id = conn.last_insert_rowid();

        for row in &parsed_rows {
            if is_duplicate_row(conn, account_id, row)? {
                skipped += 1;
                continue;
            }
            conn.execute(
                "INSERT INTO transactions (account_id, date, description, amount, import_id, is_flagged, flag_reason) VALUES (?1, ?2, ?3, ?4, ?5, 1, 'No matching rule')",
                rusqlite::params![account_id, row.date, row.description, row.amount, import_id],
            )?;
            imported += 1;
        }

        if let ResolvedImporter::BuiltIn(importer) = &resolved {
            if importer.has_post_import() {
                importer.post_import(conn, account_id, &parsed_rows)?;
            }
        }
    } else {
        for row in &parsed_rows {
            if is_duplicate_row(conn, account_id, row)? {
                skipped += 1;
            } else {
                imported += 1;
            }
        }
    }

    Ok(ImportResult {
        imported,
        skipped,
        malformed,
        duplicate_file: false,
        sample,
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
        if record.len() >= 4 && record[0].trim() == "Date" && record[1].contains("Description") {
            return true;
        }
    }
    false
}

fn parse_bofa_checking(file_path: &Path) -> Result<(Vec<ParsedRow>, usize)> {
    let mut rdr = create_csv_reader(file_path)?;
    let mut rows = Vec::new();
    let mut found_header = false;
    let mut malformed = 0usize;

    for result in rdr.records() {
        let Ok(record) = result else {
            malformed += 1;
            continue;
        };
        if !found_header {
            if record.len() >= 4 && record[0].trim() == "Date" && record[1].contains("Description")
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
        let Some(amount) = parse_amount(&record[2]) else {
            malformed += 1;
            continue;
        };
        rows.push(ParsedRow {
            date,
            description,
            amount,
        });
    }
    Ok((rows, malformed))
}

// ---------------------------------------------------------------------------
// BofA Credit Card parser
// ---------------------------------------------------------------------------

fn detect_bofa_credit_card(file_path: &Path) -> bool {
    let Ok(file) = std::fs::File::open(file_path) else {
        return false;
    };
    let reader = std::io::BufReader::new(file);
    // BofA credit card CSVs put the "CardHolder Name" header in the first
    // 1-3 rows, so scanning the first 5 lines is sufficient for detection.
    for line in reader.lines().take(5) {
        let Ok(line) = line else { continue };
        if line.contains("CardHolder Name") {
            return true;
        }
    }
    false
}

fn parse_bofa_credit_card(file_path: &Path) -> Result<(Vec<ParsedRow>, usize)> {
    parse_bofa_card_format(file_path, true)
}

// ---------------------------------------------------------------------------
// BofA Line of Credit parser
// ---------------------------------------------------------------------------

fn parse_bofa_line_of_credit(file_path: &Path) -> Result<(Vec<ParsedRow>, usize)> {
    parse_bofa_card_format(file_path, false)
}

/// Shared parser for BofA Credit Card and Line of Credit CSV formats.
/// - `has_type_column: true`  → Credit Card (D/C sign logic)
/// - `has_type_column: false` → Line of Credit (always negate)
fn parse_bofa_card_format(
    file_path: &Path,
    has_type_column: bool,
) -> Result<(Vec<ParsedRow>, usize)> {
    let mut rdr = create_csv_reader(file_path)?;
    let mut rows = Vec::new();
    let mut found_header = false;
    let mut malformed = 0usize;
    let (mut idx_date, mut idx_desc, mut idx_amount, mut idx_type) = (3, 5, 6, 9);
    let mut header_field_count: usize = 0;

    for result in rdr.records() {
        let Ok(record) = result else {
            malformed += 1;
            continue;
        };
        if !found_header {
            if record.iter().any(|f| f.contains("Posting Date")) {
                header_field_count = record.len();
                for (i, field) in record.iter().enumerate() {
                    let f = field.trim();
                    if f == "Posting Date" {
                        idx_date = i;
                    }
                    if f == "Payee" {
                        idx_desc = i;
                    }
                    if f == "Amount" {
                        idx_amount = i;
                    }
                    if has_type_column && f == "Type" {
                        idx_type = i;
                    }
                }
                found_header = true;
            }
            continue;
        }
        // When the cardholder name contains commas (e.g. "RAYGUN DESIGN, LLC"),
        // the CSV parser splits it across multiple fields, giving data rows more
        // fields than the header. We assume all extra fields originate from the
        // CardHolder Name column (position 0) — BofA only puts unquoted commas
        // there. The date and amount validation below guards against silent
        // misalignment if that assumption ever breaks.
        let offset = if header_field_count > 0 && record.len() > header_field_count {
            record.len() - header_field_count
        } else {
            0
        };
        let adj_date = idx_date + offset;
        let adj_desc = idx_desc + offset;
        let adj_amount = idx_amount + offset;
        let adj_type = idx_type + offset;
        let min_cols = if has_type_column {
            [adj_date, adj_desc, adj_amount, adj_type]
                .into_iter()
                .max()
                .unwrap_or(0)
                + 1
        } else {
            [adj_date, adj_desc, adj_amount]
                .into_iter()
                .max()
                .unwrap_or(0)
                + 1
        };
        if record.len() < min_cols {
            continue;
        }
        // Validate that adjusted indices land on the right columns — a date
        // that doesn't parse or a non-numeric amount means the offset was wrong.
        let Some(date) = parse_date_mdy(&record[adj_date]) else {
            continue;
        };
        let description = record[adj_desc].trim().to_string();
        if description.is_empty() {
            continue;
        }
        let amount_str = record[adj_amount].trim();
        // Pre-validation catches obviously non-numeric strings (e.g. text fields);
        // parse_amount catches edge cases that pass character filtering but fail f64 parsing (e.g. ".").
        if amount_str.is_empty()
            || amount_str
                .replace(['-', '.', ',', '$', ' '], "")
                .chars()
                .any(|c| !c.is_ascii_digit())
        {
            malformed += 1;
            continue;
        }
        let Some(parsed) = parse_amount(amount_str) else {
            malformed += 1;
            continue;
        };
        let amount = if has_type_column {
            let txn_type = record[adj_type].trim();
            if txn_type == "D" {
                -parsed.abs()
            } else {
                parsed.abs()
            }
        } else {
            -parsed
        };
        rows.push(ParsedRow {
            date,
            description,
            amount,
        });
    }
    Ok((rows, malformed))
}

// ---------------------------------------------------------------------------
// Gusto Payroll parser (feature-gated)
// ---------------------------------------------------------------------------

#[cfg(feature = "gusto")]
fn detect_gusto_payroll(file_path: &Path) -> bool {
    use calamine::Reader;
    if !file_path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("xlsx"))
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
        let id: std::result::Result<i64, _> = conn.query_row(
            "SELECT id FROM categories WHERE name = ?1",
            [cat_name],
            |r| r.get(0),
        );
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
            "INSERT INTO accounts (name, account_type) VALUES ('Test Checking', 'checking')",
            [],
        )
        .unwrap();
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
        assert_eq!(parse_amount("1,234.56"), Some(1234.56));
        assert_eq!(parse_amount("\"500.00\""), Some(500.0));
        assert_eq!(parse_amount("  -42.50  "), Some(-42.5));
        assert_eq!(parse_amount("0"), Some(0.0));
        assert_eq!(parse_amount("not_a_number"), None);
    }

    #[test]
    fn test_parse_amount_parenthesized_negatives() {
        assert_eq!(parse_amount("(500.00)"), Some(-500.0));
        assert_eq!(parse_amount("(1,234.56)"), Some(-1234.56));
        assert_eq!(parse_amount("\"(50.00)\""), Some(-50.0));
    }

    #[test]
    fn test_parse_amount_currency_symbol() {
        assert_eq!(parse_amount("$1,234.56"), Some(1234.56));
        assert_eq!(parse_amount("-$50.00"), Some(-50.0));
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
        let csv_path = write_bofa_csv(
            dir.path(),
            "stmt.csv",
            &[
                ("01/15/2025", "PAYMENT ONE", "-100.00"),
                ("01/16/2025", "PAYMENT TWO", "-250.00"),
                ("01/17/2025", "DEPOSIT", "500.00"),
            ],
        );
        let result = import_file(
            &conn,
            &csv_path,
            "Test Checking",
            Some("bofa_checking"),
            false,
            None,
        )
        .unwrap();
        assert_eq!(result.imported, 3);
        assert!(!result.duplicate_file);
        let count: i64 = conn
            .query_row("SELECT count(*) FROM transactions", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 3);
    }

    #[test]
    fn test_import_file_detects_file_duplicate() {
        let (dir, conn) = test_db();
        add_test_account(&conn);
        let csv_path = write_bofa_csv(
            dir.path(),
            "stmt.csv",
            &[("01/15/2025", "PAYMENT ONE", "-100.00")],
        );
        let r1 = import_file(
            &conn,
            &csv_path,
            "Test Checking",
            Some("bofa_checking"),
            false,
            None,
        )
        .unwrap();
        assert_eq!(r1.imported, 1);
        let r2 = import_file(
            &conn,
            &csv_path,
            "Test Checking",
            Some("bofa_checking"),
            false,
            None,
        )
        .unwrap();
        assert!(r2.duplicate_file);
        assert_eq!(r2.imported, 0);
    }

    #[test]
    fn test_import_file_detects_row_duplicates() {
        let (dir, conn) = test_db();
        add_test_account(&conn);
        let csv1 = write_bofa_csv(
            dir.path(),
            "stmt1.csv",
            &[
                ("01/15/2025", "PAYMENT ONE", "-100.00"),
                ("01/16/2025", "PAYMENT TWO", "-200.00"),
            ],
        );
        import_file(
            &conn,
            &csv1,
            "Test Checking",
            Some("bofa_checking"),
            false,
            None,
        )
        .unwrap();
        let csv2 = write_bofa_csv(
            dir.path(),
            "stmt2.csv",
            &[
                ("01/16/2025", "PAYMENT TWO", "-200.00"),
                ("01/18/2025", "PAYMENT THREE", "-300.00"),
            ],
        );
        let r2 = import_file(
            &conn,
            &csv2,
            "Test Checking",
            Some("bofa_checking"),
            false,
            None,
        )
        .unwrap();
        assert_eq!(r2.imported, 1);
        assert_eq!(r2.skipped, 1);
    }

    #[test]
    fn test_import_file_records_batch() {
        let (dir, conn) = test_db();
        add_test_account(&conn);
        let csv_path = write_bofa_csv(
            dir.path(),
            "stmt.csv",
            &[("01/15/2025", "PAYMENT ONE", "-100.00")],
        );
        import_file(
            &conn,
            &csv_path,
            "Test Checking",
            Some("bofa_checking"),
            false,
            None,
        )
        .unwrap();
        let count: i64 = conn
            .query_row("SELECT count(*) FROM imports", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
        let record_count: i64 = conn
            .query_row("SELECT record_count FROM imports LIMIT 1", [], |r| r.get(0))
            .unwrap();
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
        let (rows, _malformed) = ImporterKind::BofaChecking.parse(&path).unwrap();
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
        let (rows, _malformed) = ImporterKind::BofaChecking.parse(&path).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].description, "ADOBE CREATIVE");
        assert_eq!(rows[0].amount, -50.0);
        assert_eq!(rows[1].description, "STRIPE PAYOUT");
        assert_eq!(rows[1].amount, 2500.0);
    }

    #[test]
    fn test_bofa_credit_card_parse_comma_in_name() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cc.csv");
        // Header has 11 fields; data rows with "RAYGUN DESIGN, LLC" produce 12 fields
        let content = "\
CardHolder Name,Account Number,Transaction Date,Posting Date,Amount,Category,Payee,Address,City/State,Reference Number,Type
RAYGUN DESIGN, LLC,1234,01/15/2025,01/15/2025,-50.00,Shopping,AMAZON PURCHASE,123 Main St,Seattle WA,REF001,D
RAYGUN DESIGN, LLC,1234,01/16/2025,01/16/2025,-75.50,Travel,DELTA AIRLINES,456 Airport Rd,Atlanta GA,REF002,D
RAYGUN DESIGN, LLC,1234,01/17/2025,01/17/2025,200.00,Refund,STORE REFUND,789 Oak Ave,Portland OR,REF003,C
";
        std::fs::write(&path, content).unwrap();
        let (rows, _malformed) = ImporterKind::BofaCreditCard.parse(&path).unwrap();
        assert_eq!(rows.len(), 3, "all 3 rows should be parsed");
        assert_eq!(rows[0].date, "2025-01-15");
        assert_eq!(rows[0].description, "AMAZON PURCHASE");
        assert_eq!(rows[0].amount, -50.0); // D = debit
        assert_eq!(rows[1].description, "DELTA AIRLINES");
        assert_eq!(rows[1].amount, -75.5);
        assert_eq!(rows[2].description, "STORE REFUND");
        assert_eq!(rows[2].amount, 200.0); // C = credit
    }

    #[test]
    fn test_bofa_credit_card_parse_no_comma_in_name() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cc.csv");
        // Name without comma — should still work correctly (offset = 0)
        let content = "\
CardHolder Name,Account Number,Transaction Date,Posting Date,Amount,Category,Payee,Address,City/State,Reference Number,Type
RAYGUN DESIGN LLC,1234,01/15/2025,01/15/2025,-50.00,Shopping,AMAZON PURCHASE,123 Main St,Seattle WA,REF001,D
";
        std::fs::write(&path, content).unwrap();
        let (rows, _malformed) = ImporterKind::BofaCreditCard.parse(&path).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].description, "AMAZON PURCHASE");
        assert_eq!(rows[0].amount, -50.0);
    }

    #[test]
    fn test_bofa_loc_parse_comma_in_name() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("loc.csv");
        let content = "\
CardHolder Name,Account Number,Transaction Date,Posting Date,Amount,Category,Payee,Address,City/State,Reference Number
RAYGUN DESIGN, LLC,5678,02/01/2025,02/01/2025,-1000.00,Transfer,LINE OF CREDIT DRAW,100 Bank St,Boston MA,REF101
RAYGUN DESIGN, LLC,5678,02/05/2025,02/05/2025,500.00,Payment,LOC PAYMENT,100 Bank St,Boston MA,REF102
";
        std::fs::write(&path, content).unwrap();
        let (rows, _malformed) = ImporterKind::BofaLineOfCredit.parse(&path).unwrap();
        assert_eq!(rows.len(), 2, "all 2 rows should be parsed");
        assert_eq!(rows[0].date, "2025-02-01");
        assert_eq!(rows[0].description, "LINE OF CREDIT DRAW");
        assert_eq!(rows[0].amount, 1000.0); // LOC negates the amount
        assert_eq!(rows[1].description, "LOC PAYMENT");
        assert_eq!(rows[1].amount, -500.0);
    }

    #[test]
    fn test_bofa_credit_card_parse_multiple_commas_in_name() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cc.csv");
        // Name with two commas — offset should be 2
        let content = "\
CardHolder Name,Account Number,Transaction Date,Posting Date,Amount,Category,Payee,Address,City/State,Reference Number,Type
SMITH, JONES, AND ASSOCIATES,1234,01/15/2025,01/15/2025,-99.00,Office,STAPLES,100 Main,City ST,REF001,D
";
        std::fs::write(&path, content).unwrap();
        let (rows, _malformed) = ImporterKind::BofaCreditCard.parse(&path).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].description, "STAPLES");
        assert_eq!(rows[0].amount, -99.0);
    }

    #[test]
    fn test_bofa_credit_card_mixed_rows() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cc.csv");
        // Mix of rows: some cardholder names have commas, others don't
        let content = "\
CardHolder Name,Account Number,Transaction Date,Posting Date,Amount,Category,Payee,Address,City/State,Reference Number,Type
RAYGUN DESIGN, LLC,1234,01/15/2025,01/15/2025,-50.00,Shopping,AMAZON PURCHASE,123 Main St,Seattle WA,REF001,D
JOHN DOE,1234,01/16/2025,01/16/2025,-30.00,Food,RESTAURANT,456 Elm St,Portland OR,REF002,D
RAYGUN DESIGN, LLC,1234,01/17/2025,01/17/2025,100.00,Refund,STORE REFUND,789 Oak Ave,City ST,REF003,C
";
        std::fs::write(&path, content).unwrap();
        let (rows, _malformed) = ImporterKind::BofaCreditCard.parse(&path).unwrap();
        assert_eq!(rows.len(), 3, "all 3 rows should be parsed");
        assert_eq!(rows[0].description, "AMAZON PURCHASE");
        assert_eq!(rows[0].amount, -50.0);
        assert_eq!(rows[1].description, "RESTAURANT");
        assert_eq!(rows[1].amount, -30.0);
        assert_eq!(rows[2].description, "STORE REFUND");
        assert_eq!(rows[2].amount, 100.0);
    }

    #[test]
    fn test_import_skips_malformed_amount_rows() {
        let (dir, conn) = test_db();
        add_test_account(&conn);
        let csv_path = write_bofa_csv(
            dir.path(),
            "stmt.csv",
            &[
                ("01/15/2025", "VALID PAYMENT", "-100.00"),
                ("01/16/2025", "BAD AMOUNT ROW", "not_a_number"),
                ("01/17/2025", "ANOTHER VALID", "250.00"),
            ],
        );
        let result = import_file(
            &conn,
            &csv_path,
            "Test Checking",
            Some("bofa_checking"),
            false,
            None,
        )
        .unwrap();
        assert_eq!(result.imported, 2, "malformed amount row should be skipped");
        assert_eq!(
            result.malformed, 1,
            "one row should be counted as malformed"
        );
        let count: i64 = conn
            .query_row("SELECT count(*) FROM transactions", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_bofa_credit_card_comma_in_payee_skips_row() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cc.csv");
        // Comma in the Payee field instead of CardHolder Name — offset pushes
        // indices past the real columns. The date/amount validation should
        // cause this row to be skipped rather than silently importing wrong data.
        let content = "\
CardHolder Name,Account Number,Transaction Date,Posting Date,Amount,Category,Payee,Address,City/State,Reference Number,Type
JOHN DOE,1234,01/15/2025,01/15/2025,-50.00,Shopping,AMAZON, INC,123 Main St,Seattle WA,REF001,D
";
        std::fs::write(&path, content).unwrap();
        let (rows, _malformed) = ImporterKind::BofaCreditCard.parse(&path).unwrap();
        // Row has 12 fields (one extra from Payee comma) but CardHolder Name
        // has no comma, so offset=1 shifts all indices — the adjusted date
        // column now points at "01/15/2025" → date still looks valid, but
        // adjusted desc points at "Shopping" and adjusted amount at "AMAZON".
        // Amount validation rejects "AMAZON" as non-numeric, so row is skipped.
        assert_eq!(rows.len(), 0, "row with misaligned comma should be skipped");
    }

    #[test]
    fn test_import_file_links_transactions_to_import_batch() {
        let (dir, conn) = test_db();
        add_test_account(&conn);
        let csv_path = write_bofa_csv(
            dir.path(),
            "stmt.csv",
            &[("01/15/2025", "PAYMENT ONE", "-100.00")],
        );
        import_file(
            &conn,
            &csv_path,
            "Test Checking",
            Some("bofa_checking"),
            false,
            None,
        )
        .unwrap();
        let import_id: i64 = conn
            .query_row("SELECT id FROM imports LIMIT 1", [], |r| r.get(0))
            .unwrap();
        let tx_import_id: i64 = conn
            .query_row("SELECT import_id FROM transactions LIMIT 1", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(tx_import_id, import_id);
    }

    #[test]
    fn test_bofa_credit_card_malformed_amount() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cc.csv");
        let content = "\
CardHolder Name,Account Number,Transaction Date,Posting Date,Amount,Category,Payee,Address,City/State,Reference Number,Type
RAYGUN DESIGN LLC,1234,01/15/2025,01/15/2025,-50.00,Shopping,AMAZON PURCHASE,123 Main St,Seattle WA,REF001,D
RAYGUN DESIGN LLC,1234,01/16/2025,01/16/2025,NOT_A_NUMBER,Food,RESTAURANT,456 Elm St,Portland OR,REF002,D
RAYGUN DESIGN LLC,1234,01/17/2025,01/17/2025,-25.00,Travel,DELTA AIRLINES,789 Oak Ave,City ST,REF003,D
";
        std::fs::write(&path, content).unwrap();
        let (rows, malformed) = ImporterKind::BofaCreditCard.parse(&path).unwrap();
        assert_eq!(rows.len(), 2, "only valid rows should be imported");
        assert_eq!(malformed, 1, "one row should be counted as malformed");
        assert_eq!(rows[0].description, "AMAZON PURCHASE");
        assert_eq!(rows[1].description, "DELTA AIRLINES");
    }

    #[test]
    fn test_import_result_has_sample() {
        let (dir, conn) = test_db();
        add_test_account(&conn);
        let csv_path = write_bofa_csv(
            dir.path(),
            "stmt.csv",
            &[
                ("01/15/2025", "PAYMENT ONE", "-100.00"),
                ("01/16/2025", "PAYMENT TWO", "-250.00"),
                ("01/17/2025", "DEPOSIT", "500.00"),
            ],
        );
        let result = import_file(
            &conn,
            &csv_path,
            "Test Checking",
            Some("bofa_checking"),
            false,
            None,
        )
        .unwrap();
        assert_eq!(result.sample.len(), 3);
        assert_eq!(result.sample[0].description, "PAYMENT ONE");
    }

    #[test]
    fn test_dry_run_does_not_write_to_db() {
        let (dir, conn) = test_db();
        add_test_account(&conn);
        let csv_path = write_bofa_csv(
            dir.path(),
            "stmt.csv",
            &[
                ("01/15/2025", "PAYMENT ONE", "-100.00"),
                ("01/16/2025", "PAYMENT TWO", "-250.00"),
            ],
        );
        let result = import_file(
            &conn,
            &csv_path,
            "Test Checking",
            Some("bofa_checking"),
            true,
            None,
        )
        .unwrap();
        assert_eq!(result.imported, 2);
        assert_eq!(result.skipped, 0);
        assert!(!result.duplicate_file);
        let tx_count: i64 = conn
            .query_row("SELECT count(*) FROM transactions", [], |r| r.get(0))
            .unwrap();
        assert_eq!(tx_count, 0, "dry run should not insert transactions");
        let import_count: i64 = conn
            .query_row("SELECT count(*) FROM imports", [], |r| r.get(0))
            .unwrap();
        assert_eq!(import_count, 0, "dry run should not insert import record");
    }

    #[test]
    fn test_dry_run_detects_duplicates() {
        let (dir, conn) = test_db();
        add_test_account(&conn);
        let csv_path = write_bofa_csv(
            dir.path(),
            "stmt.csv",
            &[
                ("01/15/2025", "PAYMENT ONE", "-100.00"),
                ("01/16/2025", "PAYMENT TWO", "-250.00"),
            ],
        );
        import_file(
            &conn,
            &csv_path,
            "Test Checking",
            Some("bofa_checking"),
            false,
            None,
        )
        .unwrap();
        let csv2 = write_bofa_csv(
            dir.path(),
            "stmt2.csv",
            &[
                ("01/16/2025", "PAYMENT TWO", "-250.00"),
                ("01/18/2025", "PAYMENT THREE", "-300.00"),
            ],
        );
        let result = import_file(
            &conn,
            &csv2,
            "Test Checking",
            Some("bofa_checking"),
            true,
            None,
        )
        .unwrap();
        assert_eq!(result.imported, 1);
        assert_eq!(result.skipped, 1);
    }

    #[test]
    fn test_dry_run_sample_capped_at_five() {
        let (dir, conn) = test_db();
        add_test_account(&conn);
        let rows: Vec<(&str, &str, &str)> = vec![
            ("01/01/2025", "TXN 1", "-10.00"),
            ("01/02/2025", "TXN 2", "-20.00"),
            ("01/03/2025", "TXN 3", "-30.00"),
            ("01/04/2025", "TXN 4", "-40.00"),
            ("01/05/2025", "TXN 5", "-50.00"),
            ("01/06/2025", "TXN 6", "-60.00"),
            ("01/07/2025", "TXN 7", "-70.00"),
        ];
        let csv_path = write_bofa_csv(dir.path(), "stmt.csv", &rows);
        let result = import_file(
            &conn,
            &csv_path,
            "Test Checking",
            Some("bofa_checking"),
            true,
            None,
        )
        .unwrap();
        assert_eq!(result.sample.len(), 5, "sample should be capped at 5");
        assert_eq!(result.imported, 7);
    }

    #[test]
    fn test_parse_generic_csv_default_date_format() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("generic.csv");
        let content = "\
Date,Memo,Debit,Credit,Balance
01/15/2025,COFFEE SHOP,-5.50,,994.50
01/16/2025,PAYCHECK,,2000.00,2994.50
01/17/2025,RENT,-1500.00,,1494.50
";
        std::fs::write(&path, content).unwrap();
        let config = GenericCsvConfig {
            date_col: 0,
            desc_col: 1,
            amount_col: 2,
            date_format: "%m/%d/%Y".to_string(),
        };
        let (rows, malformed) = parse_generic_csv(&path, &config).unwrap();
        assert_eq!(rows.len(), 2); // row 2 has empty amount col -> malformed
        assert_eq!(malformed, 1);
        assert_eq!(rows[0].date, "2025-01-15");
        assert_eq!(rows[0].description, "COFFEE SHOP");
        assert_eq!(rows[0].amount, -5.50);
    }

    #[test]
    fn test_parse_generic_csv_custom_date_format() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("generic.csv");
        let content = "\
transaction_date,description,amount
2025-01-15,COFFEE SHOP,-5.50
2025-01-16,PAYCHECK,2000.00
";
        std::fs::write(&path, content).unwrap();
        let config = GenericCsvConfig {
            date_col: 0,
            desc_col: 1,
            amount_col: 2,
            date_format: "%Y-%m-%d".to_string(),
        };
        let (rows, _) = parse_generic_csv(&path, &config).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].date, "2025-01-15");
        assert_eq!(rows[1].amount, 2000.0);
    }

    #[test]
    fn test_generic_csv_profile_save_and_load() {
        let (_dir, conn) = test_db();
        let config = GenericCsvConfig {
            date_col: 0,
            desc_col: 2,
            amount_col: 4,
            date_format: "%d/%m/%Y".to_string(),
        };
        save_csv_profile(&conn, "chase", &config).unwrap();
        let loaded = load_csv_profile(&conn, "chase").unwrap();
        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded.date_col, 0);
        assert_eq!(loaded.desc_col, 2);
        assert_eq!(loaded.amount_col, 4);
        assert_eq!(loaded.date_format, "%d/%m/%Y");
    }

    #[test]
    fn test_generic_csv_profile_overwrite() {
        let (_dir, conn) = test_db();
        let config1 = GenericCsvConfig {
            date_col: 0,
            desc_col: 1,
            amount_col: 2,
            date_format: "%m/%d/%Y".to_string(),
        };
        save_csv_profile(&conn, "chase", &config1).unwrap();
        let config2 = GenericCsvConfig {
            date_col: 1,
            desc_col: 3,
            amount_col: 5,
            date_format: "%Y-%m-%d".to_string(),
        };
        save_csv_profile(&conn, "chase", &config2).unwrap();
        let loaded = load_csv_profile(&conn, "chase").unwrap().unwrap();
        assert_eq!(loaded.date_col, 1);
        assert_eq!(loaded.desc_col, 3);
    }

    #[test]
    fn test_import_file_with_generic_csv() {
        let (dir, conn) = test_db();
        add_test_account(&conn);
        let path = dir.path().join("generic.csv");
        let content = "\
Date,Note,Amount
01/15/2025,COFFEE,-5.50
01/16/2025,SALARY,3000.00
";
        std::fs::write(&path, content).unwrap();
        let config = GenericCsvConfig {
            date_col: 0,
            desc_col: 1,
            amount_col: 2,
            date_format: "%m/%d/%Y".to_string(),
        };
        save_csv_profile(&conn, "test_bank", &config).unwrap();
        let result = import_file(
            &conn,
            &path,
            "Test Checking",
            Some("test_bank"),
            false,
            None,
        )
        .unwrap();
        assert_eq!(result.imported, 2);
        let count: i64 = conn
            .query_row("SELECT count(*) FROM transactions", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 2);
    }
}
