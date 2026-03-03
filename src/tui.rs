use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::error::Result;
use crate::fmt::money;

pub const VERSION: &str = concat!("v", env!("CARGO_PKG_VERSION"));

pub const HEADER_STYLE: Style = Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD);

pub const FOOTER_STYLE: Style = Style::new().fg(Color::DarkGray);

pub const GREEN: Color = Color::Rgb(80, 220, 100);
pub const AMOUNT_POS_STYLE: Style = Style::new().fg(GREEN);
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

pub fn render_version(frame: &mut Frame, area: Rect) {
    frame.render_widget(
        Paragraph::new(Span::styled(VERSION, FOOTER_STYLE))
            .alignment(ratatui::layout::Alignment::Center),
        area,
    );
}

// ---------------------------------------------------------------------------
// Report view infrastructure
// ---------------------------------------------------------------------------

pub enum ReportViewAction {
    Continue,
    Close,
    /// Request data reload (e.g. after date navigation). The dashboard intercepts
    /// this to rebuild the view with new date params. In standalone CLI mode
    /// (`run_report_view`), Reload is treated as Continue — the title updates
    /// but data is not rebuilt since there is no outer controller to do so.
    Reload,
}

pub trait ReportView {
    fn draw(&mut self, frame: &mut Frame);
    fn handle_key(&mut self, code: KeyCode) -> ReportViewAction;
    /// Returns the current date parameters for this view: (year, optional month string).
    /// Used by the dashboard to pass the selected period to exports and rebuilds.
    fn date_params(&self) -> (Option<i32>, Option<String>) {
        (None, None)
    }
}

/// Run an interactive ratatui report view. Sets up the terminal, event loop,
/// and panic hook, then restores the terminal on exit.
pub fn run_report_view(view: &mut dyn ReportView) -> Result<()> {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        ratatui::restore();
        hook(info);
    }));

    let mut terminal = ratatui::init();

    let result: Result<()> = loop {
        if let Err(e) = terminal.draw(|frame| view.draw(frame)) {
            break Err(e.into());
        }

        match event::read() {
            Err(e) => break Err(e.into()),
            Ok(Event::Key(key)) => {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                    break Ok(());
                }
                match view.handle_key(key.code) {
                    ReportViewAction::Close => break Ok(()),
                    ReportViewAction::Continue | ReportViewAction::Reload => {}
                }
            }
            _ => {}
        }
    };

    drop(terminal);
    ratatui::restore();
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_string_matches_cargo_pkg() {
        assert_eq!(VERSION, concat!("v", env!("CARGO_PKG_VERSION")));
        assert!(VERSION.starts_with("v"));
    }
}
