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

use crate::effects::{self, gradient_color, Particle, GRADIENT, PARTICLE_CHARS};

const BASE_TICK_MS: u64 = 150;
const MIN_TICK_MS: u64 = 50;

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
            particles: effects::pre_seed_particles(board_width, board_height),
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
        self.food = Self::random_food_pos(
            &self.body,
            self.board_width,
            self.board_height,
            &mut self.rng,
        );
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
            // Check if the snake fills the entire board (win condition)
            let total = self.board_width as usize * self.board_height as usize;
            if self.body.len() >= total {
                self.game_over = true;
                return;
            }
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

    pub fn do_tick(&mut self) {
        self.tick();
        self.last_tick = Instant::now();

        // Advance gradient phase and particles
        self.phase += 1.0 / 70.0;
        effects::tick_particles(&mut self.particles, self.board_width, self.board_height);
    }

    pub fn tick_rate(&self) -> Duration {
        let speed_ms = BASE_TICK_MS.saturating_sub((self.body.len() as u64).saturating_sub(3) * 2);
        let tick = Duration::from_millis(speed_ms.max(MIN_TICK_MS));
        tick.saturating_sub(self.last_tick.elapsed())
    }

    pub fn draw(&mut self, frame: &mut Frame) {
        let full = frame.area();

        // Board sizing: subtract 2 for border on each side
        // Each cell is 2 chars wide to compensate for terminal aspect ratio
        let board_width = ((full.width.saturating_sub(2)) / 2).min(40);
        let board_height = (full.height.saturating_sub(2)).min(40);

        // Center the play area when terminal is larger than needed
        // Each cell is 2 chars wide, plus 2 for the border
        let render_w = board_width * 2 + 2;
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
            self.body
                .retain(|&(x, y)| x < board_width && y < board_height);
            // Ensure at least one segment
            if self.body.is_empty() {
                self.body.push_back((board_width / 2, board_height / 2));
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
        // Each cell is 2 chars wide to make movement speed look uniform
        let mut lines: Vec<Line> = Vec::with_capacity(board_height as usize);
        for y in 0..board_height {
            let mut spans: Vec<Span> = Vec::with_capacity(board_width as usize * 2);
            for x in 0..board_width {
                let pos = (x, y);
                if pos == self.body[0] {
                    // Snake head — bright white to stand out
                    spans.push(Span::styled(
                        "\u{2588}\u{2588}",
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    ));
                } else if let Some(idx) = self.body.iter().position(|&p| p == pos) {
                    // Snake body — rainbow gradient trail
                    let color = gradient_color(self.phase + idx as f64 * 0.05);
                    spans.push(Span::styled("\u{2588}\u{2588}", Style::default().fg(color)));
                } else if pos == self.food {
                    // Food — bright green square
                    spans.push(Span::styled(
                        "\u{2588}\u{2588}",
                        Style::default()
                            .fg(crate::tui::GREEN)
                            .add_modifier(Modifier::BOLD),
                    ));
                } else if let Some(p) = self
                    .particles
                    .iter()
                    .find(|p| p.x.round() as u16 == x && p.y.round() as u16 == y)
                {
                    // Background particle (pad to 2 chars)
                    let (r, g, b) = GRADIENT[p.color_idx];
                    let a = p.brightness;
                    let ch = PARTICLE_CHARS[p.char_idx];
                    spans.push(Span::styled(
                        format!("{ch} "),
                        Style::default().fg(Color::Rgb(
                            (r * a) as u8,
                            (g * a) as u8,
                            (b * a) as u8,
                        )),
                    ));
                } else {
                    spans.push(Span::raw("  "));
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
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
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
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
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
    fn self_collision_ends_game() {
        let mut game = SnakeGame::new();
        // Set up snake body in a shape that will self-collide when moving right:
        // head at (5,5) moving right, body loops around so (6,5) is occupied
        game.body.clear();
        game.body.push_back((5, 5));
        game.body.push_back((4, 5));
        game.body.push_back((4, 6));
        game.body.push_back((5, 6));
        game.body.push_back((6, 6));
        game.body.push_back((6, 5));
        game.direction = Direction::Right;
        game.next_direction = Direction::Right;
        game.tick();
        assert!(game.game_over);
    }

    #[test]
    fn board_full_ends_game() {
        let mut game = SnakeGame::new();
        game.board_width = 3;
        game.board_height = 1;
        // Fill the board: snake occupies 2 of 3 cells, food at the third
        game.body.clear();
        game.body.push_back((0, 0));
        game.body.push_back((1, 0));
        game.food = (2, 0); // will be eaten but can't place nothing
        game.food_value = 1.0;
        game.direction = Direction::Left;
        game.next_direction = Direction::Left;
        // Move left wraps to wall collision, so set up rightward instead
        game.body.clear();
        game.body.push_back((1, 0));
        game.body.push_back((0, 0));
        game.direction = Direction::Right;
        game.next_direction = Direction::Right;
        game.food = (2, 0);
        game.food_value = 1.0;
        game.tick();
        // Snake ate food and now fills all 3 cells — game over (win)
        assert_eq!(game.body.len(), 3);
        assert!(game.game_over);
        assert_eq!(game.score, 1.0);
    }

    #[test]
    fn direction_opposite() {
        assert_eq!(Direction::Up.opposite(), Direction::Down);
        assert_eq!(Direction::Down.opposite(), Direction::Up);
        assert_eq!(Direction::Left.opposite(), Direction::Right);
        assert_eq!(Direction::Right.opposite(), Direction::Left);
    }
}
