# dclock_poly — Digital Clock in Poly

A rewrite of the sibling Rust project `dclock` (a 5×5 bitmap digital clock in
a resizable `minifb` window) in the **Poly** language.

## What is Poly here?

The clock's *logic* is written entirely in Poly (`dclock.poly`): the digit
bitmaps, bitmap scaling, frame layout, colon drawing, time expansion, and the
main loop. Poly transpiles that code to Rust.

The two things Poly has no built-in for — opening a window (`minifb` crate)
and reading wall-clock time (`chrono` crate) — live in a `#rust` block at the
top of `dclock.poly` as a small platform layer with five functions:

| Extern function      | Purpose                                            |
|----------------------|----------------------------------------------------|
| `window_open`        | Create the resizable minifb window                 |
| `window_should_close`| True when the window closes or Escape is pressed   |
| `window_resize`      | Returns new `[width, height]`, `[-1, -1]` if same  |
| `window_present`     | Blit the pixel frame (converts `i32` → `u32`)      |
| `clock_seconds_now`  | `[hour, minute, second]`, refreshed once per second|

Each has an explicit `extern rust fn ...` declaration, so the program also
passes `poly --check --strict`.

## Files

- `dclock.poly` — the Poly program (source of truth)
- `src/main.rs` — the Rust code Poly transpiles it to (checked in so the
  project builds like the original; regenerate with the command below)
- `Cargo.toml` — dependencies for the generated code (`chrono`, `minifb`)

## Build & run

~~~bash
# From this directory:
cargo run --release

# Or recompile straight from the Poly source with the Poly compiler:
poly dclock.poly                 # check + build + run
poly --project build dclock.poly # regenerate a Cargo project in build/
~~~

Note: `poly --project` generates a dependency-free `Cargo.toml`; if you
regenerate, re-add `chrono = "0.4"` and `minifb = "0.23"` under
`[dependencies]` (as in the checked-in `Cargo.toml`).

## Translation notes (Rust → Poly)

- Poly `loop a..b` is **inclusive**, so loops over `0..n` elements are
  written `loop i 0..n - 1`.
- Poly's checker requires exact types, and integer literals/consts are
  `i32`, so pixel buffers are `Vec<i32>`; `window_present` converts them
  to the `Vec<u32>` minifb expects.
- Poly struct values move, and elements cannot be copied out of a
  `Vec<struct>` by index, so `draw_time` reads `digit_buffers[d].buffer[...]`
  inline instead of cloning a `DigitBuffer` out of the vector.
- The original updated the display only when the second changed; here the
  per-second throttle lives in `clock_seconds_now` (foreign side) and the
  frame is rebuilt each iteration — Poly's move semantics make "reuse the
  old buffer unless changed" awkward, and rebuilding is cheap.
- `DIGITS: [u32; 10]` became a `digit_bits` match function because Poly
  currently cannot emit a `const` array/vector.
