use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyEventKind};
use rand::seq::SliceRandom;
use ratatui::{
    layout::{Alignment, Constraint, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::effects::{self, gradient_color, Particle, LOGO};
use crate::error::Result;

const SPLASH_DURATION: Duration = Duration::from_millis(1500);
const TICK_INTERVAL: Duration = Duration::from_millis(50);
const REVEAL_MS: f64 = 500.0;
const DISSOLVE_MS: f64 = 400.0;

struct Splash {
    phase: f64,
    particles: Vec<Particle>,
    width: u16,
    height: u16,
    start: Instant,
    /// Logo character positions in randomized reveal order
    reveal_order: Vec<(usize, usize)>,
}

impl Splash {
    fn new() -> Self {
        let (width, height) = crossterm::terminal::size().unwrap_or((80, 24));
        let max_logo_width = LOGO.iter().map(|l| l.len()).max().unwrap_or(0);

        // Build a list of all non-space character positions, then shuffle
        let mut positions: Vec<(usize, usize)> = Vec::new();
        for (row, line) in LOGO.iter().enumerate() {
            let padded = format!("{:<width$}", line, width = max_logo_width);
            for (col, ch) in padded.chars().enumerate() {
                if ch != ' ' {
                    positions.push((row, col));
                }
            }
        }
        let mut rng = rand::thread_rng();
        positions.shuffle(&mut rng);

        Self {
            phase: 0.0,
            particles: effects::pre_seed_particles(width, height),
            width,
            height,
            start: Instant::now(),
            reveal_order: positions,
        }
    }

    fn is_expired(&self) -> bool {
        self.start.elapsed() >= SPLASH_DURATION
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
        let total_ms = SPLASH_DURATION.as_secs_f64() * 1000.0;
        let remaining_ms = total_ms - elapsed_ms;

        effects::render_particles(&self.particles, frame, area);

        let logo_height = LOGO.len() as u16;
        let [_top, logo_area, _bottom] = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(logo_height),
            Constraint::Fill(1),
        ])
        .areas(area);

        // Determine which characters are visible based on reveal/dissolve progress
        let total_chars = self.reveal_order.len();
        let chars_visible = if elapsed_ms < REVEAL_MS {
            // Reveal phase: progressively show characters
            let progress = elapsed_ms / REVEAL_MS;
            (progress * total_chars as f64) as usize
        } else if remaining_ms < DISSOLVE_MS {
            // Dissolve phase: progressively hide characters (reverse order)
            let progress = (remaining_ms / DISSOLVE_MS).max(0.0);
            (progress * total_chars as f64) as usize
        } else {
            total_chars
        };

        // Build set of visible positions
        let visible: std::collections::HashSet<(usize, usize)> =
            self.reveal_order[..chars_visible.min(total_chars)]
                .iter()
                .copied()
                .collect();

        let max_logo_width = LOGO.iter().map(|l| l.len()).max().unwrap_or(0);
        let gradient_width = 40.0;
        let logo_lines: Vec<Line> = LOGO
            .iter()
            .enumerate()
            .map(|(row, line)| {
                let padded = format!("{:<width$}", line, width = max_logo_width);
                let spans: Vec<Span> = padded
                    .chars()
                    .enumerate()
                    .map(|(col, ch)| {
                        if ch == ' ' || !visible.contains(&(row, col)) {
                            Span::raw(" ")
                        } else {
                            let t = (col as f64 / gradient_width)
                                + (row as f64 * 0.04)
                                - self.phase;
                            Span::styled(
                                ch.to_string(),
                                Style::default()
                                    .fg(gradient_color(t))
                                    .add_modifier(Modifier::BOLD),
                            )
                        }
                    })
                    .collect();
                Line::from(spans)
            })
            .collect();
        frame.render_widget(
            Paragraph::new(logo_lines).alignment(Alignment::Center),
            logo_area,
        );
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
        let max_logo_width = LOGO.iter().map(|l| l.len()).max().unwrap_or(0);
        let expected: usize = LOGO
            .iter()
            .map(|line| {
                format!("{:<width$}", line, width = max_logo_width)
                    .chars()
                    .filter(|c| *c != ' ')
                    .count()
            })
            .sum();
        assert_eq!(splash.reveal_order.len(), expected);
    }
}
