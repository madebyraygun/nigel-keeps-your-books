use super::parse_month_opt;
use crate::browser::RegisterBrowser;
use crate::db::get_connection;
use crate::error::Result;
use crate::reports;
use crate::reviewer::get_categories;
use crate::settings::get_data_dir;

pub fn register(
    month: Option<String>,
    year: Option<i32>,
    from_date: Option<String>,
    to_date: Option<String>,
    account: Option<String>,
) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let (my, mm) = parse_month_opt(&month);
    let y = year.or(my);
    let data = reports::get_register(
        &conn,
        y,
        mm,
        from_date.as_deref(),
        to_date.as_deref(),
        account.as_deref(),
    )?;

    // Build filters description — show effective values
    let mut filters = Vec::new();
    if let Some(ref m) = month {
        filters.push(format!("month: {m}"));
        // If --year was also passed and differs from the month's year, show it
        if let Some(yr) = year {
            if my != Some(yr) {
                filters.push(format!("year: {yr}"));
            }
        }
    } else if let Some(yr) = y {
        filters.push(format!("year: {yr}"));
    }
    if let Some(ref from) = from_date {
        filters.push(format!("from: {from}"));
    }
    if let Some(ref to) = to_date {
        filters.push(format!("to: {to}"));
    }
    if let Some(ref acct) = account {
        filters.push(format!("account: {acct}"));
    }
    let filters_desc = filters.join(", ");

    let no_filters = filters_desc.is_empty();
    let total = data.total;
    let categories = get_categories(&conn).unwrap_or_default();
    let desc = if no_filters { "all transactions".to_string() } else { filters_desc };
    let mut browser = RegisterBrowser::new(data.rows, total, desc, categories);
    if no_filters {
        browser.scroll_to_today();
    }
    browser.run(&conn)?;
    Ok(())
}
