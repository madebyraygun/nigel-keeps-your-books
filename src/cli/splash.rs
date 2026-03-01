use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyEventKind};
use ratatui::{
    layout::{Constraint, Layout},
    Frame,
};

use crate::effects::{self, Particle, LOGO};
use crate::error::Result;

const SPLASH_DURATION: Duration = Duration::from_millis(1500);
const TICK_INTERVAL: Duration = Duration::from_millis(50);

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

    fn tick(&mut self) {
        self.phase += 1.0 / 70.0;
        effects::tick_particles(&mut self.particles, self.width, self.height);
    }

    fn draw(&mut self, frame: &mut Frame) {
        let area = frame.area();
        self.width = area.width;
        self.height = area.height;

        effects::render_particles(&self.particles, frame, area);

        let logo_height = LOGO.len() as u16;
        let [_top, logo_area, _bottom] = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(logo_height),
            Constraint::Fill(1),
        ])
        .areas(area);

        effects::render_logo(self.phase, frame, logo_area);
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
        assert_eq!(splash.particles.len(), effects::MAX_PARTICLES);
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
