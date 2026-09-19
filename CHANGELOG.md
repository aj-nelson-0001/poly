# Changelog

All notable changes to the Poly language compiler will be documented in this file.

## [2.0.0-preview.3] - 2026-09-11

### Fixed

- **CLI flags are now accepted in any position.** `poly file.poly --emit-rust`,
  `poly file.poly --check`, and every other flag previously worked only when
  the flag preceded the file (`poly --emit-rust file.poly`); a file-first
  invocation silently fell through to a default build. Arguments are now
  pre-parsed into flags plus positionals, so documented invocations work in
  any order, unknown flags are rejected in every position, and `--project`
  accepts its two operands either way around.
- **`--target js` now runs the generated program with Node** when a default
  build produces a `.js` file, matching the documented CLI behavior (the
  Rust target mirrors this by building the generated project). When Node is
  unavailable the emitted file is still verified with `node --check` and the
  build succeeds.
- **Retired-syntax diagnostics restored.** The `set x to y` detector matched
  an `Identifier("to")` token, but `to` lexes as a keyword, so the migration
  hint never fired. `set x to y`, `add x n`, `sub x n`, `inc x`, and `dec x`
  now all produce named errors with targeted suggestions, and `x += value` /
  `x -= value` (lexed as two tokens) get the same treatment. Assignments to
  variables that happen to be named `add` (`add := 5`) and calls like
  `inc(counter)` are still accepted.
- **Macros now expand in value position.** `var y := double(21)` previously
  failed with `unknown function` because expansion only ran for
  statement-position calls. A macro whose body is a single expression or
  `return expr` lowers to that expression; multi-statement macros still
  expand only at statement position, with a clear arity error in both
  positions.

### Added

- MIT `LICENSE` file (declared by every workspace `Cargo.toml` but missing
  from the repository).
- 13 new tests: 8 parser tests covering the restored diagnostics,
  false-positive guards, and value-position macro expansion; 5 CLI
  end-to-end tests running the real binary for file-first flags, JS
  auto-run, unknown-flag rejection, and migration hints.

### Documentation

- Roadmap kept-features table no longer lists retired `add`/`sub`/`inc`/
  `dec` as arithmetic syntax; `POLY_JS_DESIGN.md` status updated from
  "proposal" to "implemented"; `POLY_SPEC_v2.md` documents that `^` is
  retired xor spelling (not exponentiation) and that Poly has no power
  operator; PROGRESS.md records the audit-fix session and corrects the
  stale string-mutation claim.

## [2.0.0-preview.12] - 2026-09-19

### Changed

- **Strict same-class arithmetic.** Mixed integer/float arithmetic
  (`1 + 0.5`, `count + total_f64`) is now rejected at the Poly level with a
  diagnostic naming the cast (`cast one operand with \`as\``). Previously the
  rust target rejected such programs at rustc while c printed garbage and js
  silently truncated — the same source produced three different outcomes.
- **`while` is a dedicated AST/IR node.** The parser no longer encodes `while`
  as an if-expression without an else block for backends to decode; the
  checker no longer treats every if-without-else as a loop (so `break` inside
  a genuine if-without-else is now correctly an error), and the intermediate
  representation pipeline generates a plain `if` for one.

### Added

- **Runtime-sign dispatch for variable loop steps.**
  `loop i 10..1 step s` previously used the compile-time comparison
  direction, so a runtime-negative step silently produced zero iterations on
  every target (and a zero step hung the asm backend). All four backends now
  dispatch on the step's sign per iteration; literal steps keep their
  compile-time fast paths and a zero step terminates.
- **Loop-step semantics documented** in `POLY_SPEC_v2.md` alongside the
  same-class arithmetic rule.
- **`tests/diff_whiles.poly`** differential suite covering `while` with
  `break`/`continue`, nested loops, `while true` under constant folding, and
  runtime-step `loop` ranges — byte-identical output required on
  rust/c/js/asm. Wired into the differential workflow.
- **`AUDIT_REPORT.md`** documenting the full cross-target audit that produced
  this changelog's fixes (12 findings, empirically confirmed, with repros).

### Fixed

- **asm backend: descending literal steps looped forever.**
  `loop i 10..1 step -1` parsed the step with `if let Some(IntLiteral(v))`,
  which never matches `-1` (it lexes as a negation), so the induction fell
  back to `+1` while the comparison correctly flipped — an infinite loop.
- **js backend: descending literal steps did zero iterations.** The
  comparison never flipped (`i <= 1`) and the update double-negated
  (`i -= (-1)`).
- **c backend: f64 variables printed garbage.** `put f` on an `f64` variable
  emitted `printf("%d\n", f)`; float classification now covers variables and
  float-valued expressions, not just float literals.
- **Checker: value-returning functions without a declared return type**
  passed checking and failed only at rustc; this now produces a Poly-level
  diagnostic (the repository's own `tests/asm_target_tests.poly` triggered
  it).
- **Parser: negative-literal range starts** (`loop i TOTAL - -4..TOTAL - 1`)
  mis-parsed as an infinite loop with a misleading `unknown variable`
  error; `extract_range` now unwraps the negation shell.
- **Bool rendering normalized across targets.** `put flag`,
  `flag.to_string()`, and bool operands in concat chains now print
  `true`/`false` on every target; C (via a new `poly_bool_str` helper) and
  asm (`_print_bool`/`_bool_str`/`_bool_to_string`) previously printed
  `1`/`0`, diverging from the Rust reference implementation. Covered by
  the differential suite (`diff_string_edges.poly` grew bool cases).

### Added

- **Arena-exhaustion differential coverage.**
  `tests/stress_arena_exhaust.poly` drives the asm target's 1 MiB string
  arena past its limit nightly and requires the loud-failure contract:
  diagnostic on stderr, exit code 42 (matching the vector arena's
  convention). `check_backends.py` now supports target-asymmetric programs
  via `# EXPECTED-FAIL <target>: exit=…, stderr=…` directives, so one file
  can expect success on rust/c/js and a specific failure mode on asm. The
  asm exhaustion diagnostics also moved from stdout to stderr, matching
  the C backend's `fputs(stderr)`.

- **Nightly stress differential suite.** `scripts/check_backends.py --stress`
  runs the heavier `tests/stress_*.poly` programs that are too slow for the
  every-push suite: `stress_strings.poly` pressures the asm target's 1 MiB
  string arena with two 600-iteration concat builds (~700 KB of bump
  allocations, calibrated to stay inside the arena) plus a mixed chain, and
  `stress_loops.poly` drives 64-bit accumulation past the i32 boundary
  (Σ1..100000 = 5,000,050,000) with an i64 loop counter and mixed-width
  operands. Wired into the nightly differential workflow.
- **Capability-parity test** (`poly-transpiler/tests/capability_parity.rs`)
  pinning the per-backend method-call matrix — scalar `.to_string()`, string
  `.len()`, vector `.len()`/`.push()`/`.pop()`, and the rejection set — so a
  backend silently gaining or losing a capability fails CI instead of
  drifting. Expected rows mirror `POLY_V2_SUPPORT_MATRIX.md`.

### Fixed

- **asm backend: `put "v=" + 42` failed to link.** The put-streaming
  concat fallback referenced `_print_int_nobuf` but the runtime never
  emitted it — any mixed chain with an integer leaf was a link error. The
  helper is now emitted on demand (without `_print_int`'s newline byte in
  the length count), and chain leaves are dispatched by kind: string-
  valued leaves write through `_strlen` (printing a to_string result
  through `_print_int` emitted its arena *address*), bool leaves render
  `true`/`false`, and integers print in decimal.
- **asm backend: `_bool_to_string` never NUL-terminated its output**
  (the epilogue stored the index byte instead of 0), corrupting later
  string operations. Also: the string-arena exhaustion diagnostic
  over-wrote its byte length (6 stray bytes of adjacent .data reached
  stdout) and exited 1 instead of the vector arena's 42.
- **asm backend: value-position `n.to_string()` in inferred declarations.**
  `var s := n.to_string()` was misclassified because `is_string_valued`
  returned `false` for every method call, so `s` was treated as an integer
  and `s.len()`/`put s` failed or rendered wrongly. Scalar `.to_string()` is
  now recognized as string-valued (`.len()` deliberately is not — it returns
  an integer).

## [2.0.0-preview.11] - 2026-09-18

### Fixed

- **Compiler audit (optimizer/checker/generator):** constant folding no
  longer panics on over-wide shifts (`1 << 64`), no longer folds division
  or remainder by a foldable-zero constant to `0` (the target's runtime
  check now fires), and guards identity folds (`x*1`, `x+0`, `x-x`,
  `x bitor 0`) by operand type. The function inliner substitutes
  parameters recursively through all expression kinds — an inlined
  `return xs[0]` used to leave the parameter unresolved and emit invalid
  Rust — and refuses to inline when a free identifier would remain.
  Module functions are no longer collected twice by the checker (bogus
  "duplicate function declaration" for any `module` user), and
  declarations nested inside function bodies lower to no-ops instead of
  `unreachable!()`.
- **C backend string printing:** `put` of compound string leaves
  (`.len()`, parenthesized concat) printed through `%s` with an `int`
  argument (printf segfault). The format default is `%d`; `%s` requires
  proven string-valuedness, which now also unwraps parentheses.
- **asm backend:** value-position string concatenation no longer silently
  stores 0 (later segfaulting when printed as a string pointer).
- Examples: fixed a corrupted string literal in `state_machine.poly`
  (`"Menunicode "` → `"Menu"`) and a duplicated `find_min_index`
  definition in `sorting_algorithms.poly`.

### Added

- **String support across all four targets:** C materializes value-
  position concat via an emitted `poly_concat` helper (exact-size malloc,
  no realloc) and scalar `to_string()` via typed `poly_int_to_string` /
  `poly_float_to_string` helpers; concat chains containing `to_string()`
  infer as `const char *` and print with `%s`. The asm target materializes
  concat through a 1 MiB `_str_arena` bump allocator
  (`_str_alloc`/`_strlen`/`_poly_concat`), records inferred string
  variables at declaration, and supports `.len()` via `_strlen`. C lowers
  `.len()` on strings to `strlen`.
- `tests/diff_strings.poly` joins the differential backend suite: value-
  and put-position concat chains must agree on rust/c/js/asm.
- Extensive example commentary: 15 example programs and the asm benchmark
  gained purpose-and-technique header documentation; all 27 examples
  verified to compile and run.

### Changed

- **Generated-code speed:** the C backend compiles with `-O2` (was
  `-O0`); generated Cargo projects pin `[profile.release] opt-level = 3`;
  Rust `s := s + x` appends lower to in-place `push_str` instead of a
  fresh `format!` per iteration (~500x on a 100k-append loop), and nested
  string chains flatten into a single `format!` call.
- Documentation: char/string type-mapping contradictions resolved across
  the spec and README; grammar's `break` corrected to value-less;
  `xor`-on-bool claim fixed; CLI option rows completed; support matrix
  and cheat sheet now describe string operations per target.

## [2.0.0-preview.10] - 2026-09-17

### Fixed

- **All four backends now agree on the differential suite.** A sweep of one
  program through rust/c/js/asm exposed and fixed five defects: C and JS
  rejected match expressions in value position; the asm runtime negated the
  syscall return value instead of the number when printing negative integers
  (every negative printed as `-1`); asm expression temporaries shared one
  stack slot per expression kind, corrupting nested arithmetic; asm passed a
  struct-returning call's first field value where a struct address was
  expected (segfault); and C declared struct-returning call results as
  `int32_t`, generating C that did not compile.

### Added

- **Differential backend suite as a permanent guard.** `tests/diff_*.poly`
  programs carry expected output headers; `scripts/check_backends.py` runs
  each through every target and fails on divergence. Wired into the Linux CI
  job on every push/PR, with a nightly schedule in
  `.github/workflows/differential.yml`. Support matrix and spec document
  value-position match semantics.

## [2.0.0-preview.9] - 2026-09-17

### Added

- **Generated-project marker and match-expression initializers are now
  documented.** `POLY_SPEC_v2.md` specifies the `.poly-generated` record
  format, its relative-path portability guarantee, and the refresh-safety
  rule it enables; the variable-declaration section documents match
  expressions as initializers. The cheat sheet summarizes the marker.

## [2.0.0-preview.8] - 2026-09-17

### Fixed

- **Generated projects record their source portably.** The `.poly-generated`
  marker embedded the generating machine's absolute source path, making
  committed output irreproducible across machines; the marker now records the
  source relative to the generated project, with refresh-mode identity checks
  resolving records against the output directory.

### Changed

- **tetris builds warning-free and its output is CI-guarded.** `gravity_ms`
  initializes from a match expression instead of a dead literal, and `win_open`
  uses the non-deprecated `set_target_fps(60)`. The tetris CI job now diffs the
  regenerated `Cargo.toml`, `.poly-generated`, `src/main.rs`, and a freshly
  resolved `Cargo.lock` against the committed files, so drift between
  `tetris.poly` and the committed project fails CI.

- **Grammar documentation includes `dep` declarations.**
  `POLY_GRAMMAR.md` gained the `dependency_declaration` production and its
  status reflects the current preview.

## [2.0.0-preview.7] - 2026-09-17

### Fixed

- **asm backend resolves program-scope structs and enums.** Layout and enum-
  tag registration happened during `_start` emission, after every function
  body had been emitted, so any struct/enum use inside `fn main` failed with
  "Unknown struct/enum". Registration now precedes codegen, and the struct
  ABI is coherent: struct arguments pass the struct's address, field access
  dereferences it, struct-returning functions copy all fields through
  caller-allocated space, and inferred declarations from struct literals or
  call results get struct-sized slots. `tests/asm_target_tests.poly` now
  emits working assembly (the snapshot had recorded the error).

### Changed

- **`dep` declarations warn on non-Rust targets** (`--check`, default build,
  and `--project` for C/asm/JS) instead of being silently dropped: `Warning:
  ignoring N 'dep' declaration(s): the <lang> target has no dependency
  support; 'dep' only applies to the Rust target`.

### Added

- External dependencies row in the support matrix; External Dependencies
  section in the README; commit-separation checklist item closed with
  recorded evidence.

## [2.0.0-preview.6] - 2026-09-17

### Added

- **`dep name = "version"` declarations** for external crate dependencies
  (program scope). `--project` emits them into the generated `Cargo.toml`,
  and `--check` plus a default Rust build resolve them through Cargo in a
  temporary project — programs using external crates now validate
  standalone. C, asm, and JS backends ignore the declarations. The Tetris
  example declares its `minifb`/`alsa` dependencies in source, replacing the
  manual Cargo.toml editing previously documented in its README and CI.
- Regression tests: parser acceptance/rejection of `dep` forms, manifest
  emission, and a CLI e2e test covering check + project generation with
  dependencies.

## [2.0.0-preview.5] - 2026-09-17

Re-cut of `2.0.0-preview.4` from a commit whose CI run is fully green (the
compiler code is unchanged; no source diff against `preview.4`). The original
`preview.4` tag predated the tetris CI job and two CI fixes, so its own CI run
was red while the release build itself succeeded.

### Added

- CI badge on the README tracking the `v2.0-dev` branch.

### Fixed

- The tetris CI job's regenerate step now removes the whole generated
  directory (`--project` refuses to touch an existing one) and restores the
  `minifb`/`alsa` dependencies that `--project` does not emit.

## [2.0.0-preview.4] - 2026-09-17

### Added

- **CI now runs on `v2.0-dev`** (push and pull requests) and the lint job
  audits **all** repository Poly documentation examples via
  `scripts/check_poly_examples.py --poly-bin` (previously only 10 curated
  files were checked). The audit script accepts `--poly-bin` / `$POLY_BIN` to
  use a prebuilt binary instead of always rebuilding the debug CLI.
- **CI builds and self-tests the Tetris example on Linux**: ALSA headers are
  installed, `rust_output/tetris` is regenerated from `tetris.poly` with the
  release compiler, the tracked `src/main.rs` is guarded against
  regeneration drift via an `--emit-rust` diff, and the headless `--test`
  self-test must pass.
- End-to-end CLI regression test `function_local_consts_work_across_backends`:
  a function-local `const` is compiled through JS (executed via Node, output
  asserted), Rust and C (emitted code asserted), plus `--check`.

### Fixed

- **Nested `while` loops now generate a real loop.** The parser encodes
  `while` as an if-expression without an else block; the top-level codegen
  path handled that, but `gen_expr`'s `Expr::If` arm and `gen_statement_str`'s
  `Statement::If` arm (used inside `if` bodies, `match` arms, and loop bodies)
  always emitted `if`. A `while` in any nested position therefore ran exactly
  once. Both paths now honor the while encoding, with regression tests for
  the `if`-nested and `match`-arm shapes. Found via the Tetris example, where
  a row-compaction loop inside an `if` silently kept only one cell per
  surviving row.

- **Nested `while` loops now generate a real loop.** The parser encodes
  `while` as an if-expression without an else block; the top-level codegen
  path handled that, but `gen_expr`'s `Expr::If` arm and `gen_statement_str`'s
  `Statement::If` arm (used inside `if` bodies, `match` arms, and loop bodies)
  always emitted `if`. A `while` in any nested position therefore ran exactly
  once. Both paths now honor the while encoding, with regression tests for
  the `if`-nested and `match`-arm shapes. Found via the Tetris example, where
  a row-compaction loop inside an `if` silently kept only one cell per
  surviving row.

### Fixed

- A `const` declared inside a function body no longer panics the IR generator
  (`unreachable: constants are collected first`); it lowers to an immutable
  local declaration. Program-scope constants are unaffected.
- Range loops whose start bound is a non-literal expression
  (`loop i BUF..TOTAL - 1`, `loop i TOTAL - 4..TOTAL - 1`, `loop i xs[0]..n`)
  now parse correctly; they were previously rejected with
  `Unexpected token: DotDot`. A leading literal in the loop-variable position
  (`loop 0..10`) is a var-less counted loop whose counter is discarded —
  previously it was silently mis-parsed as an infinite loop containing a bare
  range-expression statement.

### Documented

- Range-loop grammar: the start bound may be any expression, and the var-less
  `loop 0..10` form binds the counter to `_`. Stated in `POLY_SPEC_v2.md`,
  `POLY_GRAMMAR.md`, `POLY_CHEATSHEET.md`, and `POLY_QUICK_REFERENCE.md`.
- Reserved words `spawn` and `step` (previously undocumented in the maintained
  spec) and the take-and-return `impl` method pattern (`s := s.bump()`),
  including the `self.method(self.field, ...)` move-ordering pitfall.
- Strict same-type comparison rule: `=`/`!=` and ordering operators reject
  mixed numeric types (`u64 = i64`, `f64 = i64`); cast with `as`, and note
  untyped consts infer `i32` so assignment into other-width variables also
  needs an explicit cast. Stated in `POLY_SPEC_v2.md` and
  `POLY_CHEATSHEET.md`.

### Changed

- **Explicit `fn main() ... end fn` is now required.** Poly programs must
  declare their entry point; top-level executable statements (`var`, `put`,
  loops, assignments, expressions) are rejected at check time with a named
  error. Declaration statements (functions, structs, enums, traits, impls,
  modules, consts, aliases, `use`, foreign blocks, `extern <target> fn`
  declarations) remain top-level. The transpiler no longer synthesizes an
  implicit `main` from leftover top-level statements; generated Rust, C, asm,
  and JS output is unchanged for programs that already declared `fn main`.
  Enforcement is shared (`poly-parser/src/entry_point.rs`) across the checker,
  the unchecked transpile path, and the IR pipeline; the REPL wraps sessions
  without an explicit `fn main` in a synthetic one so interactive use is
  unchanged. All repository examples, fixtures, benches, doc examples, and
  playground samples were migrated, and 19 codegen snapshots were regenerated.

### Added

- **JavaScript target (`--target js`).** Poly now compiles the same
  orchestration subset as the C target to plain ES2020 JavaScript with no
  runtime dependencies, runnable under Node.js and in the browser.
  - New `poly-js-codegen` crate mirrors the C backend: scalars, arithmetic
    (integer `/` truncates via `Math.trunc`), `if`/`else`, `while`, inclusive
    numeric `loop` ranges with optional `step`, `for x in` / `loop x in`
    lowering to `for...of`, `match` to `switch`, `break`/`continue`,
    `put`/`error`/`warn`/`info` output, simple Poly functions, and verbatim
    `#js` foreign blocks emitted at module scope inside a generated
    `function main()`.
  - `extern js fn name(param: type): type` declarations are accepted by the
    parser and checked by the type checker; the checker's foreign-name
    heuristic already resolves JS-style `function name(...)` declarations, so
    `#js` calls type-check like C calls with no checker changes.
  - CLI: `--target js` and `--emit-js` (print to stdout); generated output is
    verified with `node --check` plus execution when Node is available.
  - `--project` generates a JS project directory with a `package.json`.
  - Tests: 7 `poly-js-codegen` unit tests, 7 new JS snapshot variants, and a
    `tests/js_target_tests.poly` fixture checked, emitted, `node --check`ed,
    and executed with output verification on the Linux CI job.
- **Strict mode adopted in CI.** Every CI `--check` step now passes `--strict`:
  examples and fixtures declare all their foreign calls with explicit
  `extern <target> fn ...` declarations (`tests/c_target_tests.poly`,
  `tests/target_flag_tests.poly`, and the `c_target_demo`,
  `comprehensive_demo`, and `poly_rust_hybrid` examples gained declarations).
- **Windows CI fix:** snapshot tests normalize line endings before comparing,
  fixing a pre-existing failure where `git` CRLF conversion broke byte-for-byte
  snapshot comparison on `windows-latest`.

### Fixed

- **CLI flags are now accepted in any position.** `poly file.poly --emit-rust`,
  `poly file.poly --check`, and every other flag previously worked only when
  the flag preceded the file (`poly --emit-rust file.poly`); a file-first
  invocation silently fell through to a default build. Arguments are now
  pre-parsed into flags plus positionals, so documented invocations work in
  any order, unknown flags are rejected in every position, and `--project`
  accepts its two operands either way around.
- **`--target js` now runs the generated program with Node** when a default
  build produces a `.js` file, matching the documented CLI behavior (the
  Rust target mirrors this by building the generated project). When Node is
  unavailable the emitted file is still verified with `node --check` and the
  build succeeds.
- **Retired-syntax diagnostics restored.** The `set x to y` detector matched
  an `Identifier("to")` token, but `to` lexes as a keyword, so the migration
  hint never fired. `set x to y`, `add x n`, `sub x n`, `inc x`, and `dec x`
  now all produce named errors with targeted suggestions, and `x += value` /
  `x -= value` (lexed as two tokens) get the same treatment. Assignments to
  variables that happen to be named `add` (`add := 5`) and calls like
  `inc(counter)` are still accepted.
- **Macros now expand in value position.** `var y := double(21)` previously
  failed with `unknown function` because expansion only ran for
  statement-position calls. A macro whose body is a single expression or
  `return expr` lowers to that expression; multi-statement macros still
  expand only at statement position, with a clear arity error in both
  positions.

### Documentation

- New `POLY_JS_BLOCKS.md` documents the JS target contract, supported surface,
  rejections, type mapping, and strict-mode usage.
- `POLY_V2_SUPPORT_MATRIX.md` gains a JS target column, the Language Surface
  table now carries consistent Rust/C/Asm/JS columns, and the verification
  contract includes the new doc.
- `POLY_JS_DESIGN.md` upgraded to an implementation-ready review: verified
  integration touchpoints table (lexer, parser, target dispatch, CLI, CI),
  strict-mode interaction, first-cut `#js` declaration requirements, and
  acceptance criteria.

## [2.0.0-preview.2] - 2026-09-08

Follow-up passes for the target-aware compiler model: a new assembly target,
a substantially widened C backend, and cross-target fixes.

### Added

- **Assembly target (`--target asm`, Linux x86-64).** Poly now compiles a
  subset of the language to freestanding x86-64 assembly with Linux syscalls
  and a `_start` entry point that calls `fn main` when it is declared.
  - `extern asm fn ...` declarations give the asm target a Poly-level way to
    source foreign values (previously extern declarations were stripped before
    the asm pipeline, so pure Poly code could never reference one).
  - `#asm` blocks are accepted as foreign passthrough sections selected by the
    asm target.
  - `Vec<T>` on the asm target: immutable literal vectors are static
    descriptor-backed data with index access and `for x in xs` iteration
    (integer elements print via `_print_int`, char elements via a 1-byte
    `sys_write`); `push`/`pop`/`len` switch to a growable in-place form backed
    by a static bump arena (64 KiB, doubling growth; exhaustion exits 42
    rather than corrupting memory).
  - Pointer dereference (`*ptr` via `movq (%rax)`) and `&T` locals as plain
    8-byte pointer slots.
  - `for x in some_string` iterates through a NUL-terminated pointer walk.
- **`pop()` language-wide.** `xs.pop()` type-checks as the vector's element
  type (an empty vector yields the default) and lowers to
  `pop().unwrap_or_default()` on the Rust target; the asm backend supports it
  in the growable runtime.
- **C backend expansion** (previously rejected, now compiled and verified
  end-to-end against the Rust target's output):
  - Tuple types compile to anonymous structs with `_N` fields; `.N` index
    access, nested tuples, and printing of tuple indexes all work.
  - `match` statements lower to an if/else-if chain over a match-scoped `const`
    copy of the scrutinee; literal, wildcard, range, and identifier arms are
    supported.
  - Struct literals lower to C99 compound literals (`Point { x: 3, y: 4 }` →
    `(Point){ .x = 3, .y = 4 }`), and `var p := Point { ... }` infers the
    struct type.
  - Enum declarations emit `typedef enum` with qualified enumerators
    (`Color_Red`), and `Color::Red` variants work in expressions and match
    patterns (payload-carrying variants are rejected with guidance).
  - Plain `get` (with optional prompt) reads a stdin line through an emitted
    runtime helper; the result participates in string concatenation output.
  - Non-capturing closures compile to a static function plus a typed function
    pointer; capturing closures are rejected with guidance.
- **Span-precise source maps.** The intermediate representation carries
  per-statement source locations, and `poly --source-map` reports exact
  statement-level line mappings instead of a line-identity heuristic.
- **LSP**: `textDocument/documentSymbol` reports UTF-16 ranges (previously
  byte offsets, which shifted on lines with non-ASCII text), and the JSON
  parser handles `\uD83D\uDE00`-style surrogate-pair escapes.
- CI smoke-tests the rebuilt playground wasm artifact with
  `scripts/playground_wasm_smoke.mjs` (five valid programs must transpile to
  the expected Rust symbols; six legacy-syntax programs must be rejected), and
  a `tests/extern_asm_deref.poly` job checks, compiles, runs, and verifies asm
  target output on Linux.
- Pinned development and CI builds to Rust 1.98.0 and refreshed the checked-in
  playground WASM artifact for reproducible releases.
- Added the frozen [Rust/C v2 support matrix](POLY_V2_SUPPORT_MATRIX.md),
  [preview release checklist](POLY_PREVIEW_RELEASE_CHECKLIST.md), and
  [preview release notes](POLY_V2_PREVIEW_RELEASE_NOTES.md).
- Added target-aware `extern rust fn ...` and `extern c fn ...` declarations.
  They are optional checker-only interface contracts: opaque legacy foreign
  calls remain compatible, while explicit declarations validate Poly-visible
  argument count, argument types, and return types.
- Added cross-platform Rust/C CI for Linux, macOS, and Windows. The CLI honors
  `POLY_CC`, with documented platform compiler fallbacks.

### Changed

- **Operator keywords**: logical, bitwise, and remainder operators are now
  spelled as keywords — `and`, `or`, `not`, `xor`, `mod`, `bitand`, `bitor`,
  `bitnot`, and `shift left` / `shift right`. The retired symbol spellings
  `&&`, `||`, `!`, `^`, `%`, `&`, `|`, and `~` are tokenized only to produce
  migration diagnostics ("use `and`" etc.), mirroring the `==` rejection.
  `<<` and `>>` remain valid alternative shift syntax, and `left`/`right`
  stay usable as ordinary identifiers. All examples, fixtures, and tests
  were migrated to the keyword spellings.
- **Playground**: rewrote the embedded JS demo transpiler and example programs
  for the keyword-operator dialect. The demo now maps the keyword operators
  onto the Rust symbol operators and lowers Poly `=` equality to Rust `==`
  inside expressions; the legacy `+=` handler was removed, assignment now
  supports tuple/field paths (`pair.0 := 99`), and `for` supports destructured
  patterns. All nine playground examples were rewritten to syntax that passes
  the real compiler, and the checked-in `playground/poly.wasm` was rebuilt
  from the current compiler so the browser engine matches the CLI.
- **VS Code extension**: the TextMate grammar now highlights the keyword
  operators and the retained `<<`/`>>` shift alternatives; retired symbol
  operators (`&&`, `||`, `!`, `^`, `%`, `&`, `|`, `==`) and compound
  assignments (`+=`, `%=`, `&&=`, …) are no longer highlighted as valid.

### Performance

- Optimized IR codegen output path: pre-allocated buffer, direct indent
  writing (eliminated `String::repeat()` per line), `writeln_fmt` to avoid
  intermediate `format!()` allocations, and loop body pre-allocation.

### Fixed

- `poly --target c|asm <file> -o <path>` now honors the requested output path
  instead of silently writing next to the source file.
- The asm backend sized function frames from one slot per statement, but
  expression codegen allocates temporaries during emission, so programs with
  many temporaries wrote locals below `%rsp` where call frames clobbered them
  (fixtures printed garbage). Bodies are now emitted into a scratch buffer and
  the prologue is sized from the final frame.
- `#asm` blocks left the assembler in an arbitrary section, so data emitted
  afterwards landed in read-only `.text` (DT_TEXTREL + segfault);
  `emit_string_data`/`emit_vector_data` re-establish `.section .data`.
- `_start` now actually calls `fn main` on the asm target (top-level `fn main`
  bodies were previously emitted but unreachable).
- The C backend emitted `void main()` for `fn main` alongside the generated
  `int main(void)` wrapper, so any C program using `fn main` had two main
  definitions; `fn main`'s body is now spliced into the entry point.
- Corrected `poly --help` inaccuracies around `--emit-asm` and `--target`.
- Removed the unfinished `process_config` builtin lowering that silently
  generated `()`; unknown calls now fail through the normal checker path.
- Removed the generated `compiler/output.txt` artifact from the working tree.

### Documentation

- Modernized seven v1-era guides' example blocks to the live keyword-operator
  syntax and refreshed the release PR evidence to branch-tip verification
  results.

## [2.0.0-preview.1] - 2026-08-22

### Added

- Foreign blocks now use a language-tagged `ForeignBlock` representation for
  `#rust`, `#c`, and reserved `#cpp` syntax.
- Foreign block delimiters are line-oriented and foreign blocks are rejected
  inside Poly functions, loops, and modules.
- Added the first C backend with `--target c`, `--emit-c`, C syntax checking,
  native C builds, and a C project generator.
- Target selection ignores foreign blocks for other backends, allowing one
  source file to carry Rust and C definitions side by side.
- Added lexer, parser, backend, and end-to-end C target regression coverage.

### Notes

- Foreign function signatures remain opaque to Poly and are validated by the
  selected native compiler.
- C++ block syntax is reserved but has no backend yet.

## [1.7.8] - 2026-08-17

### Added

- **Vector `clear()` and `reserve(n)`**: both type-check and lower to the
  Rust `Vec` methods, matching the performance guide's pre-allocation and
  buffer-reuse examples (which now compile standalone).
- **Iterator chains via `xs.iter()`**: `xs.iter()` yields owned elements
  (lowering to `iter().cloned()`) so `sum()`, `map(f)`, `filter(p)`,
  `collect()`, `enumerate()`, and `loop`/`for` bindings all see the element
  type `T`. The checker tracks an internal iterator type (rejecting `sum` on
  non-numeric elements and non-callback arguments), and codegen annotates
  `.sum::<T>()`/`collect::<Vec<_>>()` so bindings need no explicit type.
- **String append/mutation**: `add s, "text"` and `s += "text"` now type-check
  and compile for string targets (lowering to Rust `String += &str`), matching
  the documentation's canonical string-building syntax. The amount is
  borrowed only for string targets, so numeric `+=` is unchanged.
- **`.repeat(n)` on strings** and **`.join(sep)` on vectors** are now real
  methods (both documented in the language reference): `"#".repeat(5)` and
  `["a", "b"].join(", ")` type-check and lower to the Rust equivalents.
- **Collection loops bind owned values**: `loop x in items` and
  `for x in items` lower to `items.iter().cloned()` (and
  `.iter().cloned().enumerate()` for the enumerate form), so the loop
  variable has the element type `T` the checker declares instead of `&T`.
  The collection is still borrowed and stays usable after the loop.
- **String/vec/type registration inside loop and match bodies**: variables
  declared in bodies rendered through `gen_statement_str` are now tracked,
  so `add row, ...` on a string declared in a nested loop borrows its amount
  and `put buf` on a vector declared in a loop body prints with `{:?}`.

### Documentation

- **Full doc audit**: every fenced Poly block across all Markdown guides now
  parses; the 192 complete (non-fragment) blocks type-check and their
  generated Rust compiles, and 346 context-dependent teaching snippets are
  marked `poly fragment` per the documentation style guide.
- **`scripts/check_poly_examples.py` strengthened** to run `--check` (parse +
  type-check + Rust compile) on complete blocks instead of parsing only, so
  wrong method names and type errors in the guides fail CI.

## [1.8.0] - 2026-08-20

### Changed

- **Breaking syntax changes** for readability:
  - `loop: i 0..10` → `loop i 0..10` (colon removed)
  - `add x, 5` / `sub x, 3` / `inc x` / `dec x` → `x := x + 5` etc. (removed)
  - `set x to y` → `x := y` (removed)
  - `put expr > "file"` → `put expr to "file"` (write)
  - `put expr >> "file"` → `put expr to "file" -append` (append)
  - `get < "file"` → `get from "file"` (read)
  - Compound mutation operators (`+=`, `-=`, etc.) removed

### Added

- Migration hints: compiler suggests new syntax when old syntax is detected

## [1.7.7] - 2026-08-16

### Added

- **Tuple element assignment**: `pair.0 := 99`, `pair.1 = false`, and
  `add pair.0` now mutate tuple elements in place (assignment targets accept
  `.N`, and the checker validates the assigned type against the element type).
- **`for` loop tuple destructuring**: `for (idx, val) in items.enumerate()`
  binds the destructured names with the matching component types, borrowing
  the receiver so the collection stays usable afterwards.
- **Fixed `for x in collection.enumerate()` codegen**: previously lowered to
  the invalid `collection.enumerate().iter()`; now borrows like the `loop`
  form (`collection.iter().enumerate()`). The playground gains a Tuples
  example covering index access, destructuring, and element assignment.

## [1.7.6] - 2026-08-16

### Added

- **Tuple index access**: elements can be read by position with `pair.0`, and
  nested tuples chain as `nested.1.0`. The lexer is context-sensitive after a
  `.` (like Rust), so `1.0` in `t.1.0` lexes as `1`, `.`, `0` instead of a
  float; the checker resolves the element type (rejecting out-of-bounds
  indexes and non-tuple receivers), and codegen emits the plain Rust form.
  Covers the parser (postfix `.N`), AST/IR (`TupleIndex`), checker, optimizer,
  and adds lexer/parser/checker/runtime tests.

## [1.7.5] - 2026-08-16

Follow-up pass: remaining input stubs implemented, a mini test framework, string
interpolation, checked indexing, and a dependency audit in CI.

### Added

- **`get --until <delimiter>` is implemented**: reads stdin until the delimiter string is found (or EOF) and returns everything before it, so `get --until unicode ","` on `apple,banana` yields `apple`. The flag no longer warns.
- **`get --mask <char>` is implemented**: suppresses terminal echo while reading (best-effort `stty -echo` on Unix terminals) so passwords are not shown; on non-Unix platforms or non-terminal stdin it falls back to a plain read. The flag no longer warns.
- **Mini test framework**: `assert(cond)` (panics when the condition is false), `pass(msg)` (prints `[PASS] msg`), and `fail(msg)` (prints `[FAIL] msg` and exits with code 1) type-check and codegen as real builtins, matching the testing guide's documented API.
- **Checked (non-panicking) indexing**: `xs.get(i)` on a vector returns `Option<T>` and `"text".get(i)` returns `Option<char>`, so out-of-bounds indexes yield `None` instead of panicking. `put` of such values prints with `{:?}`.
- **`Result` accessors**: `is_ok()`/`is_error()`/`unwrap()`/`unwrap_or(v)` type-check and lower to Rust's `is_ok()`/`is_err()`/`unwrap()`/`unwrap_or()`, completing the test-framework story for `http_get`/`tcp_connect` results.
- **String interpolation**: `"Name: {name}, Age: {age}"` parses into a concatenation chain, so embedded expressions (strings, numbers, booleans, characters) are formatted in place. Braces that do not form a parseable expression (e.g. JSON-like text) stay literal.
- **`db_execute(query)` runs real SQL against a shared in-memory SQLite database**: the checker types it as `Vec<String>` and the CLI adds `rusqlite` to generated projects when the builtin is used, so `CREATE TABLE`/`INSERT`/`SELECT` flows persist across calls. It no longer warns. Requires `--project`/Cargo (a standalone rustc build lacks rusqlite).
- **Example and tooling coverage**: `examples/testing_and_interpolation.poly` demonstrates interpolation, checked indexing, and the test helpers; the playground gains an "Interpolation & Testing" example; the VS Code TextMate grammar highlights `{expr}` interpolation inside `"..."` and `unicode "..."` strings.
- **CI dependency audit**: a new `audit` job runs `cargo audit` against the workspace lockfile.

## [1.7.4] - 2026-08-16

Follow-up pass: runtime execution tests, real network builtins, and generic
substitution at call sites.

### Added

- **`http_get(url)` performs a real HTTP/1.1 GET** over a tokio TCP connection: the URL is parsed into host/port/path, a raw request is written, and the response body is returned as `Ok(body)` with connection/write/read failures surfaced as `Err`. `tcp_connect(addr)` establishes a real connection and returns the peer address. `spawn <expr>` lowers to `tokio::spawn`, running the expression as a background task. The `--mask`/`--until` flags are implemented by the Rust backend.
- **Generic substitution at call sites**: a call to a generic function now unifies the declared parameter types against the concrete argument types and substitutes the bindings into the return type — `identity(42)` yields `i32` instead of an opaque `Unknown`, so `var s ustring := identity(42)` is correctly rejected with `expected String, got i32`. Substitution threads through containers (`Vec<T>`, `Map<K, V>`, `Set<T>`, `Option`, `Result`, tuples, references) and generic bodies stay permissive.
- **Runtime execution tests**: the integration suite now compiles *and runs* generated programs — line-by-line file reads, negative loop steps, `get`/`get --as` with piped stdin, Map round-trips, and a tokio `spawn` concurrency check.

### Fixed

- **`get` trims stdin**: plain `get` and `get --as <type>` strip the trailing newline `read_line` leaves behind, fixing `put name + "!"` printing on two lines and `get --as i32` silently parsing `"21\n"` as `0`.

## [1.7.3] - 2026-08-16

Audit pass fixing optimizer soundness, runtime stubs, and checker holes.

### Fixed

- **Optimizer soundness**: functions returning closures are no longer inlined (previously the optimized `--ir` path emitted `|x| (x + n)` with a dangling capture); integer comparisons now fold to `bool` literals instead of `int` (`let flag: bool = 1;` no longer generated); logical `not` folds only over booleans while `~` keeps bitwise semantics. The CLI `--ir` path now verifies that the optimized Rust actually compiles before printing it.
- **Runtime APIs**: `open` lowers to a `BufReader` and `get_line`/`eof` are implemented (`while not f.eof()` no longer loops forever, `get_line` no longer returns an empty string). `sleep` lowers to `std::thread::sleep` and `delay` to `tokio::time::sleep`. `exit(n)` lowers to `std::process::exit`. `get --timeout`/`--default`/`--as`/`--bytes` are implemented (real timeout via channel + reader thread, typed parsing, bounded raw reads) and `get` now trims the trailing newline from stdin. `http_get` performs a real HTTP/1.1 GET over a tokio TCP connection (host/port/path parsing, raw request, response body returned as `Ok`/`Err`); `tcp_connect` establishes a real TCP connection; `spawn <expr>` runs an async expression as a background `tokio::spawn` task; `--mask`/`--until` are implemented by the Rust backend.
- **Runtime execution tests**: the integration suite now compiles *and runs* generated programs (file reads via `get_line`/`eof`, negative loop steps, `get`/`get --as` with piped stdin, Map round-trips, and a tokio-based `spawn` concurrency check), which would have caught the stub bugs this audit fixed.
- **Match patterns are validated**: a misspelled enum variant (bare or with payload) or an unknown name on a `Result` scrutinee is rejected instead of silently compiling into an irrefutable Rust binding that swallows every other value.
- **Range patterns work**: `match n ... 0..=9` no longer fails `--check` with a bogus `expected i32, got Vec<i32>` error.
- **`Map`/`Set` methods implemented**: `insert`/`get`/`remove`/`contains_key`/`contains`/`len`/`is_empty` type-check and codegen borrows keys correctly; `m[key]` reads lower to `get().map(clone).unwrap_or_default()` and `m[key] = v` lowers to `insert`. Keyword method names (`map.get(...)`) now parse.
- **Stored-then-returned closures type-check**: `var f := |x| x + n; return f` (the v1.7.2 example) now passes `--check` — `_` placeholders behave like unknown types and arithmetic on unknown operands is permissive until a signature pins types down.
- **Loop steps**: negative steps keep their magnitude (`10..1 step -2` iterates `10, 8, 6, 4, 2`), descending ranges with positive steps are empty, and a constant `step 0` is rejected.
- **LSP**: hover/completion positions are UTF-16-accurate on lines with non-ASCII text, and the JSON parser handles `\uD83D\uDE00`-style surrogate-pair escapes.
- **Version metadata synced**: README and playground advertise v1.7.x instead of v1.6.0.

### Infrastructure

- `performance_benchmarks.rs` updated to comma match syntax; CI now runs `cargo check --workspace --all-targets` (the benchmark regression was previously invisible), checks all examples with `poly --check`, and rebuilds/verifies the checked-in `playground/poly.wasm`.

## [1.7.2] - 2026-08-13

### Added

- Functions can now return closures through function-typed return annotations: `fn make_adder(n: i32): |x: i32| i32`. The returned closure literal inherits the declared signature (untyped parameters pick up the declared types), and codegen lowers the return type to `impl Fn(i32) -> i32` and marks returned closures `move` so they may capture locals. The result can be stored (`var add5 := make_adder(5)`) and called (`add5(10)`, or `make_adder(100)(1)`).
- Vec higher-order methods `map`, `filter`, and `reduce`: `xs.map(|x| x * 2)`, `xs.filter(|x| x % 2 == 0)`, `xs.reduce(0, |acc, x| acc + x)`. The checker validates callbacks against the element (and accumulator) types — non-function arguments, wrong parameter counts, and non-`bool` filter predicates are rejected — and codegen emits `iter().cloned().map(...)`, `iter().cloned().filter(...)`, and `iter().cloned().fold(...)` so the receiver stays usable.
- `map`/`filter`/`reduce` also work on strings, iterating Unicode characters (`"hello".map(|c| c)` yields `Vec<char>`), and vectors gained `sort_by`: `xs.sort_by(|a, b| a > b)` returns a sorted copy (a boolean comparator is lowered to Rust `Ordering`).
- `put` of vector-like values (arrays, vector variables, and `map`/`filter`/`sort_by` results) now prints with `{:?}` debug formatting instead of failing to compile, e.g. `put xs.map(|x| x * 2)` prints `[2, 4, 6, 8, 10]`. Vector values written to files are formatted the same way.
- Added `examples/higher_order_functions.poly` and a playground example covering function-typed parameters, returned closures, `map`/`filter`/`reduce`/`sort_by`, and vector printing.
- Match arms now use comma syntax (`pattern, expression`) instead of `=>`, while generated Rust continues to use `=>` internally.
- Range and collection loops now require an explicit binding: `loop value 1..3, 7, 19..20` and `loop item in items`; implicit `i`, `j`, and collection-name-derived bindings are no longer generated. Loop-range endpoints are inclusive, so the range example visits `1, 2, 3, 7, 19, 20` and lowers to inclusive Rust ranges.

### Fixed

- Closures stored in a variable and then returned now capture with `move`, so `var f := |x| x + n; return f` no longer generates a borrow that outlives the function.

## [1.7.1] - 2026-08-13

### Added

- Closure type annotations in parameter positions: `fn apply(f: |x: i32| i32, v: i32): i32`. The parser accepts `|param: type, ...| return_type` (parameter names optional, e.g. `|i32| i32`) as a `TypeAnnotation::Function`, and codegen renders it as `fn(i32) -> i32` in Rust.
- The type checker now checks closure literals against the expected function type: an untyped literal passed to a function-typed parameter inherits the declared parameter types, so `apply(|x| x * 2, 21)` type-checks and mismatched return types or arities are rejected with `expected fn(i32) -> i32, got ...`.
- Function-type compatibility in the checker: `PolyType::Function` values now compare structurally (parameter types and return type), so typed closures stored in variables can be passed to function-typed parameters.

### Fixed

- The type checker already inferred `for x in xs` element types for `Vec<T>` and `String` iterables; this is now covered by explicit tests and documented as supported (previously listed as a known limitation).

## [1.7.0] - 2026-08-13

### Fixed

- `break`/`continue` now work inside `while` loops (the type checker only registered loop depth for `for` and `loop` ranges, so valid programs were rejected).
- Nested `fn` declarations can now be called from the enclosing function; the checker previously reported `unknown function` even though codegen emitted valid Rust.
- Builtin `Option`/`Some`/`None` now type-check: `Some(42)`, bare `None`, and `Some(v)`/`None` match patterns against `Option<T>` scrutinees (mismatches still error).
- `Map<K,V>` and `Set<T>` declarations with an empty `[]` initializer now generate `HashMap::new()`/`HashSet::new()` instead of `vec![]`, so the documented `var m Map<ustring, i32> := []` compiles.
- REPL multi-line input works: block statements (`fn`, `if`, `while`, `for`, `loop`, `match`, `struct`, `enum`, `trait`, `impl`) buffer until the matching `end <keyword>`, and trailing-`\` continuations are stripped before parsing.
- `poly --check` now verifies async programs by checking `#[tokio::main]` output in a temporary Cargo project with tokio declared, so `examples/async_await.poly` passes.
- Workspace version bumped to 1.7.0 (CLI/REPL previously reported 1.6.0 while the changelog documented 1.7.0).

### Added

- Added rustc-style source diagnostics: lexer and parser errors now render with 1-based line/column positions, the offending source line, and a caret (`poly --check`, `--tokens`, `--ast`).
- Added a `--source-map` CLI flag that prints the generated Poly→Rust source map (line mappings, symbols, VLQ summary).
- Added a `poly-lsp` crate: a dependency-free JSON-RPC language server over stdio with publishDiagnostics, completion, hover, and document symbols. Documented in POLY_INTEGRATION_GUIDE.md with VS Code and Neovim setup snippets.
- Made the AST span-precise: top-level statements are wrapped in `Spanned<Statement>`, so source maps report exact symbol positions and LSP type-check diagnostics point at the offending statement instead of scanning source text.
- Upgraded the REPL to a stateful session: variables and functions persist across lines, the full session is re-transpiled on each submission, and a raw-mode line editor on Unix terminals provides Tab completion, arrow-key history navigation, and cursor movement. Added `:vars`, `:reset`, and `:clear-history` commands.
- Added deterministic fuzz/property tests for the lexer, parser error-recovery path, and the full transpile pipeline (seeded PRNG, no external dependencies).
- Upgraded the web playground to the v1.6.0 dialect with a faithful client-side demo transpiler, shareable code links, and a live status bar.
- Added a `poly-wasm` crate: a dependency-free WebAssembly build of the real transpiler (exports `poly_alloc`, `poly_free`, `transpile`, `result_*`, and `free_result`), bundled as `playground/poly.wasm`. The playground runs the real compiler in the browser and falls back to the demo transpiler when WebAssembly is unavailable. All six playground examples are `--check`-valid.
- Added `compiler/scripts/build_playground_wasm.sh` to rebuild the playground wasm bundle (requires the `wasm32-unknown-unknown` target).
- Added real `for variable in iterable` loops end-to-end: a `ForLoop` AST node (the parser previously discarded the body), an intermediate-representation variant, code generation that emits idiomatic `for` loops in Rust, and type-checking support.
- Extended the type checker to accept `Type::method` associated-function calls (with correct arity handling for `self` receivers) and calling closures stored in variables.
- Wrapped every nested statement block (function bodies, if/match/loop blocks, module bodies, unsafe blocks) in `Spanned<Statement>`, so byte spans are available throughout the AST.
- Added a VS Code extension in `vscode/`: a TextMate grammar (`source.poly`), language configuration, and a language-client extension that launches `poly-lsp` with diagnostics, completion, hover, and document symbols.
- Shrunk the playground wasm bundle from 551KB to 328KB with a size-optimized release profile (`opt-level = z`, LTO, `codegen-units = 1`, strip) plus `panic = "abort"` for the wasm target only.
- Upgraded the playground examples to exercise the newly supported constructs: `for` loops, first-class closures that are called, and structs with `impl` methods and associated functions.

### Changed

- Retired the duplicate AST-walking code generator: the classic transpiler now routes through the shared intermediate-representation pipeline, so every backend uses one code generator and default output benefits from optimization passes.
- Added nested-function support to the intermediate representation (generator, codegen, and optimizer) so `fn` declarations inside function bodies emit valid Rust.
- Fixed the intermediate-representation generator panicking on nested function declarations.
- Updated benchmark sources to the current declaration syntax (`var name := value`).

## [1.6.0] - 2026-08-12

### Added

- Added comma-based `if` syntax, including single-line conditions, else-if chains, and nested conditions.
- Added generic type parameters to functions, structs, and trait bounds: `fn identity<T>(value: T): T` and `struct Wrapper<T>`.
- Added proper generic type parsing: `Box<i32>`, `Rc<T>`, `Arc<T>`, `Map<K, V>`, and `Set<T>` annotations now preserve their arguments instead of discarding them.
- Added `Map<K, V>` → `std::collections::HashMap`, `Set<T>` → `std::collections::HashSet`, `Rc<T>` → `std::rc::Rc`, and `Arc<T>` → `std::sync::Arc` type mappings in both transpiler backends.
- Added compile-time macros: `macro swap_values(a, b) ... end macro` expands with hygienic parameter substitution.
- Added standard library math functions: `abs`, `sqrt`, `pow`, `min`, and `max`.
- Added string methods with correct `&str` argument lowering: `contains`, `split`, `starts_with`, `ends_with`, and `replace`.
- Added intermediate representation pipeline benchmarks to `performance_benchmarks.rs`.
- Added examples and documentation covering the v1.6.0 syntax.

### Changed

- Renamed the `IR` pipeline and its `poly-ir` crate to the full name `intermediate representation` (`poly-intermediate-representation` crate, `--intermediate-representation` CLI flag). The short `--ir` flag remains as an alias.
- Improved else-if chain parsing and Rust code generation.
- Intermediate representation code generator now matches the classic backend for `get` lowering, loop variable naming, boxed pattern guards, await statements, and negative step ranges.
- Added documentation and Poly-example validation checks to CI.
- Removed compiler warnings and maintained a warning-free Clippy build.

## [1.5.0] - 2026-08-08

### Added

#### CLI Features
- Added `--format` flag to format output with rustfmt
- Added `--diff` flag to show diff between unformatted and formatted code
- Added `--watch` flag to watch file and re-transpile on changes
- Added `--check` flag to validate code and verify Rust compilation

#### Transpiler Improvements
- Added type mapping for Poly types to Rust types:
  - `ustring` / `string` → `String`
  - `uchar` → `char`
  - `byte` → `u8`
  - `bytes` → `Vec<u8>`
- Added file write operations (`put "content" to "file"`)
- Added file append operations (`put "content" to "file" -append`)
- Added `split()` method support with `.collect::<Vec<_>>()`
- Added `open()` function support for file operations
- Added placeholder support for `eof()` and `get_line()` methods
- Fixed loop range variable naming for collection iteration
- Fixed match arm codegen to remove extra semicolons
- Fixed semicolons in `gen_statement_str` for statements in inline contexts

#### Examples
- Added `structs_enums.poly` example for struct and enum usage
- Added `closures.poly` example for closure syntax
- Added `pattern_matching.poly` example for match expressions
- Fixed `prime_numbers.poly` to use correct loop variable (`i` instead of `num`)

#### Documentation
- Updated README with new CLI options
- Created `.freebuff-prefs.md` for user preferences

### Fixed
- Fixed `break` statement generating `/* statement */` instead of `break;`
- Fixed `return` statements missing semicolons in generated Rust code
- Fixed `put` statements missing semicolons in inline contexts
- Fixed file write operations not being transpiled correctly
- Fixed string type mapping (`ustring` → `String`)
- Fixed loop variable naming for collection iteration

### Testing
- Added unit tests for file write operations
- Added unit tests for type mapping
- Added unit tests for split with collect
- Added comprehensive test suite for all example files
- All 84+ tests passing

## [1.4.0] - 2026-08-07

### Added
- Initial implementation of the Poly language compiler
- Lexer, parser, transpiler, and CLI
- Basic I/O operations (put, get)
- Error handling with Result types
- Loop ranges (SuperBASIC-inspired)
- Struct and enum support
- Match expressions
- Closures

### Fixed
- Various parser and lexer issues
- Error recovery improvements
- Integration tests

---

*This changelog follows the [Keep a Changelog](https://keepachangelog.com/) format.*
