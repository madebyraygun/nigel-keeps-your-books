use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyEventKind};
use ratatui::{
    layout::{Alignment, Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::effects::{self, Particle, LOGO};
use crate::error::Result;

const GOODBYE_DURATION: Duration = Duration::from_millis(1200);
const TICK_INTERVAL: Duration = Duration::from_millis(50);
const HOLD_MS: f64 = 400.0;
const UNREVEAL_MS: f64 = 800.0;

struct Goodbye {
    phase: f64,
    particles: Vec<Particle>,
    width: u16,
    height: u16,
    start: Instant,
    reveal_order: Vec<(usize, usize)>,
}

impl Goodbye {
    fn new() -> Self {
        let (width, height) = crossterm::terminal::size().unwrap_or((80, 24));
        Self {
            phase: 0.0,
            particles: effects::pre_seed_particles(width, height),
            width,
            height,
            start: Instant::now(),
            reveal_order: effects::logo_reveal_order(),
        }
    }

    fn is_expired(&self) -> bool {
        self.start.elapsed() >= GOODBYE_DURATION
    }

    fn tick(&mut self) {
        self.phase += 1.0 / 70.0;
        effects::tick_particles(&mut self.particles, self.width, self.height);
    }

    fn chars_visible_at(&self, elapsed_ms: f64) -> usize {
        let total_chars = self.reveal_order.len();
        if elapsed_ms < HOLD_MS {
            total_chars
        } else {
            let progress = ((elapsed_ms - HOLD_MS) / UNREVEAL_MS).min(1.0);
            ((1.0 - progress) * total_chars as f64) as usize
        }
    }

    fn opacity_at(elapsed_ms: f64) -> f64 {
        if elapsed_ms < HOLD_MS {
            1.0
        } else {
            let progress = ((elapsed_ms - HOLD_MS) / UNREVEAL_MS).min(1.0);
            1.0 - progress
        }
    }

    fn draw(&mut self, frame: &mut Frame) {
        let area = frame.area();
        self.width = area.width;
        self.height = area.height;

        let elapsed_ms = self.start.elapsed().as_secs_f64() * 1000.0;

        effects::render_particles(&self.particles, frame, area);

        let logo_height = LOGO.len() as u16;
        let goodbye_line_height = 2; // blank line + "Goodbye!"
        let total_height = logo_height + goodbye_line_height;
        let [_top, content_area, _bottom] = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(total_height),
            Constraint::Fill(1),
        ])
        .areas(area);

        let [logo_area, goodbye_area] = Layout::vertical([
            Constraint::Length(logo_height),
            Constraint::Length(goodbye_line_height),
        ])
        .areas(content_area);

        let chars_visible = self.chars_visible_at(elapsed_ms);

        effects::render_logo_reveal(
            self.phase,
            frame,
            logo_area,
            Some((&self.reveal_order, chars_visible)),
        );

        let goodbye_opacity = Self::opacity_at(elapsed_ms);

        let base_color = effects::gradient_color(self.phase);
        let faded_color = if let Color::Rgb(r, g, b) = base_color {
            Color::Rgb(
                (r as f64 * goodbye_opacity) as u8,
                (g as f64 * goodbye_opacity) as u8,
                (b as f64 * goodbye_opacity) as u8,
            )
        } else {
            base_color
        };

        let goodbye_text = Line::from(Span::styled(
            "Goodbye!",
            Style::default()
                .fg(faded_color)
                .add_modifier(Modifier::BOLD),
        ));
        frame.render_widget(
            Paragraph::new(vec![Line::raw(""), goodbye_text]).alignment(Alignment::Center),
            goodbye_area,
        );
    }
}

/// Run the goodbye screen. Blocks for up to 1.2 seconds; any keypress dismisses early.
pub fn run() -> Result<()> {
    let mut goodbye = Goodbye::new();
    let mut terminal = ratatui::init();

    let result: Result<()> = loop {
        if let Err(e) = terminal.draw(|frame| goodbye.draw(frame)) {
            break Err(e.into());
        }

        if goodbye.is_expired() {
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

        goodbye.tick();
    };

    ratatui::restore();
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn goodbye_starts_not_expired() {
        let goodbye = Goodbye::new();
        assert!(!goodbye.is_expired());
    }

    #[test]
    fn goodbye_has_pre_seeded_particles() {
        let goodbye = Goodbye::new();
        assert_eq!(goodbye.particles.len(), effects::MAX_PARTICLES * 2);
    }

    #[test]
    fn goodbye_tick_advances_phase() {
        let mut goodbye = Goodbye::new();
        let phase_before = goodbye.phase;
        goodbye.tick();
        assert!(goodbye.phase > phase_before);
    }

    #[test]
    fn goodbye_duration_is_1200ms() {
        assert_eq!(GOODBYE_DURATION, Duration::from_millis(1200));
    }

    #[test]
    fn reveal_order_contains_all_logo_chars() {
        let goodbye = Goodbye::new();
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
        assert_eq!(goodbye.reveal_order.len(), expected);
    }

    #[test]
    fn chars_visible_starts_at_full() {
        let goodbye = Goodbye::new();
        assert_eq!(goodbye.chars_visible_at(0.0), goodbye.reveal_order.len());
    }

    #[test]
    fn chars_visible_decreases_after_hold() {
        let goodbye = Goodbye::new();
        let total = goodbye.reveal_order.len();
        let visible = goodbye.chars_visible_at(HOLD_MS + UNREVEAL_MS / 2.0);
        assert!(visible < total);
        assert!(visible > 0);
    }

    #[test]
    fn chars_visible_reaches_zero_at_end() {
        let goodbye = Goodbye::new();
        assert_eq!(goodbye.chars_visible_at(HOLD_MS + UNREVEAL_MS), 0);
    }

    #[test]
    fn opacity_starts_at_full() {
        assert_eq!(Goodbye::opacity_at(0.0), 1.0);
    }

    #[test]
    fn opacity_fades_during_unreveal() {
        let opacity = Goodbye::opacity_at(HOLD_MS + UNREVEAL_MS / 2.0);
        assert!(opacity > 0.0 && opacity < 1.0);
    }

    #[test]
    fn opacity_reaches_zero_at_end() {
        assert_eq!(Goodbye::opacity_at(HOLD_MS + UNREVEAL_MS), 0.0);
    }
}
