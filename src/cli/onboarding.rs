use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use zeroize::Zeroize;

use crate::effects::{self, Particle, LOGO};
use crate::error::Result;
use crate::tui::{FOOTER_STYLE, HEADER_STYLE, SELECTED_STYLE};

/// What the user chose to do after onboarding.
#[derive(Clone, Copy)]
pub enum PostSetupAction {
    Demo,
    StartFresh,
    Import,
}

const ACTION_ITEMS: &[&str] = &[
    "View the demo",
    "Start from scratch",
    "Load existing data directory",
];

/// Intro animation timing (milliseconds)
const INTRO_PARTICLES_MS: f64 = 500.0;
const INTRO_REVEAL_MS: f64 = 500.0;
const INTRO_UI_DELAY_MS: f64 = 200.0;
const INTRO_TOTAL_MS: f64 = INTRO_PARTICLES_MS + INTRO_REVEAL_MS + INTRO_UI_DELAY_MS;

const FIELD_NAME: usize = 0;
const FIELD_COMPANY: usize = 1;
const FIELD_PASSWORD: usize = 2;
const FIELD_BUTTON: usize = 3;

enum Screen {
    NameInput,
    ConfirmPassword,
    ActionPicker,
}

enum StepResult {
    Continue,
    NextScreen,
    Finish,
    Skip,
}

pub struct OnboardingResult {
    pub user_name: String,
    pub company_name: String,
    pub password: Option<String>,
    pub action: PostSetupAction,
}

struct Onboarding {
    user_name: String,
    company_name: String,
    password: String,
    confirm_password: String,
    confirm_cursor: usize,
    confirm_mismatch: bool,
    active_field: usize,
    cursor_pos: usize,
    action_selection: usize,
    screen: Screen,
    phase: f64,
    particles: Vec<Particle>,
    width: u16,
    height: u16,
    start: Instant,
    reveal_order: Vec<(usize, usize)>,
    intro_done: bool,
}

impl Onboarding {
    fn new() -> Self {
        let (width, height) = crossterm::terminal::size().unwrap_or((80, 24));
        Self {
            user_name: String::new(),
            company_name: String::new(),
            password: String::new(),
            confirm_password: String::new(),
            confirm_cursor: 0,
            confirm_mismatch: false,
            active_field: 0,
            cursor_pos: 0,
            action_selection: 0,
            screen: Screen::NameInput,
            phase: 0.0,
            particles: effects::pre_seed_particles(width, height),
            width,
            height,
            start: Instant::now(),
            reveal_order: effects::logo_reveal_order(),
            intro_done: false,
        }
    }

    fn active_value(&self) -> &str {
        match self.active_field {
            FIELD_NAME => &self.user_name,
            FIELD_COMPANY => &self.company_name,
            _ => &self.password,
        }
    }

    fn active_value_mut(&mut self) -> &mut String {
        match self.active_field {
            FIELD_NAME => &mut self.user_name,
            FIELD_COMPANY => &mut self.company_name,
            _ => &mut self.password,
        }
    }

    fn tick(&mut self) {
        self.phase += 1.0 / 70.0;
        effects::tick_particles(&mut self.particles, self.width, self.height);
    }

    fn draw(&mut self, frame: &mut Frame) {
        let area = frame.area();
        self.width = area.width;
        self.height = area.height;

        // Draw particles as background
        effects::render_particles(&self.particles, frame, area);

        // Auto-complete intro when animation finishes
        if !self.intro_done {
            let elapsed = self.start.elapsed().as_secs_f64() * 1000.0;
            if elapsed >= INTRO_TOTAL_MS {
                self.intro_done = true;
            }
        }

        match self.screen {
            Screen::NameInput => self.draw_name_input(frame, area),
            Screen::ConfirmPassword => self.draw_confirm_password(frame, area),
            Screen::ActionPicker => self.draw_action_picker(frame, area),
        }
    }

    fn draw_name_input(&self, frame: &mut Frame, area: Rect) {
        let logo_height = LOGO.len() as u16;
        let [_top_pad, logo_area, _gap1, welcome_area, _gap2, form_area, _gap3, button_area, _gap4, hints_area, _bottom_pad] =
            Layout::vertical([
                Constraint::Fill(1),
                Constraint::Length(logo_height),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(3),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Fill(1),
            ])
            .areas(area);

        if self.intro_done {
            effects::render_logo(self.phase, frame, logo_area);
        } else {
            let elapsed_ms = self.start.elapsed().as_secs_f64() * 1000.0;
            if elapsed_ms < INTRO_PARTICLES_MS {
                // Particles only — no logo or UI yet
                return;
            }
            // Logo reveal phase
            let logo_elapsed = elapsed_ms - INTRO_PARTICLES_MS;
            if logo_elapsed < INTRO_REVEAL_MS {
                let progress = logo_elapsed / INTRO_REVEAL_MS;
                let total = self.reveal_order.len();
                let chars_visible = (progress * total as f64) as usize;
                effects::render_logo_reveal(
                    self.phase,
                    frame,
                    logo_area,
                    Some((&self.reveal_order, chars_visible)),
                );
            } else {
                // Logo fully revealed, waiting for UI delay
                effects::render_logo(self.phase, frame, logo_area);
            }
            // UI not shown yet during intro
            return;
        }

        frame.render_widget(
            Paragraph::new(Span::styled("Welcome! Let's get you set up.", HEADER_STYLE))
                .alignment(ratatui::layout::Alignment::Center),
            welcome_area,
        );

        let form_width = 50u16.min(area.width.saturating_sub(4));
        let form_x = area.x + (area.width.saturating_sub(form_width)) / 2;
        let centered_form = Rect::new(form_x, form_area.y, form_width, form_area.height);

        let [name_row, biz_row, pw_row] =
            Layout::vertical([Constraint::Length(1), Constraint::Length(1), Constraint::Length(1)])
                .areas(centered_form);

        self.draw_field(frame, name_row, "Your name:", &self.user_name, FIELD_NAME, false);
        self.draw_field(frame, biz_row, "Business name:", &self.company_name, FIELD_COMPANY, false);
        self.draw_field(frame, pw_row, "Set password (optional):", &self.password, FIELD_PASSWORD, true);

        // Continue button
        let btn_style = if self.active_field == FIELD_BUTTON {
            Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        frame.render_widget(
            Paragraph::new(Span::styled("[ Continue ]", btn_style))
                .alignment(ratatui::layout::Alignment::Center),
            button_area,
        );

        frame.render_widget(
            Paragraph::new(" Enter=next  Esc=skip")
                .style(FOOTER_STYLE)
                .alignment(ratatui::layout::Alignment::Center),
            hints_area,
        );
    }

    fn draw_action_picker(&self, frame: &mut Frame, area: Rect) {
        let logo_height = LOGO.len() as u16;
        let menu_height = ACTION_ITEMS.len() as u16;
        let [_top_pad, logo_area, _gap1, prompt_area, _gap2, menu_area, _gap3, hints_area, _bottom_pad] =
            Layout::vertical([
                Constraint::Fill(1),
                Constraint::Length(logo_height),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(menu_height),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Fill(1),
            ])
            .areas(area);

        effects::render_logo(self.phase, frame, logo_area);

        frame.render_widget(
            Paragraph::new(Span::styled("How would you like to start?", HEADER_STYLE))
                .alignment(ratatui::layout::Alignment::Center),
            prompt_area,
        );

        let menu_width = 50u16.min(area.width.saturating_sub(4));
        let menu_x = area.x + (area.width.saturating_sub(menu_width)) / 2;
        let centered_menu = Rect::new(menu_x, menu_area.y, menu_width, menu_area.height);

        let menu_lines: Vec<Line> = ACTION_ITEMS
            .iter()
            .enumerate()
            .map(|(i, label)| {
                let marker = if i == self.action_selection { ">" } else { " " };
                let style = if i == self.action_selection {
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                Line::from(Span::styled(format!(" {marker} {label}"), style))
            })
            .collect();
        frame.render_widget(Paragraph::new(menu_lines), centered_menu);

        frame.render_widget(
            Paragraph::new(" Up/Down=navigate  Enter=select")
                .style(FOOTER_STYLE)
                .alignment(ratatui::layout::Alignment::Center),
            hints_area,
        );
    }

    fn draw_confirm_password(&self, frame: &mut Frame, area: Rect) {
        let logo_height = LOGO.len() as u16;
        let error_height = if self.confirm_mismatch { 1 } else { 0 };
        let [_top_pad, logo_area, _gap1, prompt_area, _gap2, field_area, error_area, _gap3, hints_area, _bottom_pad] =
            Layout::vertical([
                Constraint::Fill(1),
                Constraint::Length(logo_height),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(error_height),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Fill(1),
            ])
            .areas(area);

        effects::render_logo(self.phase, frame, logo_area);

        frame.render_widget(
            Paragraph::new(Span::styled("Confirm your password", HEADER_STYLE))
                .alignment(ratatui::layout::Alignment::Center),
            prompt_area,
        );

        let form_width = 50u16.min(area.width.saturating_sub(4));
        let form_x = area.x + (area.width.saturating_sub(form_width)) / 2;
        let centered_field = Rect::new(form_x, field_area.y, form_width, field_area.height);

        // Render the masked confirm input inline (reuse draw logic but with confirm state)
        let label_width = 16u16;
        let [label_area, input_area] = Layout::horizontal([
            Constraint::Length(label_width),
            Constraint::Fill(1),
        ])
        .areas(centered_field);

        frame.render_widget(
            Paragraph::new(Span::styled(
                format!("{:<width$}", "Password:", width = label_width as usize),
                Style::default().add_modifier(Modifier::BOLD),
            )),
            label_area,
        );

        let input_width = input_area.width as usize;
        let display = insert_cursor(&self.confirm_password, self.confirm_cursor, true);
        let padded = format!("{:<width$}", display, width = input_width);
        frame.render_widget(
            Paragraph::new(Span::styled(padded, SELECTED_STYLE)),
            input_area,
        );

        if self.confirm_mismatch {
            frame.render_widget(
                Paragraph::new(Span::styled(
                    "Passwords do not match. Try again.",
                    Style::default().fg(Color::Red),
                ))
                .alignment(ratatui::layout::Alignment::Center),
                error_area,
            );
        }

        frame.render_widget(
            Paragraph::new(" Enter=confirm  Esc=back")
                .style(FOOTER_STYLE)
                .alignment(ratatui::layout::Alignment::Center),
            hints_area,
        );
    }

    /// Convert a char-index cursor position to a byte offset in the string.
    fn cursor_byte_pos(&self) -> usize {
        self.active_value()
            .char_indices()
            .nth(self.cursor_pos)
            .map(|(i, _)| i)
            .unwrap_or(self.active_value().len())
    }

    fn draw_field(
        &self,
        frame: &mut Frame,
        area: Rect,
        label: &str,
        value: &str,
        field_idx: usize,
        masked: bool,
    ) {
        let label_width = 16u16;
        let [label_area, input_area] = Layout::horizontal([
            Constraint::Length(label_width),
            Constraint::Fill(1),
        ])
        .areas(area);

        let label_style = if self.active_field == field_idx {
            Style::default().add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        frame.render_widget(
            Paragraph::new(Span::styled(
                format!("{:<width$}", label, width = label_width as usize),
                label_style,
            )),
            label_area,
        );

        let input_width = input_area.width as usize;
        let is_active = self.active_field == field_idx;

        let display = if is_active {
            insert_cursor(value, self.cursor_pos, masked)
        } else if masked {
            "\u{25cf}".repeat(value.chars().count())
        } else {
            value.to_string()
        };

        let padded = format!("{:<width$}", display, width = input_width);

        let style = if is_active {
            SELECTED_STYLE
        } else {
            Style::default().fg(Color::DarkGray)
        };

        frame.render_widget(Paragraph::new(Span::styled(padded, style)), input_area);
    }

    fn move_to_field(&mut self, field: usize) {
        self.active_field = field;
        if field <= FIELD_PASSWORD {
            self.cursor_pos = self.active_value().chars().count();
        }
    }

    fn handle_name_key(&mut self, code: KeyCode) -> StepResult {
        // On the button, only handle navigation and submit
        if self.active_field == FIELD_BUTTON {
            match code {
                KeyCode::Enter => return StepResult::NextScreen,
                KeyCode::Up => self.move_to_field(FIELD_PASSWORD),
                KeyCode::Esc => return StepResult::Skip,
                _ => {}
            }
            return StepResult::Continue;
        }

        // Text input fields
        match code {
            KeyCode::Enter | KeyCode::Down => {
                self.move_to_field(self.active_field + 1);
            }
            KeyCode::Up => {
                if self.active_field > 0 {
                    self.move_to_field(self.active_field - 1);
                }
            }
            KeyCode::Esc => return StepResult::Skip,
            KeyCode::Char(c) => {
                let byte_pos = self.cursor_byte_pos();
                let field = self.active_value_mut();
                field.insert(byte_pos, c);
                self.cursor_pos += 1;
            }
            KeyCode::Backspace => {
                if self.cursor_pos > 0 {
                    self.cursor_pos -= 1;
                    let byte_pos = self.cursor_byte_pos();
                    let field = self.active_value_mut();
                    field.remove(byte_pos);
                }
            }
            KeyCode::Delete => {
                let char_len = self.active_value().chars().count();
                if self.cursor_pos < char_len {
                    let byte_pos = self.cursor_byte_pos();
                    let field = self.active_value_mut();
                    field.remove(byte_pos);
                }
            }
            KeyCode::Left => {
                self.cursor_pos = self.cursor_pos.saturating_sub(1);
            }
            KeyCode::Right => {
                let char_len = self.active_value().chars().count();
                self.cursor_pos = (self.cursor_pos + 1).min(char_len);
            }
            KeyCode::Home => self.cursor_pos = 0,
            KeyCode::End => self.cursor_pos = self.active_value().chars().count(),
            _ => {}
        }
        StepResult::Continue
    }

    fn handle_confirm_key(&mut self, code: KeyCode) -> StepResult {
        match code {
            KeyCode::Enter => {
                if self.confirm_password.trim() == self.password.trim() {
                    self.confirm_mismatch = false;
                    return StepResult::NextScreen;
                }
                self.confirm_mismatch = true;
                self.confirm_password.zeroize();
                self.confirm_cursor = 0;
            }
            KeyCode::Esc => {
                self.confirm_password.zeroize();
                self.confirm_cursor = 0;
                self.confirm_mismatch = false;
                self.screen = Screen::NameInput;
                self.active_field = FIELD_PASSWORD;
                self.cursor_pos = self.password.chars().count();
            }
            KeyCode::Char(c) => {
                self.confirm_mismatch = false;
                let byte_pos = confirm_byte_pos(&self.confirm_password, self.confirm_cursor);
                self.confirm_password.insert(byte_pos, c);
                self.confirm_cursor += 1;
            }
            KeyCode::Backspace => {
                self.confirm_mismatch = false;
                if self.confirm_cursor > 0 {
                    self.confirm_cursor -= 1;
                    let byte_pos = confirm_byte_pos(&self.confirm_password, self.confirm_cursor);
                    self.confirm_password.remove(byte_pos);
                }
            }
            KeyCode::Delete => {
                self.confirm_mismatch = false;
                let char_len = self.confirm_password.chars().count();
                if self.confirm_cursor < char_len {
                    let byte_pos = confirm_byte_pos(&self.confirm_password, self.confirm_cursor);
                    self.confirm_password.remove(byte_pos);
                }
            }
            KeyCode::Left => {
                self.confirm_cursor = self.confirm_cursor.saturating_sub(1);
            }
            KeyCode::Right => {
                let len = self.confirm_password.chars().count();
                self.confirm_cursor = (self.confirm_cursor + 1).min(len);
            }
            KeyCode::Home => self.confirm_cursor = 0,
            KeyCode::End => self.confirm_cursor = self.confirm_password.chars().count(),
            _ => {}
        }
        StepResult::Continue
    }

    fn handle_action_key(&mut self, code: KeyCode) -> StepResult {
        match code {
            KeyCode::Up => {
                self.action_selection = self.action_selection.saturating_sub(1);
            }
            KeyCode::Down => {
                self.action_selection = (self.action_selection + 1).min(ACTION_ITEMS.len() - 1);
            }
            KeyCode::Enter => return StepResult::Finish,
            KeyCode::Esc => return StepResult::Skip,
            _ => {}
        }
        StepResult::Continue
    }
}

/// Convert a char-index cursor position to a byte offset.
fn confirm_byte_pos(s: &str, cursor: usize) -> usize {
    s.char_indices()
        .nth(cursor)
        .map(|(i, _)| i)
        .unwrap_or(s.len())
}

/// Build a display string with a block cursor inserted at `cursor_pos`.
fn insert_cursor(value: &str, cursor_pos: usize, masked: bool) -> String {
    let mut display = if masked {
        "\u{25cf}".repeat(value.chars().count())
    } else {
        value.to_string()
    };
    let byte_pos = display
        .char_indices()
        .nth(cursor_pos)
        .map(|(i, _)| i)
        .unwrap_or(display.len());
    display.insert(byte_pos, '\u{2588}');
    display
}

impl Drop for Onboarding {
    fn drop(&mut self) {
        self.password.zeroize();
        self.confirm_password.zeroize();
    }
}

/// Run the onboarding TUI. Returns Some(OnboardingResult) if completed, None if skipped.
pub fn run() -> Result<Option<OnboardingResult>> {
    let mut onboarding = Onboarding::new();
    let mut terminal = ratatui::init();

    let result: Result<Option<OnboardingResult>> = loop {
        if let Err(e) = terminal.draw(|frame| onboarding.draw(frame)) {
            break Err(e.into());
        }

        if event::poll(Duration::from_millis(50))? {
            match event::read() {
                Err(e) => break Err(e.into()),
                Ok(Event::Key(key)) => {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }
                    if key.modifiers.contains(KeyModifiers::CONTROL)
                        && key.code == KeyCode::Char('c')
                    {
                        break Ok(None);
                    }

                    // Skip intro animation on any keypress
                    if !onboarding.intro_done {
                        onboarding.intro_done = true;
                        continue;
                    }

                    let step = match onboarding.screen {
                        Screen::NameInput => onboarding.handle_name_key(key.code),
                        Screen::ConfirmPassword => onboarding.handle_confirm_key(key.code),
                        Screen::ActionPicker => onboarding.handle_action_key(key.code),
                    };

                    match step {
                        StepResult::Continue => {}
                        StepResult::NextScreen => match onboarding.screen {
                            Screen::NameInput => {
                                if onboarding.password.trim().is_empty() {
                                    onboarding.screen = Screen::ActionPicker;
                                } else {
                                    onboarding.confirm_password.zeroize();
                                    onboarding.confirm_cursor = 0;
                                    onboarding.confirm_mismatch = false;
                                    onboarding.screen = Screen::ConfirmPassword;
                                }
                            }
                            Screen::ConfirmPassword => {
                                onboarding.screen = Screen::ActionPicker;
                            }
                            Screen::ActionPicker => {}
                        }
                        StepResult::Finish => {
                            let action = match onboarding.action_selection {
                                0 => PostSetupAction::Demo,
                                1 => PostSetupAction::StartFresh,
                                _ => PostSetupAction::Import,
                            };
                            let pw = onboarding.password.trim().to_string();
                            break Ok(Some(OnboardingResult {
                                user_name: onboarding.user_name.trim().to_string(),
                                company_name: onboarding.company_name.trim().to_string(),
                                password: if pw.is_empty() { None } else { Some(pw) },
                                action,
                            }));
                        }
                        StepResult::Skip => break Ok(None),
                    }
                }
                _ => {}
            }
        }

        onboarding.tick();
    };

    drop(terminal);
    ratatui::restore();
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_onboarding() -> Onboarding {
        Onboarding {
            user_name: String::new(),
            company_name: String::new(),
            password: String::new(),
            confirm_password: String::new(),
            confirm_cursor: 0,
            confirm_mismatch: false,
            active_field: 0,
            cursor_pos: 0,
            action_selection: 0,
            screen: Screen::NameInput,
            phase: 0.0,
            particles: vec![],
            width: 80,
            height: 24,
            start: Instant::now(),
            reveal_order: vec![],
            intro_done: true,
        }
    }

    #[test]
    fn field_navigation_cycles_through_all_fields() {
        let mut ob = make_onboarding();
        assert_eq!(ob.active_field, FIELD_NAME);
        ob.handle_name_key(KeyCode::Enter);
        assert_eq!(ob.active_field, FIELD_COMPANY);
        ob.handle_name_key(KeyCode::Enter);
        assert_eq!(ob.active_field, FIELD_PASSWORD);
        ob.handle_name_key(KeyCode::Enter);
        assert_eq!(ob.active_field, FIELD_BUTTON);
    }

    #[test]
    fn up_from_button_goes_to_password() {
        let mut ob = make_onboarding();
        ob.active_field = FIELD_BUTTON;
        ob.handle_name_key(KeyCode::Up);
        assert_eq!(ob.active_field, FIELD_PASSWORD);
    }

    #[test]
    fn active_value_returns_correct_field() {
        let mut ob = make_onboarding();
        ob.user_name = "Alice".into();
        ob.company_name = "Acme".into();
        ob.password = "secret".into();

        ob.active_field = FIELD_NAME;
        assert_eq!(ob.active_value(), "Alice");
        ob.active_field = FIELD_COMPANY;
        assert_eq!(ob.active_value(), "Acme");
        ob.active_field = FIELD_PASSWORD;
        assert_eq!(ob.active_value(), "secret");
    }

    #[test]
    fn typing_into_password_field() {
        let mut ob = make_onboarding();
        ob.active_field = FIELD_PASSWORD;
        ob.handle_name_key(KeyCode::Char('a'));
        ob.handle_name_key(KeyCode::Char('b'));
        ob.handle_name_key(KeyCode::Char('c'));
        assert_eq!(ob.password, "abc");
    }

    /// Build an OnboardingResult from the struct, mirroring the run() finish path.
    fn finish_result(ob: &Onboarding) -> OnboardingResult {
        let action = match ob.action_selection {
            0 => PostSetupAction::Demo,
            1 => PostSetupAction::StartFresh,
            _ => PostSetupAction::Import,
        };
        let pw = ob.password.trim().to_string();
        OnboardingResult {
            user_name: ob.user_name.trim().to_string(),
            company_name: ob.company_name.trim().to_string(),
            password: if pw.is_empty() { None } else { Some(pw) },
            action,
        }
    }

    #[test]
    fn finish_with_empty_password_yields_none() {
        let ob = make_onboarding();
        let result = finish_result(&ob);
        assert!(result.password.is_none());
    }

    #[test]
    fn finish_with_password_yields_trimmed_value() {
        let mut ob = make_onboarding();
        ob.active_field = FIELD_PASSWORD;
        ob.handle_name_key(KeyCode::Char('s'));
        ob.handle_name_key(KeyCode::Char('e'));
        ob.handle_name_key(KeyCode::Char('c'));
        let result = finish_result(&ob);
        assert_eq!(result.password, Some("sec".to_string()));
    }

    #[test]
    fn button_enter_advances_to_next_screen() {
        let mut ob = make_onboarding();
        ob.active_field = FIELD_BUTTON;
        let result = ob.handle_name_key(KeyCode::Enter);
        assert!(matches!(result, StepResult::NextScreen));
    }

    #[test]
    fn down_navigation_through_all_text_fields() {
        let mut ob = make_onboarding();
        ob.handle_name_key(KeyCode::Down);
        assert_eq!(ob.active_field, 1);
        ob.handle_name_key(KeyCode::Down);
        assert_eq!(ob.active_field, 2);
        ob.handle_name_key(KeyCode::Down);
        assert_eq!(ob.active_field, 3);
    }

    #[test]
    fn up_navigation_through_all_text_fields() {
        let mut ob = make_onboarding();
        ob.active_field = FIELD_PASSWORD;
        ob.cursor_pos = 0;
        ob.handle_name_key(KeyCode::Up);
        assert_eq!(ob.active_field, 1);
        ob.handle_name_key(KeyCode::Up);
        assert_eq!(ob.active_field, 0);
        // Up from 0 stays at 0
        ob.handle_name_key(KeyCode::Up);
        assert_eq!(ob.active_field, 0);
    }

    #[test]
    fn confirm_match_advances_screen() {
        let mut ob = make_onboarding();
        ob.password = "abc".into();
        ob.screen = Screen::ConfirmPassword;
        ob.handle_confirm_key(KeyCode::Char('a'));
        ob.handle_confirm_key(KeyCode::Char('b'));
        ob.handle_confirm_key(KeyCode::Char('c'));
        let result = ob.handle_confirm_key(KeyCode::Enter);
        assert!(matches!(result, StepResult::NextScreen));
        assert!(!ob.confirm_mismatch);
    }

    #[test]
    fn confirm_mismatch_shows_error() {
        let mut ob = make_onboarding();
        ob.password = "abc".into();
        ob.screen = Screen::ConfirmPassword;
        ob.handle_confirm_key(KeyCode::Char('x'));
        let result = ob.handle_confirm_key(KeyCode::Enter);
        assert!(matches!(result, StepResult::Continue));
        assert!(ob.confirm_mismatch);
        assert!(ob.confirm_password.is_empty()); // cleared on mismatch
    }

    #[test]
    fn confirm_esc_returns_to_password_field() {
        let mut ob = make_onboarding();
        ob.password = "abc".into();
        ob.screen = Screen::ConfirmPassword;
        ob.handle_confirm_key(KeyCode::Char('x'));
        ob.handle_confirm_key(KeyCode::Esc);
        assert!(matches!(ob.screen, Screen::NameInput));
        assert_eq!(ob.active_field, FIELD_PASSWORD);
        assert!(ob.confirm_password.is_empty());
        assert!(!ob.confirm_mismatch);
    }

    #[test]
    fn confirm_typing_and_backspace() {
        let mut ob = make_onboarding();
        ob.screen = Screen::ConfirmPassword;
        ob.handle_confirm_key(KeyCode::Char('a'));
        ob.handle_confirm_key(KeyCode::Char('b'));
        assert_eq!(ob.confirm_password, "ab");
        ob.handle_confirm_key(KeyCode::Backspace);
        assert_eq!(ob.confirm_password, "a");
        assert_eq!(ob.confirm_cursor, 1);
    }

    #[test]
    fn confirm_delete_key_removes_char_at_cursor() {
        let mut ob = make_onboarding();
        ob.screen = Screen::ConfirmPassword;
        ob.handle_confirm_key(KeyCode::Char('a'));
        ob.handle_confirm_key(KeyCode::Char('b'));
        ob.handle_confirm_key(KeyCode::Char('c'));
        ob.handle_confirm_key(KeyCode::Home);
        ob.handle_confirm_key(KeyCode::Delete);
        assert_eq!(ob.confirm_password, "bc");
        assert_eq!(ob.confirm_cursor, 0);
    }

    #[test]
    fn confirm_mismatch_clears_on_next_keystroke() {
        let mut ob = make_onboarding();
        ob.password = "abc".into();
        ob.screen = Screen::ConfirmPassword;
        ob.handle_confirm_key(KeyCode::Char('x'));
        ob.handle_confirm_key(KeyCode::Enter);
        assert!(ob.confirm_mismatch);
        ob.handle_confirm_key(KeyCode::Char('a'));
        assert!(!ob.confirm_mismatch);
    }

    #[test]
    fn confirm_trims_before_comparison() {
        let mut ob = make_onboarding();
        ob.password = "abc ".into();
        ob.screen = Screen::ConfirmPassword;
        // Type "abc" without trailing space — should still match after trim
        ob.handle_confirm_key(KeyCode::Char('a'));
        ob.handle_confirm_key(KeyCode::Char('b'));
        ob.handle_confirm_key(KeyCode::Char('c'));
        let result = ob.handle_confirm_key(KeyCode::Enter);
        assert!(matches!(result, StepResult::NextScreen));
    }
}
