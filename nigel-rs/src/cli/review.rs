use colored::Colorize;
use comfy_table::{Cell, Table};
use dialoguer::{Confirm, Input};

use crate::db::get_connection;
use crate::error::Result;
use crate::fmt::money;
use crate::reviewer::{apply_review, get_categories, get_flagged_transactions};
use crate::settings::get_data_dir;

pub fn run() -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let flagged = get_flagged_transactions(&conn)?;

    if flagged.is_empty() {
        println!("{}", "No flagged transactions to review.".green());
        return Ok(());
    }

    let categories = get_categories(&conn)?;
    println!("\n{} transactions to review\n", flagged.len());

    // Print category list
    let mut cat_table = Table::new();
    cat_table.set_header(vec!["#", "Name", "Type"]);
    for (i, cat) in categories.iter().enumerate() {
        cat_table.add_row(vec![
            Cell::new(i + 1),
            Cell::new(&cat.name),
            Cell::new(&cat.category_type),
        ]);
    }
    println!("Categories\n{cat_table}\n");

    for txn in &flagged {
        println!("{}", "\u{2500}".repeat(60));
        println!("  Date:        {}", txn.date);
        println!("  Description: {}", txn.description);
        let amt_str = if txn.amount < 0.0 {
            money(txn.amount.abs()).red().to_string()
        } else {
            money(txn.amount.abs()).green().to_string()
        };
        println!("  Amount:      {amt_str}");
        println!("  Account:     {}", txn.account_name);
        println!();

        let choice: String = Input::new()
            .with_prompt("Category # (or s=skip, q=quit)")
            .interact_text()
            .unwrap_or_else(|_| "s".to_string());

        if choice.to_lowercase() == "q" {
            println!("{}", "Review paused.".yellow());
            return Ok(());
        }
        if choice.to_lowercase() == "s" {
            continue;
        }

        let idx: usize = match choice.parse::<usize>() {
            Ok(n) if n >= 1 && n <= categories.len() => n - 1,
            _ => {
                println!("{}", "Invalid choice, skipping.".red());
                continue;
            }
        };
        let cat = &categories[idx];

        let vendor: String = Input::new()
            .with_prompt("Vendor name (Enter to skip)")
            .default(String::new())
            .interact_text()
            .unwrap_or_default();
        let vendor = if vendor.is_empty() {
            None
        } else {
            Some(vendor.as_str())
        };

        let create_rule = Confirm::new()
            .with_prompt("Create rule for future matches?")
            .default(false)
            .interact()
            .unwrap_or(false);

        let mut rule_pattern = None;
        if create_rule {
            let words: Vec<&str> = txn.description.split_whitespace().collect();
            let suggested = if words.len() >= 2 {
                format!("{} {}", words[0], words[1])
            } else {
                words.first().unwrap_or(&"").to_string()
            };
            let pattern: String = Input::new()
                .with_prompt("Rule pattern")
                .default(suggested)
                .interact_text()
                .unwrap_or_default();
            rule_pattern = Some(pattern);
        }

        apply_review(
            &conn,
            txn.id,
            cat.id,
            vendor,
            create_rule,
            rule_pattern.as_deref(),
        )?;
        println!("{}", format!("\u{2192} Categorized as {}", cat.name).green());
        println!();
    }

    println!("{}", "Review complete!".green());
    Ok(())
}
