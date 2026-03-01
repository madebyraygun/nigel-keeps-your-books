use rand::Rng;
use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

/// Pastel rainbow gradient: pink -> peach -> yellow -> mint -> cyan -> lavender -> magenta -> pink
pub const GRADIENT: &[(f64, f64, f64)] = &[
    (255.0, 179.0, 186.0), // #ffb3ba soft pink
    (255.0, 200.0, 162.0), // #ffc8a2 peach
    (255.0, 224.0, 163.0), // #ffe0a3 pastel yellow
    (201.0, 255.0, 203.0), // #c9ffcb mint
    (186.0, 225.0, 255.0), // #bae1ff pastel cyan
    (196.0, 183.0, 255.0), // #c4b7ff lavender
    (255.0, 179.0, 222.0), // #ffb3de soft magenta
    (255.0, 179.0, 186.0), // #ffb3ba wrap back to pink
];

pub const MAX_PARTICLES: usize = 20;
pub const PARTICLE_CHARS: &[char] = &['·', '∘', '•', '◦'];

/// Interpolate along the gradient for a position in 0.0..1.0
pub fn gradient_color(t: f64) -> Color {
    let t = t.rem_euclid(1.0);
    let segments = (GRADIENT.len() - 1) as f64;
    let scaled = t * segments;
    let idx = (scaled as usize).min(GRADIENT.len() - 2);
    let frac = scaled - idx as f64;

    let (r1, g1, b1) = GRADIENT[idx];
    let (r2, g2, b2) = GRADIENT[idx + 1];

    let r = (r1 + (r2 - r1) * frac) as u8;
    let g = (g1 + (g2 - g1) * frac) as u8;
    let b = (b1 + (b2 - b1) * frac) as u8;

    Color::Rgb(r, g, b)
}

pub struct Particle {
    pub x: f64,
    pub y: f64,
    pub speed: f64,
    pub drift: f64,
    pub brightness: f64,
    pub char_idx: usize,
    pub color_idx: usize,
}

impl Particle {
    /// Spawn a new particle at a random viewport position, fading in from invisible.
    pub fn new(width: u16, height: u16) -> Self {
        let mut rng = rand::thread_rng();
        Self {
            x: rng.gen_range(0.0..width as f64),
            y: rng.gen_range(0.0..height as f64),
            speed: rng.gen_range(0.15..0.45),
            drift: rng.gen_range(-0.1..0.1),
            brightness: 0.0,
            char_idx: rng.gen_range(0..PARTICLE_CHARS.len()),
            color_idx: rng.gen_range(0..GRADIENT.len() - 1),
        }
    }

    /// Spawn a particle at a random position already within the viewport (for pre-seeding).
    pub fn seeded(width: u16, height: u16) -> Self {
        let mut rng = rand::thread_rng();
        Self {
            x: rng.gen_range(0.0..width as f64),
            y: rng.gen_range(0.0..height as f64),
            speed: rng.gen_range(0.15..0.45),
            drift: rng.gen_range(-0.1..0.1),
            brightness: rng.gen_range(0.2..0.6),
            char_idx: rng.gen_range(0..PARTICLE_CHARS.len()),
            color_idx: rng.gen_range(0..GRADIENT.len() - 1),
        }
    }

    pub fn tick(&mut self) {
        self.y -= self.speed;
        self.x += self.drift;
        if self.y > 0.0 {
            self.brightness = (self.brightness + 0.08).min(0.6);
        }
    }

    pub fn is_dead(&self) -> bool {
        self.y < -1.0
    }
}

/// Pre-seed particles across the viewport plus a buffer zone below to prevent
/// a visible gap when the initial batch drifts off the top.
pub fn pre_seed_particles(width: u16, height: u16) -> Vec<Particle> {
    let mut rng = rand::thread_rng();
    let mut particles: Vec<Particle> = (0..MAX_PARTICLES)
        .map(|_| Particle::seeded(width, height))
        .collect();
    // Queue up another batch below the viewport — they'll arrive as the visible ones exit
    for _ in 0..MAX_PARTICLES {
        particles.push(Particle {
            x: rng.gen_range(0.0..width as f64),
            y: height as f64 + rng.gen_range(0.0..height as f64),
            speed: rng.gen_range(0.15..0.45),
            drift: rng.gen_range(-0.1..0.1),
            brightness: 0.0,
            char_idx: rng.gen_range(0..PARTICLE_CHARS.len()),
            color_idx: rng.gen_range(0..GRADIENT.len() - 1),
        });
    }
    particles
}

/// Standard per-tick particle update: advance existing, cull dead, spawn replacements.
pub fn tick_particles(particles: &mut Vec<Particle>, width: u16, height: u16) {
    for p in particles.iter_mut() {
        p.tick();
    }
    particles.retain(|p| !p.is_dead());
    // Spawn enough to maintain steady density — up to 2 per tick to avoid gaps
    let mut spawned = 0;
    while particles.len() < MAX_PARTICLES && spawned < 2 {
        particles.push(Particle::new(width, height));
        spawned += 1;
    }
}

/// Render particles onto a frame region.
pub fn render_particles(particles: &[Particle], frame: &mut Frame, area: Rect) {
    for p in particles {
        let px = p.x.round() as u16;
        let py = p.y.round() as u16;
        if px < area.width && py < area.height {
            let (r, g, b) = GRADIENT[p.color_idx];
            let alpha = p.brightness;
            let particle_area = Rect::new(area.x + px, area.y + py, 1, 1);
            frame.render_widget(
                Paragraph::new(Span::styled(
                    PARTICLE_CHARS[p.char_idx].to_string(),
                    Style::default().fg(Color::Rgb(
                        (r * alpha) as u8,
                        (g * alpha) as u8,
                        (b * alpha) as u8,
                    )),
                )),
                particle_area,
            );
        }
    }
}

pub const LOGO: &[&str] = &[
    r"  /$$   /$$ /$$                     /$$",
    r" | $$$ | $$|__/                    | $$",
    r" | $$$$| $$ /$$  /$$$$$$   /$$$$$$ | $$",
    r" | $$ $$ $$| $$ /$$__  $$ /$$__  $$| $$",
    r" | $$  $$$$| $$| $$  \ $$| $$$$$$$$| $$",
    r" | $$\  $$$| $$| $$  | $$| $$_____/| $$",
    r" | $$ \  $$| $$|  $$$$$$$|  $$$$$$$| $$",
    r" |__/  \__/|__/ \____  $$ \_______/|__/",
    r"                /$$  \ $$",
    r"               |  $$$$$$/",
    r"                \______/",
];

/// Render the Nigel ASCII logo with animated rainbow gradient.
pub fn render_logo(phase: f64, frame: &mut Frame, logo_area: Rect) {
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
                        let t =
                            (col as f64 / gradient_width) + (row as f64 * 0.04) - phase;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gradient_color_at_zero() {
        let color = gradient_color(0.0);
        assert_eq!(color, Color::Rgb(255, 179, 186));
    }

    #[test]
    fn gradient_color_at_one_wraps() {
        // t=1.0 wraps to 0.0
        let color = gradient_color(1.0);
        assert_eq!(color, Color::Rgb(255, 179, 186));
    }

    #[test]
    fn gradient_color_negative_wraps() {
        let color = gradient_color(-1.0);
        assert_eq!(color, Color::Rgb(255, 179, 186));
    }

    #[test]
    fn gradient_color_midpoint_returns_rgb() {
        let color = gradient_color(0.5);
        assert!(matches!(color, Color::Rgb(_, _, _)));
    }

    #[test]
    fn particle_new_starts_within_viewport() {
        let p = Particle::new(80, 24);
        assert!(p.x >= 0.0 && p.x < 80.0);
        assert!(p.y >= 0.0 && p.y < 24.0);
        assert_eq!(p.brightness, 0.0);
    }

    #[test]
    fn particle_seeded_within_viewport() {
        let p = Particle::seeded(80, 24);
        assert!(p.x >= 0.0 && p.x < 80.0);
        assert!(p.y >= 0.0 && p.y < 24.0);
        assert!(p.brightness >= 0.2);
    }

    #[test]
    fn particle_tick_moves_up() {
        let mut p = Particle::new(80, 24);
        let y_before = p.y;
        p.tick();
        assert!(p.y < y_before);
    }

    #[test]
    fn particle_dies_above_screen() {
        let mut p = Particle::new(80, 24);
        p.y = -0.5;
        assert!(!p.is_dead());
        p.y = -1.5;
        assert!(p.is_dead());
    }

    #[test]
    fn pre_seed_creates_visible_and_queued_particles() {
        let particles = pre_seed_particles(80, 24);
        assert_eq!(particles.len(), MAX_PARTICLES * 2);
        let visible = particles.iter().filter(|p| p.y < 24.0).count();
        let queued = particles.iter().filter(|p| p.y >= 24.0).count();
        assert_eq!(visible, MAX_PARTICLES);
        assert_eq!(queued, MAX_PARTICLES);
    }

    #[test]
    fn tick_particles_culls_dead() {
        let mut particles = vec![Particle::new(80, 24)];
        particles[0].y = -2.0; // force dead
        tick_particles(&mut particles, 80, 24);
        // Dead particle removed, replacements spawned (up to 2 per tick)
        for p in &particles {
            assert!(!p.is_dead());
        }
        assert!(particles.len() <= MAX_PARTICLES);
    }
}
