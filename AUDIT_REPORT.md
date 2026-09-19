# Poly Project — Deep-Dive Audit Report

**Date:** 2026-09-19 · **Branch:** v2.0-dev · **Toolchain:** workspace compiles clean, 425+ tests green

> **STATUS UPDATE:** Findings 1–6 have been **fixed** in this branch (asm step negation,
> JS descending loops, C %f float printing, checker return-type diagnostic, parser
> negative-start extract_range, `tests/asm_target_tests.poly` return types). Full suite
> now passes **494 tests, 0 failed**; all five differential suites remain byte-identical
> across rust/C/JS. Findings 7–13 are open.

Findings are ranked by severity. Every "confirmed" item was reproduced empirically with
`compiler/target/debug/poly` and verified against at least one generated target.

---

## 🔴 Critical — silent wrong behavior

### 1. asm backend: `loop a..b step -N` is an INFINITE loop (confirmed)
- **Where:** `compiler/crates/poly-asm-codegen/src/lib.rs` ~line 1240:
  ~~~rust
  let step_val: i64 = if let Some(Expression::IntLiteral(v)) = step {
      v.parse().unwrap_or(1)
  } else {
      1
  };
  ~~~
  A negative step lexes as `UnaryOp(Neg, IntLiteral("2"))`, **not** `IntLiteral("-2")`, so
  `step_val` falls back to **1**. Meanwhile `descending` (line 1229, via
  `is_negative_expression`) *is* detected, flipping the comparison to `jge` — so the loop
  increments `+1` while the exit condition can never be met.
- **Reproduced:** `loop i 10..1 step -1` assembled with `gcc -no-pie -nostartfiles` and run:
  printed `10, 11, 12, …` (1,589,159 lines in 1s, killed by timeout). Same for `step -2`
  (`addq $1` in the emitted `for_incr` block).
- **Fix:** when `descending`, emit `subq $|step_val|` (negate `step_val`), and parse the
  magnitude out of the `UnaryOp::Neg` wrapper rather than falling back to 1.

### 2. JS backend: `step -N` loops do zero iterations silently (confirmed)
- **Where:** `compiler/crates/poly-js-codegen/src/lib.rs` lines 314–331:
  - `comparison` is hard-coded `<=` / `<` — never flipped for descending loops;
  - `update` is `i -= (-1)` (double negation), so `i -= (-1)` still increments.
- **Reproduced:** `loop i 10..1 step -1` emitted
  `for (let i = 10; i <= 1; i -= (-1))` → zero iterations. `step -2` → `i -= (-2)`,
  also zero iterations. (Rust target correctly emits `(1..=10).rev().step_by(2)`; C
  correctly emits `i -= -((-2))` with `>=`.)
- **Fix:** flip `comparison` to `>=` / `>` when `descending`, and emit `i += step_value`
  in that case (step_value already carries the sign) or emit `-(-step)` properly.

### 3. C backend: f64 variables print as garbage via `%d` (confirmed)
- **Where:** `compiler/crates/poly-c-codegen/src/lib.rs` — `printf_parts` →
  `format_for_expression` (line 1446):
  ~~~rust
  fn format_for_expression(&self, expression: &Expression) -> String {
      match expression {
          Expression::FloatLiteral(_) => "%f".to_string(),
          _ => "%d".to_string(),
      }
  }
  ~~~
  Float *literals* print with `%f`, but **any float-typed variable or computed float**
  falls through to `%d`. Passing a `double` to `%d` is varargs UB → garbage.
- **Reproduced:** `var f f64 := 0.1 + 0.2; put f` emitted `printf("%d\n", f);` and printed
  `1854876209` (three times in a row — the same garbage from the same bit pattern).
- **Fix:** track f64 variables the way `var_widths` already tracks 64-bit ints, and pick
  `%f` for float-valued expressions (including `BinaryOp` operands of float literals).

---

## 🟠 High — parse/type-check bugs

### 4. Checker misses "return with value in fn with no declared return type" (confirmed)
- **Where:** `compiler/crates/poly-transpiler/src/checker.rs` `check_return` (~line 632):
  when `expected` is `None`, a `return <value>` produces **no error**. The generated Rust
  `fn f(v: i32) { return (v * 2); }` then fails rustc with E0308.
- **Reproduced:** minimal 5-line file:
  ~~~poly fragment
  fn double_val(v: i32)
      return v * 2
  end fn
  ~~~
  → `--check` fails with a raw rustc error instead of a Poly diagnostic.
- **Fix:** in `check_return`, when `expected` is `None` and `value` is `Some(_)`, report
  "function has no declared return type but returns a value" (or infer/annotate).
- **Note:** `tests/asm_target_tests.poly` (line 37) has exactly this shape, so the repo's
  own test file fails default-target checking (`rust compilation error: E0308`).

### 5. `extract_range` misses negative-literal range starts → mis-parses as infinite loop (confirmed)
- **Where:** `compiler/crates/poly-parser/src/parser.rs` `extract_range` (line 2228):
  the un-shelling branch requires `matches!(start.as_ref(), Expression::IntLiteral(_))`.
  `TOTAL - -4..TOTAL - 1` shells a `Range(UnaryOp(Neg, 4), …)` inside `Sub`, which
  doesn't match, so `parse_loop_expression`'s backtracking probe classifies the construct
  as an **infinite loop**.
- **Reproduced:** `loop i TOTAL - -4..TOTAL - 1` → AST contains `InfiniteLoop`, and the
  user gets the misleading `Error: unknown variable 'i'`.
- **Fix:** widen the guard to also accept `UnaryOp::Neg` over an int literal as the range
  start.

### 6. No `step <identifier>` in loop ranges
- **Reproduced:** `loop i 5..1 step my_step` → `Parse error: Expected identifier, got Step`
  (the parser tries to re-parse the identifier position; `step` before a variable name is
  rejected). Even when renamed, see finding 7.
- **Fix:** accept an expression after `step` (or at minimum identifiers).

---

## 🟡 Medium — cross-target semantic divergence

### 7. Runtime (variable) negative step: inconsistent across all four targets (confirmed)
- **Repro:** `var my_step i32 := 0 - 1; loop i 5..1 step my_step` →
  - Rust: `(5..=1).step_by(my_step as usize)` — `as usize` makes the negative value a
    huge positive; range is empty → **0 iterations** (silently wrong);
  - C: `for (i = 5; i <= 1; i += my_step)` → **0 iterations**;
  - JS: `for (let i = 5; i <= 1; i += my_step)` → **0 iterations**;
  - asm: never even reaches the backend (parse error, finding 6).
- All four agree on *empty range = zero iterations* by accident, but a descending loop
  written with a computed negative step is simply broken everywhere. Either support it
  (runtime check of step sign) or make the checker reject non-literal/negative steps
  explicitly.
- **FIXED:** non-literal steps now lower to a runtime-sign dispatch on every backend:
  C/JS use a ternary loop condition over a step slot, the Rust path re-enters the
  matching direction's sequence per visited value (`once().flat_map(...)`), and asm
  stores the step in a fresh slot with per-sign check labels. Literal steps keep the
  compile-time fast paths; a zero step yields zero iterations everywhere.

### 8. JS `shift right` on negative numbers is an unsigned shift (confirmed)
- **Repro:** `put -5 shift right 1` →
  - Rust: `-3`, C: `-3` (arithmetic shift), **JS: `2147483645`** (emitted `>>>`).
  - `put -8 shift right 2` → Rust/C: `-2`, JS: `1073741822`.
- **Where:** poly-js-codegen emits `>>>` for `shift right`. It should emit `>>`
  (JS `>>` *is* arithmetic on 32-bit ints).
- **FIXED:** now emits arithmetic `>>`, matching Rust/C/asm. See regression test in
  `poly-js-codegen/src/lib.rs`.

### 9. Mixed int/float arithmetic is not type-checked (confirmed)
- **Repro:** `put 1 + 0.5` and `put 7 as f64 / 2`:
  - rust target: raw rustc E0277 errors (no Poly diagnostic);
  - C target: compiles and prints garbage (`1040704625` — a `%d`-printed double);
  - JS target: silently truncates (`7 as f64 / 2` → `3` via `Math.trunc`).
- The checker should either insert implicit widening (like most dynamic-to-static
  transpilers) or reject mixed arithmetic with a clear Poly-level error on **all** targets.
- **FIXED:** the checker now rejects mixed integer/float arithmetic for `+` and
  `*`/`/`/`-`/`mod` with "requires an explicit cast" guidance, so every target sees the
  same program. Same-class arithmetic and explicit `as` casts are unaffected. See
  `TypeChecker::require_same_numeric_class` and regression test in `checker.rs`.
- Related: `put 7 as f64 / 2` on the C target emits `(double)(7) / 2` (double division =
  3.5) while JS truncates to `3` — `as` binds the value, but `format_for_expression`
  prints it with `%d`, hiding the float. (Rust target fails to compile.)

---

## 🔵 Low / robustness

### 10. While-loops encoded as if-without-else (`else_block: None` vs `Some(empty)`)
- The parser turns `while` into an `IfExpression` with `else_block: None`; codegen and the
  IR treat "if-without-else in statement position" as a while loop. The `is_while` flag on
  IR `Statement::If` is never actually set. The invariant survives constant folding and
  `while true` today (verified), but any future pass that adds an else branch, or any
  backend that reasons differently about if-without-else, silently breaks every while loop.
  Recommend carrying an explicit `Loop::While` node through the IR.

### 11. `extract_range`-adjacent design fragility
- Ranges only re-associate out of the right operand of arithmetic when the range's *start*
  is a bare `IntLiteral`. Any other start shape (finding 5's negative literal, or a
  parenthesized start) falls back to single-value loop semantics silently. Consider
  banning `..` inside arithmetic expressions at the parse level instead of un-shelling.

### 12. `0 - 5 shift right 1` parses as `(0 - 5) >> 1` in Rust/C/JS (all: `-3`) — consistent,
but `-5 shift right 1` is `(-5) >> 1`; fine. Unary minus vs `shift` precedence is
documented nowhere in the spec I could find; worth a sentence in POLY_SPEC_v2.md.

### 13. Rust target: `1 << 2 as i64` casts only the shift operand
`as` binds tighter than binary ops on the right operand (emits `1 << (2 as i64)`), which
matches Rust but diverges visually from C (`(int64_t)(2)`). Consistent, just document.

---

## ✅ Verified-correct areas

- Workspace builds clean; **425+ unit/integration/snapshot tests pass**.
- All five differential suites (`diff_core`, `diff_strings`, `diff_string_edges`,
  `diff_structs`, `diff_optimizer`) produce **byte-identical output on rust/C/JS**.
- `while true { … break … }` survives the optimizer and runs correctly on JS target.
- Ascending `step N` works on all four targets.
- `--check` on every example and the tetris project: OK.
- Target-specific tests (`asm_target_tests`, `c_target_tests`, `js_target_tests`,
  `extern_asm_deref`) pass on their intended targets only, as designed.
- `stop_at_double_minus` flag-value parsing restores state correctly on error paths.
- Infinite-loop vs range-loop backtracking disambiguation is careful and mostly correct
  (only finding 5's negative-start corner misses).
- `tests/stress_loops.poly` arithmetic expectations verified correct
  (5,000,050,000 / 100000 / 41,667,916,675,000 / 1).

## .poly test/example file findings

- `tests/asm_target_tests.poly:37` — `fn double_val(v: i32)` returns a value without a
  declared return type; fails default-rust `--check` (finding 4). Add `: i32`.
- `tests/c_target_tests.poly`, `tests/js_target_tests.poly`, `tests/extern_asm_deref.poly`
  fail `--check` on non-native targets — expected (target-specific externs), but the
  default-target failure in `asm_target_tests.poly` is a genuine latent bug per finding 4.
- All `examples/*.poly` files: syntax and logic verified clean.

## Recommended fix order

1. asm `step_val` negation (critical, infinite loop, tiny fix).
2. JS descending-loop comparison + update (critical, silent data loss).
3. C `%f` for float variables (critical, garbage output).
4. Checker: value-return in fn without return type (turns rustc errors into Poly errors).
5. Parser: `extract_range` negative-start guard (misleading parse errors).
6. Checker: mixed int/float arithmetic now rejected with explicit-cast guidance
   (done); variable negative steps now supported via runtime-sign dispatch on all
   four backends (done).
7. Longer term: explicit `Loop` node in IR (retire the while-as-if encoding).
