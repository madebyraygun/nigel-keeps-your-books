# Snake Game — Design

**Issue:** #35
**Date:** 2026-02-28

## Overview

Add a playable Snake game to the Nigel dashboard as an easter egg. Food is rendered as green dollar signs with random monetary values. Score is displayed as accumulated money. Escape returns to the dashboard.

## Architecture

**New file:** `src/cli/snake.rs` — self-contained `SnakeGame` struct.

**Integration:** New `DashboardScreen::Snake(SnakeGame)` variant wired into the dashboard menu and event loop.

## Game State

```rust
struct SnakeGame {
    body: VecDeque<(u16, u16)>,   // snake segments, head at front
    direction: Direction,          // Up/Down/Left/Right enum
    food: (u16, u16),             // current dollar sign position
    food_value: f64,              // random $1.00–$9.99
    score: f64,                   // accumulated money
    game_over: bool,
    last_tick: Instant,           // consistent tick rate
    board_size: (u16, u16),       // computed from frame
    rng: ThreadRng,
}
```

## Rendering

- Border block titled `"$ Snake $"` with yellow header style
- Snake body: `█` in green
- Food: `$` in bright green/bold
- Score in title: `"Score: $12.50"`
- Game over: centered overlay with final score, `[R] Restart  [Esc] Quit`

## Controls

- Arrow keys: change direction (no 180-degree reversal)
- Esc: return to dashboard at any time
- R: restart on game over screen

## Tick Mechanism

Dashboard event loop uses blocking `event::read()`. For the Snake screen, use `event::poll(~150ms)` timeout instead. If no key arrives, the snake advances one step. Only the Snake branch uses polling; other screens remain unchanged.

## Menu Integration

Add `"Snake"` as the last item in `MENU_ITEMS`. On selection, create `SnakeGame::new()` and transition to `DashboardScreen::Snake`.
