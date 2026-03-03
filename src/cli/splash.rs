use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::Paragraph,
    Frame,
};
use zeroize::Zeroize;

use crate::effects::{self, Particle, LOGO};
use crate::error::Result;
use crate::tui;

const SPLASH_DURATION: Duration = Duration::from_millis(1500);
const TICK_INTERVAL: Duration = Duration::from_millis(50);
const REVEAL_MS: f64 = 500.0;
const MAX_PASSWORD_ATTEMPTS: u8 = 3;

struct Splash {
    phase: f64,
    particles: Vec<Particle>,
    width: u16,
    height: u16,
    start: Instant,
    reveal_order: Vec<(usize, usize)>,
    // Password mode fields
    password_mode: bool,
    db_path: Option<PathBuf>,
    password: String,
    cursor: usize,
    error_msg: Option<String>,
    attempts: u8,
}

impl Splash {
    fn new() -> Self {
        let (width, height) = crossterm::terminal::size().unwrap_or((80, 24));
        Self {
            phase: 0.0,
            particles: effects::pre_seed_particles(width, height),
            width,
            height,
            start: Instant::now(),
            reveal_order: effects::logo_reveal_order(),
            password_mode: false,
            db_path: None,
            password: String::new(),
            cursor: 0,
            error_msg: None,
            attempts: 0,
        }
    }

    fn new_with_password(db_path: &Path) -> Self {
        let mut splash = Self::new();
        splash.password_mode = true;
        splash.db_path = Some(db_path.to_path_buf());
        splash
    }

    fn is_expired(&self) -> bool {
        if self.password_mode {
            return false;
        }
        self.start.elapsed() >= SPLASH_DURATION
    }

    fn logo_revealed(&self) -> bool {
        self.start.elapsed().as_secs_f64() * 1000.0 >= REVEAL_MS
    }

    fn tick(&mut self) {
        self.phase += 1.0 / 70.0;
        effects::tick_particles(&mut self.particles, self.width, self.height);
    }

    fn draw(&mut self, frame: &mut Frame) {
        let area = frame.area();
        self.width = area.width;
        self.height = area.height;

        let elapsed_ms = self.start.elapsed().as_secs_f64() * 1000.0;

        effects::render_particles(&self.particles, frame, area);

        let logo_height = LOGO.len() as u16;

        if self.password_mode {
            // Layout with password input below logo
            let [_top, logo_area, _gap, pw_area, _bottom, version_area] = Layout::vertical([
                Constraint::Fill(1),
                Constraint::Length(logo_height),
                Constraint::Length(2),
                Constraint::Length(4),
                Constraint::Fill(1),
                Constraint::Length(1),
            ])
            .areas(area);

            let total_chars = self.reveal_order.len();
            let chars_visible = if elapsed_ms < REVEAL_MS {
                let progress = elapsed_ms / REVEAL_MS;
                (progress * total_chars as f64) as usize
            } else {
                total_chars
            };

            effects::render_logo_reveal(
                self.phase,
                frame,
                logo_area,
                Some((&self.reveal_order, chars_visible)),
            );

            if elapsed_ms >= REVEAL_MS {
                tui::render_version(frame, version_area);
                self.draw_password_input(frame, pw_area);
            }
        } else {
            // Original layout
            let [_top, logo_area, _bottom, version_area] = Layout::vertical([
                Constraint::Fill(1),
                Constraint::Length(logo_height),
                Constraint::Fill(1),
                Constraint::Length(1),
            ])
            .areas(area);

            let total_chars = self.reveal_order.len();
            let chars_visible = if elapsed_ms < REVEAL_MS {
                let progress = elapsed_ms / REVEAL_MS;
                (progress * total_chars as f64) as usize
            } else {
                total_chars
            };

            effects::render_logo_reveal(
                self.phase,
                frame,
                logo_area,
                Some((&self.reveal_order, chars_visible)),
            );

            if elapsed_ms >= REVEAL_MS {
                tui::render_version(frame, version_area);
            }
        }
    }

    fn draw_password_input(&self, frame: &mut Frame, area: Rect) {
        let form_width = 40u16.min(area.width.saturating_sub(4));
        let form_x = area.x + (area.width.saturating_sub(form_width)) / 2;
        let centered = Rect::new(form_x, area.y, form_width, area.height);

        let [label_area, input_area, _gap, error_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(centered);

        // Label
        frame.render_widget(
            Paragraph::new(Span::styled(
                "Enter password:",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ))
            .alignment(Alignment::Center),
            label_area,
        );

        // Masked input with cursor
        let masked: String = "\u{25cf}".repeat(self.password.chars().count());
        let cursor_display = format!("{masked}\u{2588}");
        let width = input_area.width as usize;
        let padded = format!("{:<width$}", cursor_display, width = width);
        frame.render_widget(
            Paragraph::new(Span::styled(padded, tui::SELECTED_STYLE)).alignment(Alignment::Left),
            input_area,
        );

        // Error message
        if let Some(ref msg) = self.error_msg {
            frame.render_widget(
                Paragraph::new(Span::styled(msg.as_str(), Style::default().fg(Color::Red)))
                    .alignment(Alignment::Center),
                error_area,
            );
        }
    }

    /// Attempt to unlock the database with the current password.
    /// On success, sets the global database password and returns `Ok(true)`.
    /// On wrong password, increments the attempt counter, sets an error message,
    /// clears the password field, and returns `Ok(false)`.
    /// Infrastructure errors (I/O, permissions) are propagated as `Err`.
    fn try_password(&mut self) -> Result<bool> {
        if let Some(ref db_path) = self.db_path {
            match crate::db::validate_password(db_path, &self.password) {
                Ok(true) => {
                    crate::db::set_db_password(Some(self.password.clone()));
                    return Ok(true);
                }
                Ok(false) => {
                    self.attempts += 1;
                    if self.attempts >= MAX_PASSWORD_ATTEMPTS {
                        self.error_msg = Some("Failed to unlock database after 3 attempts.".into());
                    } else {
                        self.error_msg =
                            Some(format!("Wrong password. Try again ({}/3).", self.attempts));
                    }
                    self.password.zeroize();
                    self.password = String::new();
                    self.cursor = 0;
                }
                Err(e) => return Err(e),
            }
        }
        Ok(false)
    }
}

impl Drop for Splash {
    fn drop(&mut self) {
        self.password.zeroize();
    }
}

/// Run the splash screen. Blocks for up to 1.5 seconds; any keypress dismisses early.
pub fn run() -> Result<()> {
    let mut splash = Splash::new();
    let mut terminal = ratatui::init();

    let result: Result<()> = loop {
        if let Err(e) = terminal.draw(|frame| splash.draw(frame)) {
            break Err(e.into());
        }

        if splash.is_expired() {
            break Ok(());
        }

        if event::poll(TICK_INTERVAL)? {
            match event::read() {
                Err(e) => break Err(e.into()),
                Ok(Event::Key(key)) => {
                    if key.kind == KeyEventKind::Press {
                        break Ok(());
                    }
                }
                _ => {}
            }
        }

        splash.tick();
    };

    ratatui::restore();
    result
}

/// Run the splash screen with inline password input for encrypted databases.
/// Holds the screen until correct password is entered or 3 attempts are exhausted.
/// Sets the global database password on success. Esc cancels and returns an error.
pub fn run_with_password(db_path: &Path) -> Result<()> {
    let mut splash = Splash::new_with_password(db_path);
    let mut terminal = ratatui::init();

    let result: Result<()> = loop {
        if let Err(e) = terminal.draw(|frame| splash.draw(frame)) {
            break Err(e.into());
        }

        if event::poll(TICK_INTERVAL)? {
            match event::read() {
                Err(e) => break Err(e.into()),
                Ok(Event::Key(key)) => {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }
                    // Before logo is revealed, ignore all input
                    if !splash.logo_revealed() {
                        continue;
                    }
                    match key.code {
                        KeyCode::Enter => {
                            if splash.password.is_empty() {
                                continue;
                            }
                            match splash.try_password() {
                                Ok(true) => break Ok(()),
                                Ok(false) => {
                                    if splash.attempts >= MAX_PASSWORD_ATTEMPTS {
                                        break Err(crate::error::NigelError::Other(
                                            "Failed to unlock database after 3 attempts.".into(),
                                        ));
                                    }
                                }
                                Err(e) => break Err(e),
                            }
                        }
                        KeyCode::Char(c) => {
                            splash.password.push(c);
                            splash.cursor += 1;
                        }
                        KeyCode::Backspace => {
                            if splash.cursor > 0 {
                                splash.password.pop();
                                splash.cursor -= 1;
                            }
                        }
                        KeyCode::Esc => {
                            break Err(crate::error::NigelError::Other(
                                "Password entry cancelled.".into(),
                            ));
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }

        splash.tick();
    };

    ratatui::restore();
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splash_starts_not_expired() {
        let splash = Splash::new();
        assert!(!splash.is_expired());
    }

    #[test]
    fn splash_has_pre_seeded_particles() {
        let splash = Splash::new();
        // Visible + queued below viewport
        assert_eq!(splash.particles.len(), effects::MAX_PARTICLES * 2);
    }

    #[test]
    fn splash_tick_advances_phase() {
        let mut splash = Splash::new();
        let phase_before = splash.phase;
        splash.tick();
        assert!(splash.phase > phase_before);
    }

    #[test]
    fn splash_duration_is_1500ms() {
        assert_eq!(SPLASH_DURATION, Duration::from_millis(1500));
    }

    #[test]
    fn reveal_order_contains_all_logo_chars() {
        let splash = Splash::new();
        let width = effects::max_logo_width();
        let expected: usize = LOGO
            .iter()
            .map(|line| {
                format!("{:<width$}", line, width = width)
                    .chars()
                    .filter(|c| *c != ' ')
                    .count()
            })
            .sum();
        assert_eq!(splash.reveal_order.len(), expected);
    }

    #[test]
    fn password_mode_never_expires() {
        let splash = Splash::new_with_password(Path::new("/tmp/test.db"));
        assert!(splash.password_mode);
        assert!(!splash.is_expired());
    }

    #[test]
    fn password_mode_fields_initialized() {
        let splash = Splash::new_with_password(Path::new("/tmp/test.db"));
        assert!(splash.password.is_empty());
        assert_eq!(splash.cursor, 0);
        assert!(splash.error_msg.is_none());
        assert_eq!(splash.attempts, 0);
        assert_eq!(splash.db_path.as_deref(), Some(Path::new("/tmp/test.db")));
    }

    /// Create an encrypted test DB using open_connection (avoids global password state).
    fn create_encrypted_test_db(db_path: &Path, password: &str) {
        let conn = crate::db::open_connection(db_path, Some(password)).unwrap();
        crate::db::init_db(&conn).unwrap();
        drop(conn);
    }

    #[test]
    fn try_password_increments_attempts_on_failure() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("encrypted.db");
        create_encrypted_test_db(&db_path, "secret");

        let mut splash = Splash::new_with_password(&db_path);
        splash.password = "wrong".into();
        assert!(!splash.try_password().unwrap());
        assert_eq!(splash.attempts, 1);
        assert!(splash
            .error_msg
            .as_ref()
            .unwrap()
            .contains("Try again (1/3)"));
        assert!(splash.password.is_empty());
        assert_eq!(splash.cursor, 0);
    }

    #[test]
    fn try_password_succeeds_with_correct_password() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("encrypted.db");
        create_encrypted_test_db(&db_path, "secret");

        let mut splash = Splash::new_with_password(&db_path);
        splash.password = "secret".into();
        assert!(splash.try_password().unwrap());
        assert_eq!(splash.attempts, 0);
        // try_password sets the global password on success; clean up immediately
        crate::db::set_db_password(None);
    }

    #[test]
    fn try_password_locks_out_after_max_attempts() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("encrypted.db");
        create_encrypted_test_db(&db_path, "secret");

        let mut splash = Splash::new_with_password(&db_path);
        for i in 1..=3 {
            splash.password = "wrong".into();
            assert!(!splash.try_password().unwrap());
            assert_eq!(splash.attempts, i);
        }
        assert!(splash.error_msg.as_ref().unwrap().contains("3 attempts"));
    }

    #[test]
    fn try_password_without_db_path_returns_false() {
        let mut splash = Splash::new();
        splash.password = "test".into();
        assert!(!splash.try_password().unwrap());
        assert_eq!(splash.attempts, 0);
        assert!(splash.error_msg.is_none());
    }

    #[test]
    fn password_zeroized_on_drop() {
        let mut splash = Splash::new_with_password(Path::new("/tmp/test.db"));
        splash.password = "sensitive".into();
        drop(splash);
        // Can't directly test zeroization of dropped value, but we verify
        // the Drop impl exists and compiles
    }
}
