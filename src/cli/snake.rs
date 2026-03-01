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

#[derive(Clone, Copy, PartialEq)]
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
        }
    }

    fn random_food_pos(
        body: &VecDeque<(u16, u16)>,
        w: u16,
        h: u16,
        rng: &mut rand::rngs::ThreadRng,
    ) -> (u16, u16) {
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
            KeyCode::Up => {
                if self.direction != Direction::Down {
                    self.next_direction = Direction::Up;
                }
            }
            KeyCode::Down => {
                if self.direction != Direction::Up {
                    self.next_direction = Direction::Down;
                }
            }
            KeyCode::Left => {
                if self.direction != Direction::Right {
                    self.next_direction = Direction::Left;
                }
            }
            KeyCode::Right => {
                if self.direction != Direction::Left {
                    self.next_direction = Direction::Right;
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
    }

    pub fn tick_rate(&self) -> Duration {
        TICK_RATE.saturating_sub(self.last_tick.elapsed())
    }

    pub fn draw(&mut self, frame: &mut Frame) {
        let area = frame.area();

        // Board sizing: leave room for border (2 cols) and title/footer (4 rows)
        let board_width = (area.width.saturating_sub(2)).min(80);
        let board_height = (area.height.saturating_sub(4)).min(40);

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
            .border_style(Style::default().fg(Color::DarkGray))
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

        // Build game field
        let mut lines: Vec<Line> = Vec::with_capacity(board_height as usize);
        for y in 0..board_height {
            let mut spans: Vec<Span> = Vec::with_capacity(board_width as usize);
            for x in 0..board_width {
                let pos = (x, y);
                if pos == self.body[0] {
                    // Snake head
                    spans.push(Span::styled(
                        "\u{2588}",
                        Style::default()
                            .fg(Color::LightGreen)
                            .add_modifier(Modifier::BOLD),
                    ));
                } else if self.body.contains(&pos) {
                    // Snake body
                    spans.push(Span::styled(
                        "\u{2588}",
                        Style::default().fg(Color::Green),
                    ));
                } else if pos == self.food {
                    // Food
                    spans.push(Span::styled(
                        "$",
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
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
