# Snake Game Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a playable Snake game to the Nigel dashboard where food is green dollar signs with random monetary values and score displays as accumulated money.

**Architecture:** Self-contained `SnakeGame` struct in `src/cli/snake.rs`, integrated as a `DashboardScreen::Snake` variant. The dashboard event loop gets a polling branch for the Snake screen to drive game ticks independently of input.

**Tech Stack:** ratatui (rendering), crossterm (input/polling), rand (food placement & values), std::collections::VecDeque (snake body)

---

### Task 1: Create SnakeGame struct with core game logic

**Files:**
- Create: `src/cli/snake.rs`
- Modify: `src/cli/mod.rs:1` (add `pub mod snake;`)

**Step 1: Create the snake module with core types and game state**

```rust
// src/cli/snake.rs
use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crossterm::event::KeyCode;
use rand::Rng;
use ratatui::Frame;

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
            Self::Up => Self::Down,
            Self::Down => Self::Up,
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }
}

pub struct SnakeGame {
    body: VecDeque<(u16, u16)>,
    direction: Direction,
    next_direction: Direction,
    food: (u16, u16),
    food_value: f64,
    score: f64,
    game_over: bool,
    last_tick: Instant,
    board_width: u16,
    board_height: u16,
    rng: rand::rngs::ThreadRng,
}

impl SnakeGame {
    pub fn new() -> Self {
        let mut rng = rand::thread_rng();
        // Sensible defaults; board_size is recalculated on first draw
        let bw: u16 = 40;
        let bh: u16 = 20;
        let start_x = bw / 2;
        let start_y = bh / 2;
        let mut body = VecDeque::new();
        body.push_back((start_x, start_y));
        body.push_back((start_x.saturating_sub(1), start_y));
        body.push_back((start_x.saturating_sub(2), start_y));

        let food = Self::random_food_pos(&body, bw, bh, &mut rng);
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
            board_width: bw,
            board_height: bh,
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
        self.food = Self::random_food_pos(
            &self.body,
            self.board_width,
            self.board_height,
            &mut self.rng,
        );
        self.food_value = Self::random_food_value(&mut self.rng);
    }

    pub fn tick(&mut self) {
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

        // Wall collision
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
            _ if self.game_over => match code {
                KeyCode::Char('r') => {
                    *self = Self::new();
                    return SnakeAction::Continue;
                }
                _ => return SnakeAction::Continue,
            },
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

pub enum SnakeAction {
    Continue,
    Quit,
}
```

**Step 2: Add module declaration**

In `src/cli/mod.rs`, add `pub mod snake;` to the module list (alphabetical order, after `pub mod rules;`).

**Step 3: Build and verify**

Run: `cargo build 2>&1`
Expected: compiles with no errors (warnings about unused draw are OK)

**Step 4: Commit**

```
feat: add SnakeGame core logic

Ref #35
```

---

### Task 2: Add rendering

**Files:**
- Modify: `src/cli/snake.rs`

**Step 1: Add the draw method to SnakeGame**

Add these imports to the top of `snake.rs`:

```rust
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, canvas::{Canvas, Context}},
    Frame,
};
```

Wait — ratatui Canvas may not be needed. Use a simpler approach: render into a `Vec<Line>` and display as a `Paragraph`. Each cell is either empty, snake body, or food.

Add this `draw` method to `SnakeGame`:

```rust
pub fn draw(&mut self, frame: &mut Frame) {
    let area = frame.area();

    // Recalculate board to fit terminal (leave room for border)
    let bw = (area.width.saturating_sub(2)).min(80);
    let bh = (area.height.saturating_sub(4)).min(40);
    if bw != self.board_width || bh != self.board_height {
        self.board_width = bw;
        self.board_height = bh;
        // Clamp snake and food to new bounds
        self.body.retain(|&(x, y)| x < bw && y < bh);
        if self.body.is_empty() {
            self.body.push_back((bw / 2, bh / 2));
        }
        if self.food.0 >= bw || self.food.1 >= bh {
            self.spawn_food();
        }
    }

    let score_str = format!("${:.2}", self.score);
    let title = format!(" $ Snake $ | Score: {} ", score_str);

    let block = Block::default()
        .title(title)
        .title_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let snake_style = Style::default().fg(Color::Green);
    let head_style = Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD);
    let food_style = Style::default().fg(Color::Green).add_modifier(Modifier::BOLD);

    let mut lines: Vec<Line> = Vec::with_capacity(bh as usize);
    for y in 0..bh {
        let mut spans: Vec<Span> = Vec::with_capacity(bw as usize);
        for x in 0..bw {
            if self.body.front() == Some(&(x, y)) {
                spans.push(Span::styled("█", head_style));
            } else if self.body.contains(&(x, y)) {
                spans.push(Span::styled("█", snake_style));
            } else if (x, y) == self.food {
                spans.push(Span::styled("$", food_style));
            } else {
                spans.push(Span::raw(" "));
            }
        }
        lines.push(Line::from(spans));
    }

    let game_para = Paragraph::new(lines).block(block);
    frame.render_widget(game_para, area);

    // Game over overlay
    if self.game_over {
        let overlay_width = 34u16;
        let overlay_height = 5u16;
        let ox = area.x + (area.width.saturating_sub(overlay_width)) / 2;
        let oy = area.y + (area.height.saturating_sub(overlay_height)) / 2;
        let overlay_area = Rect::new(ox, oy, overlay_width, overlay_height);

        let game_over_block = Block::default()
            .title(" Game Over ")
            .title_style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Red));

        let game_over_text = vec![
            Line::from(Span::styled(
                format!("Final Score: ${:.2}", self.score),
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            )),
            Line::from(vec![
                Span::styled("[R]", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                Span::raw(" Restart  "),
                Span::styled("[Esc]", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                Span::raw(" Quit"),
            ]),
        ];

        let overlay_para = Paragraph::new(game_over_text)
            .block(game_over_block)
            .alignment(Alignment::Center);

        // Clear the overlay area first
        frame.render_widget(
            Paragraph::new("").style(Style::default()),
            overlay_area,
        );
        frame.render_widget(overlay_para, overlay_area);
    }
}
```

**Step 2: Build and verify**

Run: `cargo build 2>&1`
Expected: compiles with no errors

**Step 3: Commit**

```
feat: add Snake game rendering with dollar sign food

Ref #35
```

---

### Task 3: Wire Snake into the dashboard

**Files:**
- Modify: `src/cli/dashboard.rs`

**Step 1: Add import and DashboardScreen variant**

Add to the imports at top of `dashboard.rs`:

```rust
use crate::cli::snake::{SnakeAction, SnakeGame};
```

Add `Snake(SnakeGame)` variant to the `DashboardScreen` enum (after `ReportView`).

**Step 2: Add "Snake" to MENU_ITEMS**

Add `"Snake"` as the last entry in the `MENU_ITEMS` array.

**Step 3: Wire menu selection**

In `handle_home_key`, add a match arm for the new menu index (8) that sets `self.screen = DashboardScreen::Snake(SnakeGame::new())`.

**Step 4: Add draw branch**

In `Dashboard::draw()`, find where each `DashboardScreen` variant calls its draw method. Add:

```rust
DashboardScreen::Snake(ref mut game) => game.draw(frame),
```

**Step 5: Add event handling branch with polling**

This is the critical change. In the main event loop (`run()` function), the `DashboardScreen::Snake` branch needs polling instead of blocking reads.

Replace the `event::read()` call with a check: if we're on the Snake screen, use `event::poll()` + tick logic. The cleanest way is to restructure the inner loop:

In the `run()` function, before `event::read()`, add a tick check for Snake:

```rust
// Inside the loop, before event::read():
if let DashboardScreen::Snake(ref mut game) = dashboard.screen {
    let timeout = game.tick_rate();
    if event::poll(timeout)? {
        // fall through to event::read() below
    } else {
        // No input — advance the game tick
        game.do_tick();
        continue;
    }
}
// existing event::read() follows
```

Then in the key handling match, add:

```rust
DashboardScreen::Snake(ref mut game) => {
    if game.should_tick() {
        game.do_tick();
    }
    match game.handle_key(key.code) {
        SnakeAction::Quit => {
            return_home = true;
        }
        SnakeAction::Continue => {}
    }
    false
}
```

**Step 6: Build and verify**

Run: `cargo build 2>&1`
Expected: compiles with no errors

**Step 7: Manual test**

Run: `cargo run` → navigate to "Snake" menu item → Enter → play the game → Esc to return.

Verify:
- Snake moves continuously
- Arrow keys change direction
- Dollar signs appear as food
- Score accumulates as money
- Game over on wall/self collision
- R restarts, Esc quits to dashboard

**Step 8: Commit**

```
feat: wire Snake game into dashboard menu

Ref #35
```

---

### Task 4: Add unit tests for game logic

**Files:**
- Modify: `src/cli/snake.rs` (add `#[cfg(test)]` module)

**Step 1: Add tests**

```rust
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
        // Should have moved down from starting position
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
        // Force game over
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
```

**Step 2: Run tests**

Run: `cargo test --lib snake 2>&1`
Expected: all tests pass

**Step 3: Commit**

```
test: add unit tests for Snake game logic

Ref #35
```

---

### Task 5: Update documentation

**Files:**
- Modify: `CLAUDE.md`
- Modify: `README.md`

**Step 1: Update CLAUDE.md**

In the `MENU_ITEMS` / dashboard description, add mention of the Snake game easter egg.

In `DashboardScreen` enum list, add `Snake(SnakeGame)`.

In the Project Structure section, add `snake.rs` under `src/cli/`:
```
    snake.rs            # Snake game easter egg (ratatui, accessible from dashboard)
```

**Step 2: Update README.md**

Add a brief mention of the Snake game under the Features section or as a fun note.

**Step 3: Commit**

```
docs: document Snake game feature

Ref #35
```

---

### Task 6: Final verification

**Step 1: Run full test suite**

Run: `cargo test 2>&1`
Expected: all tests pass

**Step 2: Run clippy**

Run: `cargo clippy 2>&1`
Expected: no warnings

**Step 3: Run fmt check**

Run: `cargo fmt --check 2>&1`
Expected: no formatting issues

**Step 4: Manual smoke test**

Run: `cargo run`
- Launch dashboard
- Select "Snake"
- Play game, eat dollar signs, verify score shows as money
- Hit wall → game over screen shows final score
- Press R → restarts
- Press Esc → returns to dashboard
- Verify dashboard still works normally
