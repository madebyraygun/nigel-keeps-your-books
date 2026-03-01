use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyEventKind};
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::effects::{self, gradient_color, Particle, GRADIENT, LOGO, PARTICLE_CHARS};
use crate::error::Result;

const SPLASH_DURATION: Duration = Duration::from_millis(1500);
const TICK_INTERVAL: Duration = Duration::from_millis(50);
const FADE_IN_MS: f64 = 400.0;
const FADE_OUT_MS: f64 = 400.0;

struct Splash {
    phase: f64,
    particles: Vec<Particle>,
    width: u16,
    height: u16,
    start: Instant,
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
        }
    }

    fn is_expired(&self) -> bool {
        self.start.elapsed() >= SPLASH_DURATION
    }

    fn opacity(&self) -> f64 {
        let elapsed = self.start.elapsed().as_secs_f64() * 1000.0;
        let total = SPLASH_DURATION.as_secs_f64() * 1000.0;
        let remaining = total - elapsed;
        if elapsed < FADE_IN_MS {
            elapsed / FADE_IN_MS
        } else if remaining < FADE_OUT_MS {
            (remaining / FADE_OUT_MS).max(0.0)
        } else {
            1.0
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
        let opacity = self.opacity();

        // Render particles with fade
        for p in &self.particles {
            let px = p.x.round() as u16;
            let py = p.y.round() as u16;
            if px < area.width && py < area.height {
                let (r, g, b) = GRADIENT[p.color_idx];
                let a = p.brightness * opacity;
                let particle_area = Rect::new(area.x + px, area.y + py, 1, 1);
                frame.render_widget(
                    Paragraph::new(Span::styled(
                        PARTICLE_CHARS[p.char_idx].to_string(),
                        Style::default().fg(Color::Rgb(
                            (r * a) as u8,
                            (g * a) as u8,
                            (b * a) as u8,
                        )),
                    )),
                    particle_area,
                );
            }
        }

        // Render logo with fade
        let logo_height = LOGO.len() as u16;
        let [_top, logo_area, _bottom] = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(logo_height),
            Constraint::Fill(1),
        ])
        .areas(area);

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
                        if ch == ' ' {
                            Span::raw(" ")
                        } else {
                            let t = (col as f64 / gradient_width)
                                + (row as f64 * 0.04)
                                - self.phase;
                            let color = gradient_color(t);
                            let (r, g, b) = match color {
                                Color::Rgb(r, g, b) => (r, g, b),
                                _ => (255, 255, 255),
                            };
                            Span::styled(
                                ch.to_string(),
                                Style::default()
                                    .fg(Color::Rgb(
                                        (r as f64 * opacity) as u8,
                                        (g as f64 * opacity) as u8,
                                        (b as f64 * opacity) as u8,
                                    ))
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
}
