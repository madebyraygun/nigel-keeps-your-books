use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

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

enum Screen {
    NameInput,
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
    pub action: PostSetupAction,
}

struct Onboarding {
    user_name: String,
    company_name: String,
    active_field: usize,
    cursor_pos: usize,
    action_selection: usize,
    screen: Screen,
    phase: f64,
    particles: Vec<Particle>,
    width: u16,
    height: u16,
}

impl Onboarding {
    fn new() -> Self {
        let (width, height) = crossterm::terminal::size().unwrap_or((80, 24));
        Self {
            user_name: String::new(),
            company_name: String::new(),
            active_field: 0,
            cursor_pos: 0,
            action_selection: 0,
            screen: Screen::NameInput,
            phase: 0.0,
            particles: effects::pre_seed_particles(width, height),
            width,
            height,
        }
    }

    fn active_value(&self) -> &str {
        match self.active_field {
            0 => &self.user_name,
            _ => &self.company_name,
        }
    }

    fn active_value_mut(&mut self) -> &mut String {
        match self.active_field {
            0 => &mut self.user_name,
            _ => &mut self.company_name,
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

        match self.screen {
            Screen::NameInput => self.draw_name_input(frame, area),
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
                Constraint::Length(2),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Fill(1),
            ])
            .areas(area);

        effects::render_logo(self.phase, frame, logo_area);

        frame.render_widget(
            Paragraph::new(Span::styled("Welcome! Let's get you set up.", HEADER_STYLE))
                .alignment(ratatui::layout::Alignment::Center),
            welcome_area,
        );

        let form_width = 50u16.min(area.width.saturating_sub(4));
        let form_x = area.x + (area.width.saturating_sub(form_width)) / 2;
        let centered_form = Rect::new(form_x, form_area.y, form_width, form_area.height);

        let [name_row, biz_row] =
            Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).areas(centered_form);

        self.draw_field(frame, name_row, "Your name:", &self.user_name, 0);
        self.draw_field(frame, biz_row, "Business name:", &self.company_name, 1);

        // Continue button
        let btn_style = if self.active_field == 2 {
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

    /// Convert a char-index cursor position to a byte offset in the string.
    fn cursor_byte_pos(&self) -> usize {
        self.active_value()
            .char_indices()
            .nth(self.cursor_pos)
            .map(|(i, _)| i)
            .unwrap_or(self.active_value().len())
    }

    fn draw_field(&self, frame: &mut Frame, area: Rect, label: &str, value: &str, field_idx: usize) {
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
            let mut s = value.to_string();
            let byte_pos = s.char_indices()
                .nth(self.cursor_pos)
                .map(|(i, _)| i)
                .unwrap_or(s.len());
            s.insert(byte_pos, '█');
            s
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
        if field <= 1 {
            self.cursor_pos = self.active_value().chars().count();
        }
    }

    fn handle_name_key(&mut self, code: KeyCode) -> StepResult {
        // On the button (field 2), only handle navigation and submit
        if self.active_field == 2 {
            match code {
                KeyCode::Enter => return StepResult::NextScreen,
                KeyCode::Up => self.move_to_field(1),
                KeyCode::Esc => return StepResult::Skip,
                _ => {}
            }
            return StepResult::Continue;
        }

        // Text input fields (0 and 1)
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

                    let step = match onboarding.screen {
                        Screen::NameInput => onboarding.handle_name_key(key.code),
                        Screen::ActionPicker => onboarding.handle_action_key(key.code),
                    };

                    match step {
                        StepResult::Continue => {}
                        StepResult::NextScreen => {
                            onboarding.screen = Screen::ActionPicker;
                        }
                        StepResult::Finish => {
                            let action = match onboarding.action_selection {
                                0 => PostSetupAction::Demo,
                                1 => PostSetupAction::StartFresh,
                                _ => PostSetupAction::Import,
                            };
                            break Ok(Some(OnboardingResult {
                                user_name: onboarding.user_name.trim().to_string(),
                                company_name: onboarding.company_name.trim().to_string(),
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
