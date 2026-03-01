use rand::Rng;
use ratatui::style::Color;

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
pub const PARTICLE_CHARS: &[char] = &['\u{00b7}', '\u{2218}', '\u{2022}', '\u{25e6}'];

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
    /// Spawn a new particle below the visible area so it floats upward.
    pub fn new(width: u16, height: u16) -> Self {
        let mut rng = rand::thread_rng();
        Self {
            x: rng.gen_range(0.0..width as f64),
            y: height as f64 + rng.gen_range(0.0..5.0),
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

/// Pre-seed a full set of particles spread across the viewport.
pub fn pre_seed_particles(width: u16, height: u16) -> Vec<Particle> {
    (0..MAX_PARTICLES)
        .map(|_| Particle::seeded(width, height))
        .collect()
}

/// Standard per-tick particle update: advance existing, cull dead, maybe spawn new.
pub fn tick_particles(particles: &mut Vec<Particle>, width: u16, height: u16) {
    for p in particles.iter_mut() {
        p.tick();
    }
    particles.retain(|p| !p.is_dead());
    let mut rng = rand::thread_rng();
    if particles.len() < MAX_PARTICLES && rng.gen_range(0..3) == 0 {
        particles.push(Particle::new(width, height));
    }
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
    fn particle_new_starts_below_screen() {
        let p = Particle::new(80, 24);
        assert!(p.y >= 24.0);
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
    fn pre_seed_creates_max_particles() {
        let particles = pre_seed_particles(80, 24);
        assert_eq!(particles.len(), MAX_PARTICLES);
    }

    #[test]
    fn tick_particles_culls_dead() {
        let mut particles = vec![Particle::new(80, 24)];
        particles[0].y = -2.0; // force dead
        tick_particles(&mut particles, 80, 24);
        // Dead particle removed, maybe a new one spawned (0 or 1)
        assert!(particles.len() <= 1);
        for p in &particles {
            assert!(!p.is_dead());
        }
    }
}
