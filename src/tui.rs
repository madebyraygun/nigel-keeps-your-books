use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;

use crate::fmt::money;

pub const HEADER_STYLE: Style = Style::new()
    .fg(Color::Yellow)
    .add_modifier(Modifier::BOLD);

pub const FOOTER_STYLE: Style = Style::new().fg(Color::DarkGray);

pub const AMOUNT_POS_STYLE: Style = Style::new().fg(Color::Rgb(80, 220, 100));
pub const AMOUNT_NEG_STYLE: Style = Style::new().fg(Color::Red);

pub const SELECTED_STYLE: Style = Style::new()
    .bg(Color::Rgb(40, 40, 60))
    .add_modifier(Modifier::BOLD);

/// Format an amount as a colored Span (green for income, red for expense).
/// Shows absolute value — color conveys the sign.
pub fn money_span(amount: f64) -> Span<'static> {
    let style = if amount < 0.0 {
        AMOUNT_NEG_STYLE
    } else {
        AMOUNT_POS_STYLE
    };
    Span::styled(money(amount.abs()), style)
}

/// Wrap text to a given width. Returns (wrapped_string, line_count).
pub fn wrap_text(text: &str, width: usize) -> (String, u16) {
    if width == 0 {
        return (text.to_string(), 1);
    }
    let wrapped = textwrap::fill(text, width);
    let lines = wrapped.lines().count().max(1) as u16;
    (wrapped, lines)
}
