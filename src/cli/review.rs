use colored::Colorize;
use crossterm::{
    cursor, execute, queue,
    event::{self, Event, KeyCode, KeyModifiers},
    terminal::{self, Clear, ClearType},
};
use dialoguer::{Confirm, Input};
use std::io::{self, Write};

use crate::db::get_connection;
use crate::error::Result;
use crate::fmt::money;
use crate::reviewer::{apply_review, get_categories, get_flagged_transactions, undo_review};
use crate::settings::get_data_dir;

enum PickResult {
    Selected(usize),
    Skipped,
    Back,
}

/// Custom category picker: shows nothing until the user types, then filters inline.
/// Returns Selected(index) on Enter, Skipped on Tab, Back on Esc.
fn category_pick(prompt: &str, items: &[String], allow_back: bool) -> PickResult {
    let mut stdout = io::stdout();
    terminal::enable_raw_mode().unwrap();

    let max_visible: usize = 9;
    let mut query = String::new();
    let mut sel: usize = 0;

    let result = loop {
        // Filter — show nothing until the user types
        let matches: Vec<(usize, &str)> = if query.is_empty() {
            vec![]
        } else {
            let q = query.to_lowercase();
            items
                .iter()
                .enumerate()
                .filter(|(_, item)| item.to_lowercase().contains(&q))
                .map(|(i, s)| (i, s.as_str()))
                .take(max_visible)
                .collect()
        };

        sel = if matches.is_empty() {
            0
        } else {
            sel.min(matches.len() - 1)
        };

        // Render
        queue!(stdout, cursor::MoveToColumn(0), Clear(ClearType::FromCursorDown)).unwrap();
        write!(stdout, "{prompt}: {query}").unwrap();

        if !query.is_empty() && matches.is_empty() {
            write!(stdout, "\r\n  {}", "(no matches)".bright_black()).unwrap();
            queue!(stdout, cursor::MoveUp(1)).unwrap();
        } else {
            for (i, (_, label)) in matches.iter().enumerate() {
                let marker = if i == sel { ">" } else { " " };
                write!(stdout, "\r\n  {marker} {label}").unwrap();
            }
            if !matches.is_empty() {
                queue!(stdout, cursor::MoveUp(matches.len() as u16)).unwrap();
            }
        }

        queue!(
            stdout,
            cursor::MoveToColumn((prompt.len() + 2 + query.len()) as u16)
        )
        .unwrap();
        stdout.flush().unwrap();

        // Handle input
        if let Ok(Event::Key(key)) = event::read() {
            match key.code {
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    queue!(stdout, cursor::MoveToColumn(0), Clear(ClearType::FromCursorDown))
                        .unwrap();
                    write!(stdout, "\r\n").unwrap();
                    stdout.flush().unwrap();
                    terminal::disable_raw_mode().unwrap();
                    std::process::exit(130);
                }
                KeyCode::Char(c) => {
                    query.push(c);
                    sel = 0;
                }
                KeyCode::Backspace => {
                    query.pop();
                    sel = 0;
                }
                KeyCode::Up => sel = sel.saturating_sub(1),
                KeyCode::Down => {
                    if !matches.is_empty() {
                        sel = (sel + 1).min(matches.len() - 1);
                    }
                }
                KeyCode::Enter if !matches.is_empty() => break PickResult::Selected(matches[sel].0),
                KeyCode::Esc if allow_back => break PickResult::Back,
                KeyCode::Esc => break PickResult::Skipped,
                KeyCode::Tab => break PickResult::Skipped,
                _ => {}
            }
        }
    };

    // Clean up — show what was selected, restore terminal
    queue!(stdout, cursor::MoveToColumn(0), Clear(ClearType::FromCursorDown)).unwrap();
    match &result {
        PickResult::Selected(idx) => {
            write!(stdout, "{prompt}: {}\r\n", items[*idx]).unwrap();
        }
        PickResult::Skipped => {
            write!(stdout, "{prompt}: {}\r\n", "(skipped)".bright_black()).unwrap();
        }
        PickResult::Back => {
            write!(stdout, "{prompt}: {}\r\n", "(back)".bright_black()).unwrap();
        }
    }
    stdout.flush().unwrap();
    terminal::disable_raw_mode().unwrap();

    result
}

/// Tracks a review decision so it can be undone when navigating back.
struct ReviewDecision {
    transaction_id: i64,
    rule_id: Option<i64>,
}

pub fn run() -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let flagged = get_flagged_transactions(&conn)?;

    if flagged.is_empty() {
        println!("{}", "No flagged transactions to review.".green());
        return Ok(());
    }

    let categories = get_categories(&conn)?;
    let total = flagged.len();
    println!("\n{total} transactions to review\n");

    let labels: Vec<String> = categories
        .iter()
        .map(|c| format!("{} ({})", c.name, c.category_type))
        .collect();

    // Track decisions for undo when navigating back
    let mut decisions: Vec<Option<ReviewDecision>> = Vec::new();

    let mut stdout = io::stdout();
    let mut i: usize = 0;
    while i < total {
        let txn = &flagged[i];

        // Clear screen so the category chart is always pinned at the top
        execute!(stdout, Clear(ClearType::All), cursor::MoveTo(0, 0)).unwrap();

        // Sticky category chart (multi-column)
        let entries: Vec<String> = categories
            .iter()
            .map(|c| {
                let tag = if c.category_type == "income" { "inc" } else { "exp" };
                format!("{} ({})", c.name, tag)
            })
            .collect();
        let col_width = entries.iter().map(|e| e.len()).max().unwrap_or(20) + 2;
        let term_width = terminal_size::terminal_size()
            .map(|(w, _)| w.0 as usize)
            .unwrap_or(80);
        let cols = (term_width / col_width).max(1);
        let rows = (entries.len() + cols - 1) / cols;
        println!("{}", "Categories".bright_black());
        for row in 0..rows {
            let mut line = String::new();
            for col in 0..cols {
                let idx = col * rows + row;
                if let Some(entry) = entries.get(idx) {
                    line.push_str(&format!("{:<width$}", entry, width = col_width));
                }
            }
            println!("{}", line.bright_black());
        }
        println!();

        // Progress bar
        let bar_width = 40;
        let filled = if total > 1 {
            (i * bar_width) / (total - 1)
        } else {
            bar_width
        };
        let empty = bar_width - filled;
        println!(
            "[{} of {}] {}{}",
            i + 1,
            total,
            "\u{2501}".repeat(filled).green(),
            "\u{2501}".repeat(empty).bright_black(),
        );
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

        let allow_back = i > 0;
        let hint = if allow_back {
            "Category (type to filter, Esc=back, Tab=skip)"
        } else {
            "Category (type to filter, Tab=skip)"
        };
        let selection = category_pick(hint, &labels, allow_back);

        match selection {
            PickResult::Back => {
                // Undo the previous transaction's review if it was categorized
                i -= 1;
                if let Some(Some(decision)) = decisions.pop() {
                    undo_review(&conn, decision.transaction_id, decision.rule_id)?;
                } else {
                    decisions.pop(); // remove the None entry for skipped transactions
                }
                continue;
            }
            PickResult::Skipped => {
                decisions.push(None);
                i += 1;
                continue;
            }
            PickResult::Selected(idx) => {
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

                let rule_id = apply_review(
                    &conn,
                    txn.id,
                    cat.id,
                    vendor,
                    create_rule,
                    rule_pattern.as_deref(),
                )?;

                decisions.push(Some(ReviewDecision {
                    transaction_id: txn.id,
                    rule_id,
                }));

                println!(
                    "{}",
                    format!("\u{2192} Categorized as {}", cat.name).green()
                );
                println!();
                i += 1;
            }
        }
    }

    println!("{}", "Review complete!".green());
    Ok(())
}
