#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Account {
    pub id: i64,
    pub name: String,
    pub account_type: String,
    pub institution: Option<String>,
    pub last_four: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Category {
    pub id: i64,
    pub name: String,
    pub category_type: String,
    pub parent_id: Option<i64>,
    pub tax_line: Option<String>,
    pub form_line: Option<String>,
    pub description: Option<String>,
    pub is_active: bool,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Transaction {
    pub id: Option<i64>,
    pub account_id: i64,
    pub date: String,
    pub description: String,
    pub amount: f64,
    pub category_id: Option<i64>,
    pub vendor: Option<String>,
    pub notes: Option<String>,
    pub is_flagged: bool,
    pub flag_reason: Option<String>,
    pub import_id: Option<i64>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Rule {
    pub id: Option<i64>,
    pub pattern: String,
    pub category_id: i64,
    pub match_type: String,
    pub vendor: Option<String>,
    pub priority: i64,
    pub hit_count: i64,
    pub is_active: bool,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ImportRecord {
    pub id: Option<i64>,
    pub filename: String,
    pub account_id: i64,
    pub record_count: Option<i64>,
    pub date_range_start: Option<String>,
    pub date_range_end: Option<String>,
    pub checksum: Option<String>,
}

/// Intermediate representation from a CSV/XLSX parser before DB insert.
#[derive(Debug, Clone)]
pub struct ParsedRow {
    pub date: String,
    pub description: String,
    pub amount: f64,
}
