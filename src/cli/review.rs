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
use crate::reviewer::{apply_review, get_categories, get_flagged_transactions};
use crate::settings::get_data_dir;

/// Custom category picker: shows nothing until the user types, then filters inline.
/// Returns Some(index) on Enter, None on Esc.
fn category_pick(prompt: &str, items: &[String]) -> Option<usize> {
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
                KeyCode::Enter if !matches.is_empty() => break Some(matches[sel].0),
                KeyCode::Esc => break None,
                _ => {}
            }
        }
    };

    // Clean up — show what was selected, restore terminal
    queue!(stdout, cursor::MoveToColumn(0), Clear(ClearType::FromCursorDown)).unwrap();
    if let Some(idx) = result {
        write!(stdout, "{prompt}: {}\r\n", items[idx]).unwrap();
    } else {
        write!(stdout, "{prompt}: {}\r\n", "(skipped)".bright_black()).unwrap();
    }
    stdout.flush().unwrap();
    terminal::disable_raw_mode().unwrap();

    result
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

    let mut stdout = io::stdout();
    for (i, txn) in flagged.iter().enumerate() {
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
        let term_width = crossterm::terminal::size()
            .map(|(w, _)| w as usize)
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

        let selection = category_pick("Category (type to filter, Esc=skip)", &labels);

        let idx = match selection {
            Some(idx) => idx,
            None => continue,
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
        println!(
            "{}",
            format!("\u{2192} Categorized as {}", cat.name).green()
        );
        println!();
    }

    println!("{}", "Review complete!".green());
    Ok(())
}
