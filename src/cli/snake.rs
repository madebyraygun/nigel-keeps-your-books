use crossterm::event::KeyCode;
use rand::Rng;
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
        body.push_back((cx - 1, cy));
        body.push_back((cx - 2, cy));

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
            Direction::Up => {
                if hy == 0 {
                    self.game_over = true;
                    return;
                }
                (hx, hy - 1)
            }
            Direction::Down => {
                let ny = hy + 1;
                if ny >= self.board_height {
                    self.game_over = true;
                    return;
                }
                (hx, ny)
            }
            Direction::Left => {
                if hx == 0 {
                    self.game_over = true;
                    return;
                }
                (hx - 1, hy)
            }
            Direction::Right => {
                let nx = hx + 1;
                if nx >= self.board_width {
                    self.game_over = true;
                    return;
                }
                (nx, hy)
            }
        };

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
}
