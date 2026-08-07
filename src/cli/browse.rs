use super::{parse_month_opt, RegisterFilterArgs};
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
    filters: &RegisterFilterArgs,
) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let (my, mm) = parse_month_opt(&month);
    let y = year.or(my);
    let filters = filters.resolve(&conn)?;
    let data = reports::get_register(
        &conn,
        y,
        mm,
        from_date.as_deref(),
        to_date.as_deref(),
        &filters,
    )?;

    // Build filters description — show effective values
    let mut desc_parts = Vec::new();
    if let Some(ref m) = month {
        desc_parts.push(format!("month: {m}"));
        // If --year was also passed and differs from the month's year, show it
        if let Some(yr) = year {
            if my != Some(yr) {
                desc_parts.push(format!("year: {yr}"));
            }
        }
    } else if let Some(yr) = y {
        desc_parts.push(format!("year: {yr}"));
    }
    if let Some(ref from) = from_date {
        desc_parts.push(format!("from: {from}"));
    }
    if let Some(ref to) = to_date {
        desc_parts.push(format!("to: {to}"));
    }
    desc_parts.extend(filters.labels());
    let filters_desc = desc_parts.join(", ");

    let no_date_filters = y.is_none() && mm.is_none() && from_date.is_none() && to_date.is_none();
    let total = data.total;
    let categories = get_categories(&conn).unwrap_or_default();
    let desc = if filters_desc.is_empty() {
        "all transactions".to_string()
    } else {
        filters_desc
    };
    let mut browser = RegisterBrowser::new(data.rows, total, desc, categories);
    if no_date_filters {
        browser.scroll_to_today();
    }
    browser.run(&conn)?;
    Ok(())
}
