use std::collections::HashMap;

use comfy_table::{Cell, Table};
use regex::Regex;
use rusqlite::Connection;
use serde::Serialize;

use crate::categorizer::matches as rule_matches;
use crate::cli::categories::ensure_category_exists;
use crate::db::get_connection;
use crate::error::{NigelError, Result};
use crate::settings::get_data_dir;

/// An active categorization rule joined to the category it assigns.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleRow {
    pub id: i64,
    pub pattern: String,
    pub match_type: String,
    pub vendor: Option<String>,
    pub category: String,
    pub category_id: i64,
    pub priority: i64,
    pub hit_count: i64,
}

/// Active rules in the order the categorizer applies them: highest priority
/// first, ties broken by insertion order so the sequence is stable.
pub fn list_rules(conn: &Connection) -> Result<Vec<RuleRow>> {
    let mut stmt = conn.prepare(
        "SELECT r.id, r.pattern, r.match_type, r.vendor, c.name, r.category_id, \
                r.priority, r.hit_count \
         FROM rules r JOIN categories c ON r.category_id = c.id \
         WHERE r.is_active = 1 \
         ORDER BY r.priority DESC, r.id ASC",
    )?;
    let rules = stmt
        .query_map([], |row| {
            Ok(RuleRow {
                id: row.get(0)?,
                pattern: row.get(1)?,
                match_type: row.get(2)?,
                vendor: row.get(3)?,
                category: row.get(4)?,
                category_id: row.get(5)?,
                priority: row.get(6)?,
                hit_count: row.get(7)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rules)
}

/// The match types the categorizer understands.
pub const MATCH_TYPES: [&str; 3] = ["contains", "starts_with", "regex"];

/// A rule about to be created.
pub struct NewRule<'a> {
    pub pattern: &'a str,
    pub category_id: i64,
    pub vendor: Option<&'a str>,
    pub match_type: &'a str,
    pub priority: i64,
}

/// Fields to change on an existing rule. `None` leaves a field alone; a
/// `Some(None)` vendor clears it.
#[derive(Debug, Default, Clone)]
pub struct RuleUpdate {
    pub pattern: Option<String>,
    pub match_type: Option<String>,
    pub vendor: Option<Option<String>>,
    pub category_id: Option<i64>,
    pub priority: Option<i64>,
}

impl RuleUpdate {
    fn is_empty(&self) -> bool {
        self.pattern.is_none()
            && self.match_type.is_none()
            && self.vendor.is_none()
            && self.category_id.is_none()
            && self.priority.is_none()
    }
}

/// One description a pattern would match, and how many transactions carry it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleTestMatch {
    pub description: String,
    pub count: i64,
}

/// The dry-run result for a pattern: what it would match today.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleTestResult {
    pub total: i64,
    pub matches: Vec<RuleTestMatch>,
}

/// Reject a match type the categorizer would silently never match on, and a
/// regex that will not compile. `matches()` answers `false` for both, so a rule
/// saved with either is dead weight nobody would notice.
pub fn validate_match_type(match_type: &str, pattern: &str) -> Result<()> {
    if !MATCH_TYPES.contains(&match_type) {
        return Err(NigelError::Invalid(format!(
            "Invalid match type: {match_type}. Must be one of: {}",
            MATCH_TYPES.join(", ")
        )));
    }
    if match_type == "regex" {
        Regex::new(pattern).map_err(|e| NigelError::Invalid(format!("Invalid regex: {e}")))?;
    }
    Ok(())
}

/// Look up a category id by name, the way the CLI addresses categories.
pub fn resolve_category_id(conn: &Connection, name: &str) -> Result<i64> {
    conn.query_row(
        "SELECT id FROM categories WHERE name = ?1 AND is_active = 1",
        [name],
        |row| row.get(0),
    )
    .map_err(|_| NigelError::UnknownCategory(name.to_string()))
}

/// A rule by id, active or not — an edit screen still has to be able to show a
/// rule it is about to reject.
pub fn get_rule(conn: &Connection, id: i64) -> Result<RuleRow> {
    conn.query_row(
        "SELECT r.id, r.pattern, r.match_type, r.vendor, c.name, r.category_id, \
                r.priority, r.hit_count \
         FROM rules r JOIN categories c ON r.category_id = c.id \
         WHERE r.id = ?1",
        [id],
        |row| {
            Ok(RuleRow {
                id: row.get(0)?,
                pattern: row.get(1)?,
                match_type: row.get(2)?,
                vendor: row.get(3)?,
                category: row.get(4)?,
                category_id: row.get(5)?,
                priority: row.get(6)?,
                hit_count: row.get(7)?,
            })
        },
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => {
            NigelError::NotFound(format!("No rule with ID {id}"))
        }
        other => NigelError::Db(other),
    })
}

fn rule_is_active(conn: &Connection, id: i64) -> Result<bool> {
    conn.query_row("SELECT is_active FROM rules WHERE id = ?1", [id], |row| {
        row.get::<_, i32>(0)
    })
    .map(|active| active == 1)
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => {
            NigelError::NotFound(format!("No rule with ID {id}"))
        }
        other => NigelError::Db(other),
    })
}

/// Create a rule and return its id.
pub fn add_rule(conn: &Connection, rule: NewRule<'_>) -> Result<i64> {
    if rule.pattern.trim().is_empty() {
        return Err(NigelError::Invalid("Pattern is required".into()));
    }
    validate_match_type(rule.match_type, rule.pattern)?;
    ensure_category_exists(conn, rule.category_id)?;
    conn.execute(
        "INSERT INTO rules (pattern, match_type, vendor, category_id, priority) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![
            rule.pattern,
            rule.match_type,
            rule.vendor,
            rule.category_id,
            rule.priority
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Apply a partial update to an active rule.
pub fn update_rule(conn: &Connection, id: i64, update: &RuleUpdate) -> Result<()> {
    let current = get_rule(conn, id)?;
    if !rule_is_active(conn, id)? {
        return Err(NigelError::Conflict {
            code: "already_inactive",
            message: format!("Rule {id} is inactive"),
        });
    }
    if update.is_empty() {
        return Err(NigelError::Invalid(
            "Nothing to update — provide at least one flag".to_string(),
        ));
    }

    // Only re-validate when the pattern or the match type is actually moving:
    // a rule stored with a since-broken regex should still accept a priority
    // change.
    if update.pattern.is_some() || update.match_type.is_some() {
        let pattern = update.pattern.as_deref().unwrap_or(&current.pattern);
        let match_type = update.match_type.as_deref().unwrap_or(&current.match_type);
        validate_match_type(match_type, pattern)?;
    }
    if let Some(category_id) = update.category_id {
        ensure_category_exists(conn, category_id)?;
    }

    let mut updates = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(ref pattern) = update.pattern {
        params.push(Box::new(pattern.clone()));
        updates.push(format!("pattern = ?{}", params.len()));
    }
    if let Some(ref match_type) = update.match_type {
        params.push(Box::new(match_type.clone()));
        updates.push(format!("match_type = ?{}", params.len()));
    }
    if let Some(ref vendor) = update.vendor {
        params.push(Box::new(vendor.clone()));
        updates.push(format!("vendor = ?{}", params.len()));
    }
    if let Some(category_id) = update.category_id {
        params.push(Box::new(category_id));
        updates.push(format!("category_id = ?{}", params.len()));
    }
    if let Some(priority) = update.priority {
        params.push(Box::new(priority));
        updates.push(format!("priority = ?{}", params.len()));
    }

    params.push(Box::new(id));
    let sql = format!(
        "UPDATE rules SET {} WHERE id = ?{}",
        updates.join(", "),
        params.len()
    );
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    conn.execute(&sql, param_refs.as_slice())?;
    Ok(())
}

/// Soft-delete a rule. The row stays for its hit count and for anything that
/// still references it.
pub fn deactivate_rule(conn: &Connection, id: i64) -> Result<()> {
    if !rule_is_active(conn, id)? {
        return Err(NigelError::Conflict {
            code: "already_inactive",
            message: format!("Rule {id} is already inactive"),
        });
    }
    conn.execute("UPDATE rules SET is_active = 0 WHERE id = ?1", [id])?;
    Ok(())
}

/// What a pattern would match right now, without saving anything: the same
/// scan `nigel rules test` prints, as data.
pub fn test_pattern(conn: &Connection, pattern: &str, match_type: &str) -> Result<RuleTestResult> {
    validate_match_type(match_type, pattern)?;

    let mut stmt = conn.prepare("SELECT description FROM transactions")?;
    let descriptions: Vec<String> = stmt
        .query_map([], |row| row.get(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    let mut match_counts: HashMap<String, i64> = HashMap::new();
    for desc in &descriptions {
        if rule_matches(desc, pattern, match_type) {
            *match_counts.entry(desc.clone()).or_default() += 1;
        }
    }

    let total: i64 = match_counts.values().sum();
    let mut matches: Vec<RuleTestMatch> = match_counts
        .into_iter()
        .map(|(description, count)| RuleTestMatch { description, count })
        .collect();
    // Busiest description first, alphabetical within a tie.
    matches.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then(a.description.cmp(&b.description))
    });

    Ok(RuleTestResult { total, matches })
}

// ---------------------------------------------------------------------------
// CLI commands
// ---------------------------------------------------------------------------

pub fn add(
    pattern: &str,
    category: &str,
    vendor: Option<&str>,
    match_type: &str,
    priority: i64,
) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let category_id = resolve_category_id(&conn, category)?;
    add_rule(
        &conn,
        NewRule {
            pattern,
            category_id,
            vendor,
            match_type,
            priority,
        },
    )?;
    println!("Added rule: '{pattern}' \u{2192} {category}");
    Ok(())
}

pub fn list() -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;

    let mut table = Table::new();
    table.set_header(vec![
        "ID", "Pattern", "Type", "Vendor", "Category", "Priority", "Hits",
    ]);
    for rule in list_rules(&conn)? {
        table.add_row(vec![
            Cell::new(rule.id),
            Cell::new(rule.pattern),
            Cell::new(rule.match_type),
            Cell::new(rule.vendor.unwrap_or_default()),
            Cell::new(rule.category),
            Cell::new(rule.priority),
            Cell::new(rule.hit_count),
        ]);
    }
    println!("Rules\n{table}");
    Ok(())
}

pub fn update(
    id: i64,
    pattern: Option<String>,
    category: Option<String>,
    vendor: Option<String>,
    match_type: Option<String>,
    priority: Option<i64>,
) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let category_id = category
        .as_deref()
        .map(|name| resolve_category_id(&conn, name))
        .transpose()?;
    update_rule(
        &conn,
        id,
        &RuleUpdate {
            pattern,
            match_type,
            // The CLI can set a vendor but has no flag for clearing one.
            vendor: vendor.map(Some),
            category_id,
            priority,
        },
    )?;
    println!("Updated rule {id}");
    Ok(())
}

pub fn delete(id: i64) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let rule = get_rule(&conn, id)?;
    deactivate_rule(&conn, id)?;
    println!(
        "Deleted rule {id}: '{}' \u{2192} {}",
        rule.pattern, rule.category
    );
    Ok(())
}

pub fn test(pattern: &str, match_type: &str) -> Result<()> {
    // Validated before the database is touched, so a typo answers the same way
    // whether or not there is data to scan.
    validate_match_type(match_type, pattern)?;

    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let result = test_pattern(&conn, pattern, match_type)?;

    if result.total == 0 {
        println!("No transactions match pattern \"{pattern}\" ({match_type})");
        return Ok(());
    }

    println!(
        "Would match {} transaction{}:",
        result.total,
        if result.total == 1 { "" } else { "s" }
    );
    for entry in &result.matches {
        println!("  {} ({})", entry.description, entry.count);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{NewRule, RuleUpdate};
    use crate::db::{get_connection, init_db};
    use crate::error::NigelError;
    use rusqlite::Connection;

    fn test_db() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("test.db")).unwrap();
        init_db(&conn).unwrap();
        (dir, conn)
    }

    fn add_rule(conn: &Connection, pattern: &str) -> i64 {
        let cat_id: i64 = conn
            .query_row(
                "SELECT id FROM categories WHERE name = 'Software & Subscriptions'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        conn.execute(
            "INSERT INTO rules (pattern, match_type, vendor, category_id, priority, is_active) \
             VALUES (?1, 'contains', 'TestVendor', ?2, 0, 1)",
            rusqlite::params![pattern, cat_id],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    fn add_rule_with(conn: &Connection, pattern: &str, vendor: Option<&str>, priority: i64) -> i64 {
        let cat_id: i64 = conn
            .query_row(
                "SELECT id FROM categories WHERE name = 'Software & Subscriptions'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        conn.execute(
            "INSERT INTO rules (pattern, match_type, vendor, category_id, priority, is_active) \
             VALUES (?1, 'contains', ?2, ?3, ?4, 1)",
            rusqlite::params![pattern, vendor, cat_id, priority],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    fn software_id(conn: &Connection) -> i64 {
        conn.query_row(
            "SELECT id FROM categories WHERE name = 'Software & Subscriptions'",
            [],
            |r| r.get(0),
        )
        .unwrap()
    }

    #[test]
    fn deactivate_hides_a_rule_but_keeps_its_hits() {
        let (_dir, conn) = test_db();
        let id = add_rule(&conn, "ADOBE");
        conn.execute("UPDATE rules SET hit_count = 42 WHERE id = ?1", [id])
            .unwrap();

        super::deactivate_rule(&conn, id).unwrap();

        assert!(super::list_rules(&conn).unwrap().is_empty());
        let rule = super::get_rule(&conn, id).unwrap();
        assert_eq!(rule.hit_count, 42, "the record of its work survives");
    }

    #[test]
    fn a_deactivated_rule_refuses_further_edits() {
        let (_dir, conn) = test_db();
        let id = add_rule(&conn, "ADOBE");
        super::deactivate_rule(&conn, id).unwrap();

        for err in [
            super::deactivate_rule(&conn, id).unwrap_err(),
            super::update_rule(
                &conn,
                id,
                &RuleUpdate {
                    priority: Some(5),
                    ..Default::default()
                },
            )
            .unwrap_err(),
        ] {
            assert!(
                matches!(
                    err,
                    NigelError::Conflict {
                        code: "already_inactive",
                        ..
                    }
                ),
                "got: {err}"
            );
        }
    }

    #[test]
    fn a_missing_rule_is_not_found_everywhere_it_can_be_addressed() {
        let (_dir, conn) = test_db();
        for err in [
            super::get_rule(&conn, 4242).unwrap_err(),
            super::deactivate_rule(&conn, 4242).unwrap_err(),
            super::update_rule(
                &conn,
                4242,
                &RuleUpdate {
                    priority: Some(1),
                    ..Default::default()
                },
            )
            .unwrap_err(),
        ] {
            assert!(matches!(err, NigelError::NotFound(_)), "got: {err}");
            assert!(err.to_string().contains("No rule with ID 4242"));
        }
    }

    #[test]
    fn add_rule_returns_an_id_that_round_trips() {
        let (_dir, conn) = test_db();
        let category_id = software_id(&conn);

        let id = super::add_rule(
            &conn,
            NewRule {
                pattern: "FIGMA",
                category_id,
                vendor: Some("Figma"),
                match_type: "starts_with",
                priority: 7,
            },
        )
        .unwrap();

        let rule = super::get_rule(&conn, id).unwrap();
        assert_eq!(rule.pattern, "FIGMA");
        assert_eq!(rule.match_type, "starts_with");
        assert_eq!(rule.vendor.as_deref(), Some("Figma"));
        assert_eq!(rule.priority, 7);
        assert_eq!(rule.category_id, category_id);
    }

    #[test]
    fn a_rule_is_only_as_good_as_the_pattern_the_categorizer_can_run() {
        let (_dir, conn) = test_db();
        let category_id = software_id(&conn);
        let new = |pattern, match_type| NewRule {
            pattern,
            category_id,
            vendor: None,
            match_type,
            priority: 0,
        };

        // `matches()` answers false to an unknown match type and to a regex it
        // cannot compile, so either would save a rule that never fires.
        let bad_type = super::add_rule(&conn, new("X", "fuzzy")).unwrap_err();
        assert!(bad_type.to_string().contains("Invalid match type: fuzzy"));
        let bad_regex = super::add_rule(&conn, new("[unclosed", "regex")).unwrap_err();
        assert!(bad_regex.to_string().contains("Invalid regex"));
        let blank = super::add_rule(&conn, new("   ", "contains")).unwrap_err();
        assert!(blank.to_string().contains("Pattern is required"));

        // And the category has to be one the chart of accounts still offers.
        let gone =
            crate::cli::categories::add_category(&conn, "Doomed", "expense", None, None).unwrap();
        crate::cli::categories::delete_category(&conn, gone).unwrap();
        let err = super::add_rule(
            &conn,
            NewRule {
                pattern: "X",
                category_id: gone,
                vendor: None,
                match_type: "contains",
                priority: 0,
            },
        )
        .unwrap_err();
        assert!(matches!(err, NigelError::NotFound(_)), "got: {err}");
    }

    #[test]
    fn update_touches_only_the_fields_it_is_given() {
        let (_dir, conn) = test_db();
        let id = add_rule(&conn, "ADOBE");
        let before = super::get_rule(&conn, id).unwrap();

        super::update_rule(
            &conn,
            id,
            &RuleUpdate {
                pattern: Some("PHOTOSHOP".into()),
                priority: Some(10),
                ..Default::default()
            },
        )
        .unwrap();

        let after = super::get_rule(&conn, id).unwrap();
        assert_eq!(after.pattern, "PHOTOSHOP");
        assert_eq!(after.priority, 10);
        assert_eq!(after.match_type, before.match_type);
        assert_eq!(after.vendor, before.vendor);
        assert_eq!(after.category_id, before.category_id);
    }

    #[test]
    fn update_clears_a_vendor_only_when_asked_to() {
        let (_dir, conn) = test_db();
        let id = add_rule(&conn, "ADOBE");
        assert_eq!(
            super::get_rule(&conn, id).unwrap().vendor.as_deref(),
            Some("TestVendor")
        );

        // Absent leaves it alone...
        super::update_rule(
            &conn,
            id,
            &RuleUpdate {
                priority: Some(3),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(super::get_rule(&conn, id).unwrap().vendor.is_some());

        // ...an explicit "no vendor" clears it.
        super::update_rule(
            &conn,
            id,
            &RuleUpdate {
                vendor: Some(None),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(super::get_rule(&conn, id).unwrap().vendor, None);
    }

    #[test]
    fn an_empty_update_is_rejected() {
        let (_dir, conn) = test_db();
        let id = add_rule(&conn, "ADOBE");
        let err = super::update_rule(&conn, id, &RuleUpdate::default()).unwrap_err();
        assert!(err.to_string().contains("Nothing to update"));
    }

    #[test]
    fn update_revalidates_only_when_the_pattern_moves() {
        let (_dir, conn) = test_db();
        let id = add_rule(&conn, "ADOBE");

        // Switching to regex validates the stored pattern against the new type.
        super::update_rule(
            &conn,
            id,
            &RuleUpdate {
                match_type: Some("regex".into()),
                ..Default::default()
            },
        )
        .unwrap();

        // A rule whose stored regex no longer compiles can still be reprioritized
        // — otherwise a bad pattern would trap the row.
        conn.execute("UPDATE rules SET pattern = '[unclosed' WHERE id = ?1", [id])
            .unwrap();
        super::update_rule(
            &conn,
            id,
            &RuleUpdate {
                priority: Some(9),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(super::get_rule(&conn, id).unwrap().priority, 9);

        // But rewriting it to something still broken is refused.
        let err = super::update_rule(
            &conn,
            id,
            &RuleUpdate {
                pattern: Some("(also broken".into()),
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(err.to_string().contains("Invalid regex"));
    }

    #[test]
    fn test_pattern_agrees_with_the_categorizer_it_will_run_under() {
        let (_dir, conn) = test_db();
        conn.execute(
            "INSERT INTO accounts (name, account_type) VALUES ('Test', 'checking')",
            [],
        )
        .unwrap();
        let account = conn.last_insert_rowid();
        let descriptions = [
            "ADOBE CREATIVE CLOUD",
            "ADOBE CREATIVE CLOUD",
            "adobe stock",
            "GITHUB",
        ];
        for desc in descriptions {
            conn.execute(
                "INSERT INTO transactions (account_id, date, description, amount) \
                 VALUES (?1, '2025-01-01', ?2, -10.0)",
                rusqlite::params![account, desc],
            )
            .unwrap();
        }

        for (pattern, match_type) in [
            ("adobe", "contains"),
            ("ADOBE", "starts_with"),
            ("^ADOBE", "regex"),
            ("GIT", "contains"),
            ("NOTHING", "contains"),
        ] {
            let result = super::test_pattern(&conn, pattern, match_type).unwrap();
            let expected = descriptions
                .iter()
                .filter(|d| crate::categorizer::matches(d, pattern, match_type))
                .count() as i64;
            assert_eq!(
                result.total, expected,
                "{pattern} ({match_type}) should match {expected}"
            );
        }

        // Identical descriptions collapse into one entry, busiest first.
        let result = super::test_pattern(&conn, "a", "contains").unwrap();
        assert_eq!(result.total, 3);
        assert_eq!(result.matches[0].description, "ADOBE CREATIVE CLOUD");
        assert_eq!(result.matches[0].count, 2);
        assert_eq!(result.matches[1].description, "adobe stock");

        let empty = super::test_pattern(&conn, "NOTHING", "contains").unwrap();
        assert_eq!(empty.total, 0);
        assert!(empty.matches.is_empty());

        assert!(super::test_pattern(&conn, "[broken", "regex").is_err());
    }

    #[test]
    fn resolve_category_id_names_the_category_it_cannot_find() {
        let (_dir, conn) = test_db();
        assert_eq!(
            super::resolve_category_id(&conn, "Software & Subscriptions").unwrap(),
            software_id(&conn)
        );
        let err = super::resolve_category_id(&conn, "No Such Category").unwrap_err();
        assert!(matches!(err, NigelError::UnknownCategory(_)), "got: {err}");
    }

    #[test]
    fn list_rules_orders_by_priority_then_id() {
        let (_dir, conn) = test_db();
        let low = add_rule_with(&conn, "LOW", None, 0);
        let high = add_rule_with(&conn, "HIGH", None, 10);
        let also_high = add_rule_with(&conn, "ALSO", None, 10);

        let ids: Vec<i64> = super::list_rules(&conn)
            .unwrap()
            .iter()
            .map(|r| r.id)
            .collect();
        assert_eq!(ids, vec![high, also_high, low]);
    }

    #[test]
    fn list_rules_keeps_a_null_vendor_null() {
        let (_dir, conn) = test_db();
        add_rule_with(&conn, "NOVENDOR", None, 0);
        add_rule_with(&conn, "VENDOR", Some("Adobe"), 0);

        let rules = super::list_rules(&conn).unwrap();
        let by_pattern = |p: &str| {
            rules
                .iter()
                .find(|r| r.pattern == p)
                .unwrap_or_else(|| panic!("no rule {p}"))
        };
        assert_eq!(by_pattern("NOVENDOR").vendor, None);
        assert_eq!(by_pattern("VENDOR").vendor, Some("Adobe".to_string()));
    }

    #[test]
    fn list_rules_excludes_inactive_rules_and_carries_the_category() {
        let (_dir, conn) = test_db();
        let id = add_rule(&conn, "ADOBE");
        let rule = super::list_rules(&conn).unwrap().remove(0);
        assert_eq!(rule.id, id);
        assert_eq!(rule.category, "Software & Subscriptions");
        let expected_id: i64 = conn
            .query_row(
                "SELECT id FROM categories WHERE name = 'Software & Subscriptions'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(rule.category_id, expected_id);

        conn.execute("UPDATE rules SET is_active = 0 WHERE id = ?1", [id])
            .unwrap();
        assert!(super::list_rules(&conn).unwrap().is_empty());
    }

    #[test]
    fn rule_row_serializes_camel_case() {
        let (_dir, conn) = test_db();
        add_rule(&conn, "ADOBE");
        let rule = super::list_rules(&conn).unwrap().remove(0);
        let json = serde_json::to_value(&rule).unwrap();
        for key in ["matchType", "categoryId", "hitCount"] {
            assert!(json.get(key).is_some(), "missing {key} in {json}");
        }
    }

    #[test]
    fn test_inactive_rule_excluded_from_active_list() {
        let (_dir, conn) = test_db();
        let id = add_rule(&conn, "ADOBE");
        let count_before: i64 = conn
            .query_row("SELECT COUNT(*) FROM rules WHERE is_active = 1", [], |r| {
                r.get(0)
            })
            .unwrap();
        conn.execute("UPDATE rules SET is_active = 0 WHERE id = ?1", [id])
            .unwrap();
        let count_after: i64 = conn
            .query_row("SELECT COUNT(*) FROM rules WHERE is_active = 1", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(count_after, count_before - 1);
    }
}
