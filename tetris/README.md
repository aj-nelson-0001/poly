# Poly Tetris

A faithful Poly rewrite of the original JavaScript Tetris (`/home/andy/Projects/tetris`),
targeting **Rust** as its only backend. Game logic lives in Poly (`tetris.poly`); the
platform layer (window, input, audio, clock, RNG seed, high-score persistence) is a
hand-written Rust `#rust` block at the bottom of the same file.

## Layout

~~~
tetris.poly                  the whole program (Poly logic + #rust platform layer)
rust_output/tetris/          generated Cargo project (poly --project)
rust_output/tetris/src/main.rs   generated Rust (do not edit by hand)
~~~

## Build & run

Requires the Poly compiler (`compiler/target/release/poly`) and ALSA headers
(`libasound2-dev`) for sound.

~~~sh
# validate (dep declarations resolve minifb/alsa through Cargo)
poly tetris.poly --check

# regenerate the Rust project from tetris.poly
poly --project rust_output/tetris tetris.poly

cargo build --release --manifest-path rust_output/tetris/Cargo.toml

# play
./rust_output/tetris/target/release/tetris

# headless logic self-test (no window)
./rust_output/tetris/target/release/tetris --test
~~~

Note: `poly --project` will not overwrite an existing directory — remove
`rust_output/tetris` first. The `dep minifb = "0.27"` and `dep alsa = "0.9"`
declarations at the top of tetris.poly are emitted into Cargo.toml
automatically.

## Controls

| Key            | Action                    |
|----------------|---------------------------|
| ←/→            | Move (with DAS auto-repeat) |
| ↓ or V         | Soft drop                 |
| ↑ or X         | Rotate clockwise          |
| Z              | Rotate counter-clockwise  |
| Space          | Hard drop                 |
| C              | Hold                      |
| P              | Pause                     |
| R              | Restart                   |
| M              | Toggle mute               |
| Enter          | Start / restart after game over |
| Esc            | Quit                      |

## Faithfulness to the original

- Same rotation system (the original's own per-rotation shape set, not SRS) and
  simple wall kicks (`0, -1, 1, -2, 2`).
- NES scoring: 40/100/300/800 × (level + 1), +1 per soft-drop cell, +2 per
  hard-drop cell.
- Level up every 10 lines; NES gravity table in frames (48 → 1, capped at
  level 29), converted to milliseconds.
- 7-bag randomizer with one-bag lookahead and a 5-piece preview.
- Hold with single-use lock until the next piece locks.
- Ghost piece, line-clear flash (the cleared rows' own cells fading out in the
  top slots over 180 ms, alpha floor 0.4, exactly like the original's
  `flashes` array), and the original's WebAudio sounds, re-synthesized
  natively through ALSA (square/noise tones, mixed chords).
- High score persisted to `/tmp/poly_tetris_highscore`.

### Visuals

The window reproduces the original page's look at 2× scale (660×732):

- Radial dark-plum background gradient (`#15083d` → `#05010a`) and a CRT
  scanline overlay on the playfield.
- 300×600 board with `#000814` grid background, faint purple grid lines and a
  `#2a1c63` frame.
- Beveled cell sprites matching the original's `_cellSprite`: 1 px gap,
  45 % white top/left edge, 45 % black bottom/right edge, faint inner glow.
- Authentic piece palette from the original's `COLORS` (I cyan, O yellow,
  T purple, S green, Z red, J blue, L orange).
- Left panel with Score/High/Level/Lines + Hold box; right panel with the
  Next-5 queue (first slot highlighted) and a Controls list.
- Line-clear clearing only the completed rows — verified by a regression
  self-test that locks a stack and asserts every non-full row survives
  cell-for-cell.

## Architecture notes (Poly → Rust)

Poly moves values into functions, so the code follows two patterns:

1. **Take-and-return state**: every `GameState`/`Bag` method is
   `fn m(self, ...): GameState` and ends with `return self`; callers write
   `s := s.m(...)`.
2. **Query via `q_*` fields**: a method returning a bare primitive would move the
   receiver on each call, so read-only queries (`collides`, `query_drop`,
   `query_cell`, `query_queue`, `Bag.peek`) stash their answer in a Copy field
   (`q_hit`, `q_ghost_row`, `q_scored`, `q_queue`, `q_peek`) and return `self`.

Other constraints encoded here that future edits should respect:

- `self.method(self.field, ...)` moves `self` before the arguments evaluate —
  copy fields to locals first.
- Loop ranges (`loop i a..b`) are inclusive and only detected when the range
  starts with a numeric literal; use `while` loops for variable bounds.
  (Older compilers mis-generated `while` loops nested inside an `if` as one-shot
  `if`s — fixed in the compiler; the game no longer needs to avoid that shape.)
- `spawn` and `step` are reserved words.
- Equality compares are strictly typed: cast explicitly (`x as i64`, `16 as i64`)
  instead of mixing i32/i64/u64.
- The framebuffer is a fresh `Vec<i32>` per frame (`make_fb`), converted to
  `Vec<u32>` only inside `win_present`.
