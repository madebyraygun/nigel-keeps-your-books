use crossterm::event::KeyCode;
use rand::Rng;
use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

const TICK_RATE: Duration = Duration::from_millis(150);

// Pastel rainbow gradient: pink → peach → yellow → mint → cyan → lavender → magenta → pink
const GRADIENT: &[(f64, f64, f64)] = &[
    (255.0, 179.0, 186.0),
    (255.0, 200.0, 162.0),
    (255.0, 224.0, 163.0),
    (201.0, 255.0, 203.0),
    (186.0, 225.0, 255.0),
    (196.0, 183.0, 255.0),
    (255.0, 179.0, 222.0),
    (255.0, 179.0, 186.0),
];

fn gradient_color(t: f64) -> Color {
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

const MAX_PARTICLES: usize = 20;
const PARTICLE_CHARS: &[char] = &['·', '∘', '•', '◦'];

struct Particle {
    x: f64,
    y: f64,
    speed: f64,
    drift: f64,
    brightness: f64,
    char_idx: usize,
    color_idx: usize,
}

impl Particle {
    fn new(width: u16, height: u16) -> Self {
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

    fn tick(&mut self) {
        self.y -= self.speed;
        self.x += self.drift;
        if self.y > 0.0 {
            self.brightness = (self.brightness + 0.08).min(0.6);
        }
    }

    fn is_dead(&self) -> bool {
        self.y < -1.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Direction {
    Up,
    Down,
    Left,
    Right,
}

impl Direction {
    fn opposite(self) -> Self {
        match self {
            Direction::Up => Direction::Down,
            Direction::Down => Direction::Up,
            Direction::Left => Direction::Right,
            Direction::Right => Direction::Left,
        }
    }
}

pub enum SnakeAction {
    Continue,
    Quit,
}

pub struct SnakeGame {
    pub body: VecDeque<(u16, u16)>,
    direction: Direction,
    next_direction: Direction,
    pub food: (u16, u16),
    pub food_value: f64,
    pub score: f64,
    pub game_over: bool,
    last_tick: Instant,
    pub board_width: u16,
    pub board_height: u16,
    rng: rand::rngs::ThreadRng,
    particles: Vec<Particle>,
    phase: f64,
}

impl SnakeGame {
    pub fn new() -> Self {
        let board_width: u16 = 40;
        let board_height: u16 = 20;
        let mut rng = rand::thread_rng();

        let cx = board_width / 2;
        let cy = board_height / 2;
        let mut body = VecDeque::new();
        body.push_back((cx, cy));
        body.push_back((cx.saturating_sub(1), cy));
        body.push_back((cx.saturating_sub(2), cy));

        let food = Self::random_food_pos(&body, board_width, board_height, &mut rng);
        let food_value = Self::random_food_value(&mut rng);

        Self {
            body,
            direction: Direction::Right,
            next_direction: Direction::Right,
            food,
            food_value,
            score: 0.0,
            game_over: false,
            last_tick: Instant::now(),
            board_width,
            board_height,
            rng,
            particles: Vec::new(),
            phase: 0.0,
        }
    }

    fn random_food_pos(
        body: &VecDeque<(u16, u16)>,
        w: u16,
        h: u16,
        rng: &mut rand::rngs::ThreadRng,
    ) -> (u16, u16) {
        let total = (w as usize) * (h as usize);
        if body.len() >= total {
            return (0, 0);
        }
        loop {
            let x = rng.gen_range(0..w);
            let y = rng.gen_range(0..h);
            if !body.contains(&(x, y)) {
                return (x, y);
            }
        }
    }

    fn random_food_value(rng: &mut rand::rngs::ThreadRng) -> f64 {
        let cents = rng.gen_range(100..=999);
        cents as f64 / 100.0
    }

    fn spawn_food(&mut self) {
        self.food =
            Self::random_food_pos(&self.body, self.board_width, self.board_height, &mut self.rng);
        self.food_value = Self::random_food_value(&mut self.rng);
    }

    fn tick(&mut self) {
        if self.game_over {
            return;
        }

        self.direction = self.next_direction;

        let (hx, hy) = self.body[0];
        let new_head = match self.direction {
            Direction::Up => (hx, hy.wrapping_sub(1)),
            Direction::Down => (hx, hy + 1),
            Direction::Left => (hx.wrapping_sub(1), hy),
            Direction::Right => (hx + 1, hy),
        };

        // Wall collision (wrapping underflow gives u16::MAX which is >= board size)
        if new_head.0 >= self.board_width || new_head.1 >= self.board_height {
            self.game_over = true;
            return;
        }

        // Self collision
        if self.body.contains(&new_head) {
            self.game_over = true;
            return;
        }

        self.body.push_front(new_head);

        if new_head == self.food {
            self.score += self.food_value;
            self.spawn_food();
        } else {
            self.body.pop_back();
        }
    }

    pub fn handle_key(&mut self, code: KeyCode) -> SnakeAction {
        match code {
            KeyCode::Esc => return SnakeAction::Quit,
            _ if self.game_over => {
                if let KeyCode::Char('r') | KeyCode::Char('R') = code {
                    *self = Self::new();
                }
            }
            KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right => {
                let new_dir = match code {
                    KeyCode::Up => Direction::Up,
                    KeyCode::Down => Direction::Down,
                    KeyCode::Left => Direction::Left,
                    _ => Direction::Right,
                };
                if self.direction != new_dir.opposite() {
                    self.next_direction = new_dir;
                }
            }
            _ => {}
        }
        SnakeAction::Continue
    }

    pub fn should_tick(&self) -> bool {
        self.last_tick.elapsed() >= TICK_RATE
    }

    pub fn do_tick(&mut self) {
        self.tick();
        self.last_tick = Instant::now();

        // Advance gradient phase and particles
        self.phase += 1.0 / 70.0;
        for p in &mut self.particles {
            p.tick();
        }
        self.particles.retain(|p| !p.is_dead());
        let mut rng = rand::thread_rng();
        if self.particles.len() < MAX_PARTICLES && rng.gen_range(0..3) == 0 {
            self.particles
                .push(Particle::new(self.board_width, self.board_height));
        }
    }

    pub fn tick_rate(&self) -> Duration {
        TICK_RATE.saturating_sub(self.last_tick.elapsed())
    }

    pub fn draw(&mut self, frame: &mut Frame) {
        let full = frame.area();

        // Board sizing: subtract 2 for border on each side
        let board_width = (full.width.saturating_sub(2)).min(80);
        let board_height = (full.height.saturating_sub(2)).min(40);

        // Center the play area when terminal is larger than needed
        let render_w = board_width + 2;
        let render_h = board_height + 2;
        let area = Rect::new(
            full.x + full.width.saturating_sub(render_w) / 2,
            full.y + full.height.saturating_sub(render_h) / 2,
            render_w.min(full.width),
            render_h.min(full.height),
        );

        // If board size changed, clamp snake and food to new bounds
        if board_width != self.board_width || board_height != self.board_height {
            self.board_width = board_width;
            self.board_height = board_height;

            // Retain only segments within bounds
            self.body.retain(|&(x, y)| x < board_width && y < board_height);
            // Ensure at least one segment
            if self.body.is_empty() {
                self.body
                    .push_back((board_width / 2, board_height / 2));
            }
            // Respawn food if out of bounds
            if self.food.0 >= board_width || self.food.1 >= board_height {
                self.spawn_food();
            }
        }

        // Title bar
        let score_str = format!(" $ Snake $ | Score: ${:.2} ", self.score);
        let footer_str = " Arrow keys: move | Esc: quit ";

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(gradient_color(self.phase)))
            .title(
                Line::from(Span::styled(
                    score_str,
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ))
                .alignment(Alignment::Center),
            )
            .title_bottom(
                Line::from(Span::styled(
                    footer_str,
                    Style::default().fg(Color::DarkGray),
                ))
                .alignment(Alignment::Center),
            );

        // Build game field with rainbow body and background particles
        let mut lines: Vec<Line> = Vec::with_capacity(board_height as usize);
        for y in 0..board_height {
            let mut spans: Vec<Span> = Vec::with_capacity(board_width as usize);
            for x in 0..board_width {
                let pos = (x, y);
                if pos == self.body[0] {
                    // Snake head — bright white to stand out
                    spans.push(Span::styled(
                        "\u{2588}",
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    ));
                } else if let Some(idx) = self.body.iter().position(|&p| p == pos) {
                    // Snake body — rainbow gradient trail
                    let color = gradient_color(self.phase + idx as f64 * 0.05);
                    spans.push(Span::styled(
                        "\u{2588}",
                        Style::default().fg(color),
                    ));
                } else if pos == self.food {
                    // Food
                    spans.push(Span::styled(
                        "$",
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    ));
                } else if let Some(p) = self.particles.iter().find(|p| {
                    p.x.round() as u16 == x && p.y.round() as u16 == y
                }) {
                    // Background particle
                    let (r, g, b) = GRADIENT[p.color_idx];
                    let a = p.brightness;
                    let ch = PARTICLE_CHARS[p.char_idx];
                    spans.push(Span::styled(
                        ch.to_string(),
                        Style::default().fg(Color::Rgb(
                            (r * a) as u8,
                            (g * a) as u8,
                            (b * a) as u8,
                        )),
                    ));
                } else {
                    spans.push(Span::raw(" "));
                }
            }
            lines.push(Line::from(spans));
        }

        let paragraph = Paragraph::new(lines).block(block);
        frame.render_widget(paragraph, area);

        // Game over overlay
        if self.game_over {
            let overlay_width: u16 = 34;
            let overlay_height: u16 = 5;
            let ox = area.x + area.width.saturating_sub(overlay_width) / 2;
            let oy = area.y + area.height.saturating_sub(overlay_height) / 2;
            let overlay_rect = Rect::new(ox, oy, overlay_width, overlay_height);

            let overlay_block = Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Red))
                .title(
                    Line::from(Span::styled(
                        " Game Over ",
                        Style::default()
                            .fg(Color::Red)
                            .add_modifier(Modifier::BOLD),
                    ))
                    .alignment(Alignment::Center),
                );

            let overlay_lines = vec![
                Line::from(Span::styled(
                    format!("Final Score: ${:.2}", self.score),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(vec![
                    Span::styled(
                        "[R]",
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" Restart  "),
                    Span::styled(
                        "[Esc]",
                        Style::default()
                            .fg(Color::Red)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" Quit"),
                ]),
            ];

            let overlay_paragraph = Paragraph::new(overlay_lines)
                .block(overlay_block)
                .alignment(Alignment::Center);
            frame.render_widget(overlay_paragraph, overlay_rect);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_game_starts_correctly() {
        let game = SnakeGame::new();
        assert_eq!(game.body.len(), 3);
        assert_eq!(game.score, 0.0);
        assert!(!game.game_over);
        assert_eq!(game.direction, Direction::Right);
    }

    #[test]
    fn snake_moves_right() {
        let mut game = SnakeGame::new();
        let head_before = game.body[0];
        game.tick();
        let head_after = game.body[0];
        assert_eq!(head_after.0, head_before.0 + 1);
        assert_eq!(head_after.1, head_before.1);
        assert_eq!(game.body.len(), 3); // no growth without food
    }

    #[test]
    fn snake_changes_direction() {
        let mut game = SnakeGame::new();
        game.handle_key(KeyCode::Down);
        game.tick();
        let head = game.body[0];
        // Should have moved down
        assert_eq!(head.1, game.body[1].1 + 1);
    }

    #[test]
    fn cannot_reverse_direction() {
        let mut game = SnakeGame::new();
        // Moving right, try to go left — should be ignored
        game.handle_key(KeyCode::Left);
        game.tick();
        let head = game.body[0];
        // Still moving right
        assert!(head.0 > game.body[1].0);
    }

    #[test]
    fn wall_collision_ends_game() {
        let mut game = SnakeGame::new();
        // Move right until wall
        for _ in 0..100 {
            if game.game_over {
                break;
            }
            game.tick();
        }
        assert!(game.game_over);
    }

    #[test]
    fn eating_food_grows_snake_and_scores() {
        let mut game = SnakeGame::new();
        // Place food directly ahead
        let head = game.body[0];
        game.food = (head.0 + 1, head.1);
        game.food_value = 5.0;
        let len_before = game.body.len();
        game.tick();
        assert_eq!(game.body.len(), len_before + 1);
        assert_eq!(game.score, 5.0);
    }

    #[test]
    fn food_value_in_range() {
        let mut rng = rand::thread_rng();
        for _ in 0..100 {
            let val = SnakeGame::random_food_value(&mut rng);
            assert!(val >= 1.0 && val <= 9.99);
        }
    }

    #[test]
    fn esc_returns_quit() {
        let mut game = SnakeGame::new();
        let action = game.handle_key(KeyCode::Esc);
        assert!(matches!(action, SnakeAction::Quit));
    }

    #[test]
    fn restart_resets_game() {
        let mut game = SnakeGame::new();
        game.game_over = true;
        game.score = 42.0;
        game.handle_key(KeyCode::Char('r'));
        assert!(!game.game_over);
        assert_eq!(game.score, 0.0);
        assert_eq!(game.body.len(), 3);
    }

    #[test]
    fn direction_opposite() {
        assert_eq!(Direction::Up.opposite(), Direction::Down);
        assert_eq!(Direction::Down.opposite(), Direction::Up);
        assert_eq!(Direction::Left.opposite(), Direction::Right);
        assert_eq!(Direction::Right.opposite(), Direction::Left);
    }
}
