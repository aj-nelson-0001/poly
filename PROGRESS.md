# Poly — Session Progress

Working log of improvements made to the Poly compiler, playground, and tooling.
Last updated: 2026-09-06.

## Codegen optimizations and project audit (2026-09-06)

Session audit of the full project: build status, test coverage, clippy
warnings, missing features, documentation bugs, and codegen performance
optimizations.

### Follow-up resolution (2026-09-07)

- **Clippy strict CI restored**: fixed all 8 warnings — three
  `manual_div_ceil` (`.div_ceil(8)` for slot rounding), two `collapsible_match`
  (collapsed nested `if let` into outer patterns) in `poly-asm-codegen`, and
  three in `snapshot_tests.rs` (two unused imports, one needless borrow).
  `cargo clippy --workspace --all-targets -- -D warnings` now passes.
- **Tutorial `with validate` corrected**: the three tutorial presentations
  (input section, complete example, summary) now use a loop-based validation
  pattern with a note that `with validate` is a parser-only compatibility form
  per the v2 spec. The replacement examples were verified with `--check` and a
  runtime run (valid input, re-prompt on invalid, masked password re-prompt).
- **Doc audit note**: `scripts/check_poly_examples.py` reports 43 pre-existing
  unmarked failures in historical v1 docs (verified identical before and after
  these changes; the tutorial itself has 0 failures). `check_markdown.py`
  failures come from the two untracked report files using triple-backtick
  fences.
- Verified: 383/383 tests pass, snapshot tests byte-identical, strict clippy
  clean.

### Project audit

- **Build**: 0 errors, 383 tests pass, 0 failures.
- **Clippy**: 8 warnings total (5 in poly-asm-codegen, 3 in poly-transpiler
test). Would fail strict CI (`-D warnings`).
- **Missing features**: JS backend (design only), C++ backend (reserved),
C backend gaps (stdin, closures, tuples, pattern matching), ASM backend
gaps (for-in loops, dereferencing).
- **Documentation bug**: `POLY_TUTORIAL.md` documents `with validate` as a
working feature, but it is a parser-only stub — the validation closure is
never executed at runtime. The v2 spec explicitly states this is "not part
of the maintained runnable v2 API."

### Codegen optimizations (poly-intermediate-representation)

All changes internal to `codegen.rs`. Poly language and generated output
are unchanged. 383 tests pass including snapshot tests.

1. **Output buffer pre-allocation**: `String::with_capacity(4096)` instead
of `String::new()` — avoids ~log₂(output_size) reallocations.
2. **Indent writing**: Write indent characters directly via `push_str("
    ")` in a loop instead of `String::repeat(self.indent)` — eliminates
one allocation per output line.
3. **`writeln_fmt` method**: Writes indent + formatted args directly to the
output buffer, avoiding the intermediate `String` from `format!()`. Applied
to all hot paths: VarDecl, LetDecl, Assignment, Mutation, Return, Put,
Error/Warn/Info, If/While/Match, gen_put, gen_function, gen_struct,
gen_enum.
4. **`gen_loop_range` optimization**: Uses `writeln!` directly on the result
string instead of `push_str(&format!(...))`. Pre-allocates
`String::with_capacity(128)` for loop bodies.
5. **Boolean literal optimization**: Returns `&str` constants for `true`/
`false` instead of `value.to_string()`.
6. **Identifier lookup optimization**: Avoids unnecessary `.cloned()` in the
common (non-enum) case.

### Verification

- `cargo test --workspace`: 383 passed, 0 failed.
- `cargo clippy -p poly-intermediate-representation --all-targets`: clean
(no new warnings).
- Snapshot tests: all pass (byte-for-byte output verification).

## v2 audit fixes, snapshot tests, and JS design (2026-08-30)

Session audit of the asm/c codegen, checker, and CLI; documentation
cross-check; snapshot-test hardening; and a JS-target design proposal
(implementation on hold per review decision).

### Bugs found and fixed (asm backend + CLI)

- **String variables printed as addresses.** The asm backend routed string
  variables through the integer print path (`_print_int`); string variables
  hold pointers, so `put name` printed an address. They now use the
  strlen-based sys_write path, with `var_types` tracking for
  struct/enum/string locals. Regression test `supports_string_variable` added.
- **Function frame size computed too early.** Locals were gathered after the
  `subq` stack reservation was emitted, undersizing the frame for functions
  with many variables (stack corruption). Now two-phase: collect all locals,
  then reserve the frame.
- **Missing trailing newline** in asm output (POSIX text-file convention).
- **CLI help inaccuracies** in `poly-cli`: `--emit-asm`/`--target`
  descriptions corrected and rust-only flags (`--ir`, `--source-map`,
  `--format`, `--diff`) labeled "rust target only".

### Documentation cross-check

- `POLY_V2_SUPPORT_MATRIX.md` and CLI docs cross-checked against actual
  behavior; flag descriptions now match the implemented CLI.
- Roadmap, changelog, and C-blocks docs verified current.

### Snapshot-test hardening

- Added `compiler/crates/poly-transpiler/tests/snapshot_tests.rs` with 26
  committed snapshots in `tests/snapshots/`: 18 target×sample (rust/c/asm),
  6 IR-pipeline, and the two target fixtures (`asm_target_tests.poly`,
  `c_target_tests.poly`).
- Byte-for-byte comparison; unintended codegen changes fail CI. Regenerate
  with `POLY_UPDATE_SNAPSHOTS=1 cargo test -p poly-transpiler --test
  snapshot_tests`.
- Verified the assertions catch real regressions: a one-token C-backend edit
  tripped two snapshots; reverting went green.

### Verification

- `cargo test --workspace`: 383 passed, 0 failed.
- `cargo clippy -p poly-asm-codegen --all-targets`: clean.
- `cargo fmt --all`: applied and verified.
- End-to-end smoke: comprehensive_demo → rust (built+ran), c_target_demo → c
  (binary), asm_target_tests → asm (.S generated).

### JS target design (on hold)

- Added `POLY_JS_DESIGN.md` as a proposal for review. Implementation is on
  hold per review decision.

## v2 preview contract and release hardening (2026-08-23)

Implemented the recommended next hardening steps in order.

### Contract and interface

- Added `POLY_V2_SUPPORT_MATRIX.md` as the frozen Rust/C target contract.
- Added optional top-level `extern rust fn ...` and `extern c fn ...`
  declarations. They are checker-only metadata and provide Poly-side arity,
  argument-type, and return-type checks while native compilers remain
  authoritative for foreign definitions and ABIs.
- Preserved opaque foreign calls for compatibility and target-filtered both
  foreign blocks and explicit declarations before checking/codegen.

### Incomplete behavior audit

- Removed the unfinished `process_config` builtin from checker and Rust
  lowering. It now fails as an unknown function instead of silently emitting
  `()`.
- Added regression coverage for the rejected compatibility path and explicit
  foreign signature checking.
- Removed the generated `compiler/output.txt` artifact.

### Cross-platform validation

- Expanded CI Rust/C validation to Ubuntu, macOS, and Windows.
- Added `POLY_CC` compiler selection with platform defaults and Windows MSYS2
  GCC setup.
- Made generated C project and integration-test executable paths honor the
  platform executable suffix.

### Release documentation

- Added `POLY_PREVIEW_RELEASE_CHECKLIST.md`.
- Updated the v2 spec, grammar, API reference, quick references, migration,
  troubleshooting, C backend guide, audit, roadmap, README, and changelog.
- Maintained v2 documentation audit now includes the support matrix and release
  checklist.

## v2 documentation and source-comment audit (2026-08-22)

Completed the repository-wide documentation and source-comment pass for the
v2 preview. The maintained documentation now reflects the verified compiler
contract, while older v1 guides and release-era examples are explicitly
historical rather than normative.

### Documentation changes

- Added `POLY_DOCUMENTATION_INDEX.md` as the map of current, historical, and
  verification documents.
- Rewrote the maintained API reference, quick reference, cheatsheet, and
  troubleshooting guide around the implemented v2 behavior.
- Corrected current syntax to use `:=` for declarations/assignment and `=` for
  equality; legacy `==` is rejected with a migration diagnostic.
- Corrected append output to `put value to "file" -append`.
- Documented Rust as the default target and the C11 orchestration subset under
  `--target c`; documented explicit `#cpp` rejection.
- Clarified that `#rust` and `#c` blocks are top-level opaque target-language
  definitions and that unsupported target features should use foreign helpers.
- Marked historical v1 documentation and release material so obsolete syntax
  remains available for migration without presenting it as current behavior.

### Source comments and small correctness fixes

- Added why/how comments around AST spans, parser recovery, migration
  diagnostics, lexer errors, source maps, target-neutral types, JSON-RPC
  parsing, C code generation, and CLI target selection.
- Corrected stale AST/parser comments and initializer/redirect suggestions.
- Preserved native Rust operators inside opaque `#rust` fixtures.

### Verification

- `cargo test --workspace -- --test-threads=1`: passed with
  `CARGO_BUILD_JOBS=1`.
- `cargo check --workspace --all-targets`: passed.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo fmt --all -- --check`: passed.
- Markdown audit: 43 files, 1,284 tilde fence markers, no triple-backtick
  fences.
- Maintained v2 Poly audit: 42 blocks, 0 unmarked failures.
- Rust target `--check` smoke test: passed.
- C target `--check` smoke test: passed.
- Native C11 compile with `-Wall -Wextra -Werror` and execution: passed.

## Remove `putl` keyword (2026-08-19)

Removed `putl` from the language entirely. `put` is now the only output
command — it always appends a newline (like Rust's `println!`). The
`putl` keyword (which suppressed the trailing newline) and the legacy
`put -n` flag are both gone. Users who need raw Rust `print!` can use
raw Rust.

### Compiler changes

- **Lexer**: removed `Putl` variant from `TokenKind` and the
  `"putl"` → `TokenKind::Putl` mapping.
- **Parser**: removed `no_newline: bool` from the `PutStatement` AST
  node, removed `Putl` handling from `parse_statement` and
  `parse_put_statement`, updated `keyword_as_field_name` and
  `substitute_statement`.
- **IR**: removed `no_newline` from the `Put` variant in the
  intermediate representation.
- **Codegen**: simplified `gen_put` to always emit `println!`; removed
  all `no_newline` branches in both the main and expression-style code
  generators.
- **Grammar BNF**: removed `putl` from reserved keywords and from the
  `put_statement` rule.
- **LSP**: updated test fixture.
- **REPL**: updated error-tip message.

### Tests

- All Rust test files: replaced `putl` with `put` in embedded Poly
  source strings.
- Replaced `test_parse_putl_statement_sets_no_newline` with
  `test_putl_is_now_an_identifier` (verifies `putl` parses as a plain
  identifier).
- Updated `test_reject_legacy_put_no_newline_flag` assertion.
- All `.poly` example/test files: `putl` → `put`.
- **344 unit + integration tests pass; all examples compile.**

### Documentation

- Every `.md` file updated: removed `putl` references, cleaned up
  transpilation tables (no more `print!` row), removed `-n` flag docs,
  fixed section headers ("put and putl" → "put"), removed progress
  indicator sections that relied on `putl`.

---

## Follow-up pass 5 (2026-08-17, v1.7.8)

Full documentation syntax audit plus the language features the docs surfaced.

### Documentation audit

- **Every fenced Poly block across all guides now parses.** 193 complete
  (non-fragment) blocks type-check and their generated Rust compiles; 345
  context-dependent teaching snippets are marked `poly fragment` per the
  documentation style guide (they reference fictional helpers like
  `process()`/`is_windows()`/`BuildError`, or Good/Bad duplicate definitions).
  `scripts/check_poly_examples.py` now runs `--check` (parse + type-check +
  Rust compile) on complete blocks instead of `--ast`, so wrong method names
  and type errors in the guides fail CI.
- **Genuine doc bugs fixed**: `.length`→`.len()`, `print`→`put`,
  `to_lower`→`to_lowercase`, `255u8`→`as` casts, `.iter().sum()` chains→
  `reduce`, `get --timeout`/`--default`/`--as` Result/type handling, `get from 
  file --timeout N` argument order, `get --as Person`→real types, and the
  if/else syntax template in the spec made into a compiling example.
- **Compiler fixes the docs surfaced**:
  - String mutation: `add s, "text"` / `s += "text"` type-check and compile
    (`String += &str`, amount borrowed for string targets only).
  - `.repeat(n)` on strings and `.join(sep)` on vectors are real methods.
  - Collection loops bind owned values: `loop x in items` lowers to
    `items.iter().cloned()` (enumerate: `.iter().cloned().enumerate()`), so
    the loop variable has element type `T` while the collection stays usable.
  - String/vec/type registration works inside `gen_statement_str` bodies
    (loop/match bodies render through a `&self` path that previously tracked
    nothing): string `+=` in nested loops borrows its amount, and `put buf`
    on a vector declared in a loop body uses `{:?}`. The scope stacks became
    `RefCell`-wrapped so `gen_statement_str` can manage them.

### Vec `clear`/`reserve` and iterator chains

- **`xs.clear()` and `xs.reserve(n)`** type-check (arity/argument validated)
  and lower to the Rust `Vec` methods; the performance guide's pre-allocation
  block now compiles standalone.
- **`xs.iter()` yields owned elements** (`iter().cloned()`), so `sum()`,
  `map(f)`, `filter(p)`, `collect()`, `enumerate()`, and `loop`/`for`
  bindings all see element type `T`. The checker gained an internal
  `PolyType::Iterator(T)` (rejecting non-numeric `sum` and non-callback
  `map`/`filter` arguments); codegen annotates `sum::<T>()` and
  `collect::<Vec<_>>()` via new vec-element-type tracking (from `Vec<T>`
  annotations, array literals, and closure bodies).

### Tooling

- **Playground wasm rebuilt (396K) and smoke-tested in Node** by driving the
  real ABI (`poly_alloc` → `transpile` → `result_*`); higher-order functions
  example transpiles, `+= &(` string appends emit, iter chains emit
  `collect::<Vec<_>>()`, type errors still report, unicode round-trips.
- **LSP verified over stdio**: match-comma and `loop i 1..3` syntax produce
  zero diagnostics; genuine type errors still publish.
- **VS Code grammar**: removed the retired `=>` from operator highlighting.
- Verified: 20 test suites pass, all `examples/*.poly` and `tests/*.poly`
  compile, doc audit 0 failures, CHANGELOG 1.7.8 entry added.

## Follow-up pass 4 (2026-08-16, v1.8.0)

Follow-ups from the tuple index access pass: destructuring, assignment,
enumerate codegen fix, and a playground example.

- **Tuple element assignment**: `pair.0 := 99`, `pair.1 = false`, and
  `add pair.0` mutate tuple elements (assignment targets accept `.N`;
  `starts_assignment_statement` recognizes it; checker validates the type).
- **`for (a, b) in ...` destructuring**: the parser accepts a tuple pattern
  after `for` (stored like `loop` does), the checker declares each name with
  its component type, and codegen emits a Rust tuple pattern.
- **Fixed `for x in xs.enumerate()`**: previously generated the invalid
  `xs.enumerate().iter()`; `gen_for_loop` now mirrors the `loop` lowering
  (`xs.iter().enumerate()`), so the receiver is borrowed and stays usable.
- **Playground**: new Tuples example (index access, nested chains, function
  results, enumerate destructuring, element assignment).
- Verified: 331 workspace tests pass (was 322), clippy/fmt clean, playground
  example runs end-to-end.

## Follow-up pass 3 (2026-08-16, v1.7.6)

Fixes the audit's noted pre-existing limitation: tuple index access.

- **Tuple index access**: `pair.0` and chained `nested.1.0` now parse
  (AST/IR `TupleIndex`), type-check (element type resolved; out-of-bounds and
  non-tuple receivers rejected), and codegen to plain Rust. The lexer is
  context-sensitive after a dot so `t.1.0` lexes as `1`, `.`, `0` like Rust
  instead of a `1.0` float; float literals are otherwise unchanged.
- Verified: 322 workspace tests pass (was 316), clippy/fmt clean, `--ir`
  optimizer path compiles the generated Rust, runtime test runs tuple
  programs end-to-end.

## Follow-up pass 2 (2026-08-16, v1.7.5)

Completes the remaining input stubs and the audit's "other ideas" list:

- **`get --until <delim>` implemented**: reads stdin until the delimiter string
  is found (or EOF) and returns everything before it; the checker warning is
  gone. `get --mask <char>` suppresses terminal echo (best-effort `stty -echo`
  on Unix terminals; plain read on non-Unix or non-terminal stdin).
- **Mini test framework**: `assert(cond)` (panics on false), `pass(msg)`
  (`[PASS] msg`), `fail(msg)` (`[FAIL] msg` + exit code 1), plus `Result`
  accessors `is_ok()`/`is_error()`/`unwrap()`/`unwrap_or(v)` — the
  testing-guide API now type-checks and runs.
- **Checked indexing**: `xs.get(i)` returns `Option<T>` for vectors and
  `Option<char>` for strings (out-of-bounds yields `None` instead of
  panicking); `put` of these prints with `{:?}`.
- **String interpolation**: `"Name: {name}, Age: {age}"` parses into a
  concatenation chain; embedded strings/numbers/bools/chars format in place;
  braces that don't form a parseable expression stay literal.
- **`db_execute(query)` is now real**: it executes the SQL query against a
  shared in-memory SQLite database (`rusqlite`) and returns one string per row
  as `Vec<String>`; the CLI adds `rusqlite` to generated Cargo projects when
  the builtin is used, so CREATE/INSERT/SELECT flows persist across calls. The
  stub warning is gone.
- **Tooling/coverage**: `examples/testing_and_interpolation.poly` demonstrates
  interpolation, checked indexing, and the test helpers; the playground gains
  an "Interpolation & Testing" example; the VS Code TextMate grammar highlights
  `{expr}` interpolation inside double-quoted and `unicode` strings.
- **CI**: new `audit` job runs `cargo audit`.
- Verified: workspace tests pass (incl. new runtime db_execute test), clippy/
  fmt clean, all 21 examples pass `--check`, both doc scripts green, wasm
  rebuilt (380K).

## Follow-up pass (2026-08-16, v1.7.4)

Follow-ups from the v1.7.3 audit-fix pass: runtime execution tests, real
network builtins, and generic substitution at call sites.

- **Runtime execution tests**: the integration suite now compiles *and runs*
  generated programs — line-by-line file reads (`open`/`get_line`/`eof`),
  negative loop steps, plain `get` and `get --as` with piped stdin, Map
  round-trips, and a tokio `spawn` concurrency check. These would have caught
  the v1.7.3 stub bugs (infinite `eof` loop, empty `get_line`, untrimmed `get`).
- **Real network builtins**: `http_get(url)` performs a real HTTP/1.1 GET over a
  tokio TCP connection (URL parsing, raw request, body returned as `Ok`/`Err`);
  `tcp_connect(addr)` establishes a real connection and returns the peer
  address; `spawn <expr>` lowers to `tokio::spawn`. Only `db_execute` (and the
  `--mask`/`--until` flags) remain compile-only stubs with checker warnings.
- **Generic substitution at call sites**: `fn identity<T>(x: T): T` called as
  `identity(42)` now returns `i32` instead of `Unknown`, so mismatched use is
  caught (`var s ustring := identity(42)` errors with `expected String, got
  i32`). Substitution threads through containers (`Vec<T>`, `Map<K, V>`, ...)
  and tuples. Generic bodies stay permissive (`Unknown`), so existing generic
  examples are unaffected.
- **`get` trims stdin**: plain `get` and `get --as <type>` strip the trailing
  newline `read_line` leaves behind, fixing `get` (was returning "Alice\n") and
  `--as i32` parsing (was silently returning 0 on "21\n").

## Audit-fix pass (2026-08-16, v1.7.3)

All items from the 2026-08-15 project review's prioritized follow-up are addressed:

- **Optimizer soundness**: closure-returning functions are excluded from inlining;
  integer comparisons fold to `bool` literals; logical `not` folds only on booleans
  while `~` keeps bitwise semantics. `poly --ir` now verifies the optimized Rust
  compiles before printing, so invalid optimizer output can no longer ship silently.
- **Benchmarks + CI**: `performance_benchmarks.rs` uses comma match syntax; CI runs
  `cargo check --workspace --all-targets`, checks every example with `poly --check`,
  and rebuilds/verifies the checked-in wasm bundle.
- **Runtime APIs**: `open`/`get_line`/`eof` are implemented via `BufReader`;
  `sleep`/`delay`/`exit` lower to real calls; `get --timeout`/`--default`/`--as`/
  `--bytes` are implemented (channel + reader thread, typed parsing) and `get`
  trims the trailing newline. `http_get` does a real HTTP/1.1 GET over tokio TCP,
  `tcp_connect` opens a real connection (returning the peer address), and
  `spawn <expr>` lowers to `tokio::spawn`. Only `db_execute`, `--mask`, and
  `--until` remain stubs (now with checker warnings).
- **Runtime execution tests**: integration tests now compile and *run* generated
  programs — file reads, negative loop steps, `get`/`get --as` with piped stdin,
  Map round-trips, and a tokio `spawn` concurrency check.
- **Match-pattern validation**: unknown names on enum/struct/Result scrutinees are
  rejected instead of silently becoming irrefutable Rust bindings; range patterns
  (`0..=9`) now pass `--check`.
- **Map/Set**: `insert`/`get`/`remove`/`contains_key`/`contains`/`len`/`is_empty`
  type-check and codegen borrows keys; `m[key]`/`m[key] = v` lower correctly;
  keyword method names (`map.get(...)`) parse.
- **Closures**: `var f := |x| x + n; return f` now passes `--check` (`_` behaves as
  unknown; arithmetic on unknown operands is permissive).
- **Loop steps**: negative steps keep magnitude (`10..1 step -2` ->
  `(1..=10).rev().step_by(2)`); constant `step 0` rejected.
- **LSP**: UTF-16-correct hover/completion positions; JSON parser handles surrogate
  pairs.
- **Version metadata**: README/playground now advertise v1.7.3.
- Tests: 13 new checker tests, 7 new optimizer tests, 6 new codegen tests, 4 new
  LSP tests; all 20 example files pass `poly --check`; the wasm was rebuilt (364K).


## Explicit loop variables (2026-08-14)

- `loop` now requires the binding name immediately after the colon, for example
  `loop value 1..3, 7, 19..20` and `loop item in items`.
- The explicit variable is carried through the parser AST, intermediate representation,
  checker, optimizer, and Rust codegen; the old implicit `i`/`j` and collection-name
  heuristics were removed.
- `loop` ranges are inclusive at both endpoints: `1..3` produces `1, 2, 3` and
  lowers to Rust `1..=3`; ordinary non-loop range expressions keep their existing
  exclusive `..` behavior.
- Added parser, checker, and codegen regression coverage and updated examples,
  grammar, cheatsheet, README, guides, and playground source.

## Status

All items in this log are **complete and verified**:

- Workspace build: 0 errors / 0 warnings
- Workspace tests: all pass (20 suites, incl. 34 runtime integration tests)
- Clippy (`--workspace --all-targets`): 0 warnings
- `cargo fmt --all -- --check`: clean
- `scripts/check_markdown.py`: passes (tilde fences, no triple backticks)
- `scripts/check_poly_examples.py`: passes — 538 blocks, 193 complete blocks
  type-check and compile, 345 marked fragments, 0 unmarked failures
- All `examples/*.poly` and `tests/*.poly` pass `poly --check`
- Playground wasm rebuilt (396K) and smoke-tested in Node against the real
  ABI; LSP verified over stdio with the new syntax; VS Code grammar updated

## Higher-order functions (2026-08-13, v1.7.2)

- **Functions can return closures**: `fn make_adder(n: i32): |x: i32| i32` parses,
  `check_return` checks the returned closure literal against the declared
  signature (untyped parameters inherit the declared types), and codegen lowers
  the return type to `impl Fn(i32) -> i32` and emits returned closures with
  `move` so they may capture locals. Closures assigned to a variable and then
  returned are also identified ahead of code generation and emitted with
  `move`. Verified end-to-end: `make_adder(5)(10)` prints `15`,
  `make_adder(100)(1)` prints `101`.
- **Vec `map`/`filter`/`reduce` methods**: `xs.map(|x| x * 2)`,
  `xs.filter(|x| x % 2 == 0)`, `xs.reduce(0, |acc, x| acc + x)`. The checker
  derives the callback signature from the element (and accumulator) types and
  rejects non-function callbacks, wrong parameter counts, and non-`bool` filter
  predicates. Codegen emits `iter().cloned().map/filter/fold(...)` so the
  receiver stays usable; the filter predicate derefs its single parameter.
- **Docs/examples**: `examples/higher_order_functions.poly` (passes `--check`
  and runs), a 7th playground example (`hof`) covering all three features, and
  a README section. The playground demo transpiler now lowers closure-typed fn
  signatures to `impl Fn`.
- Verified: 9 new tests (7 checker, 2 codegen); the wasm was rebuilt and all 7
  playground examples transpile through the real binary. Version bumped to 1.7.2.

### Follow-up: vector printing, string HOFs, and sort_by

- **`put` of vectors prints with `{:?}`**: array literals, vector variables
  (typed or inferred, tracked in a new `vec_scopes` mirroring `string_scopes`),
  and `map`/`filter`/`sort_by`/`split` results now print with debug formatting
  instead of failing to compile (`put xs.map(|x| x * 2)` prints `[2, 4, ...]`).
  File writes (`>`, `>>`) format vectors the same way.
- **Strings support `map`/`filter`/`reduce`** over their characters:
  `"hello".map(|c| c)` yields `Vec<char>`; codegen lowers to `.chars().map/filter/fold`.
- **`sort_by` returns a sorted copy**: `xs.sort_by(|a, b| a > b)` — the checker
  validates a two-parameter boolean comparator, and codegen emits a clone +
  `v.sort_by` wrapper that derefs the references and maps the boolean to
  `Ordering`. Sorting a string is rejected by the checker.
- 6 new tests (3 checker, 3 codegen); `examples/higher_order_functions.poly`
  and the playground `hof` example now demonstrate vector printing, `sort_by`,
  and string `map`. The wasm was rebuilt again (340K).

## Closure type annotations (2026-08-13, v1.7.1)

- **Closure type annotations in parameter positions**: `fn apply(f: |x: i32| i32, v: i32): i32`
  now parses. `parse_type` gained a `Pipe` branch that accepts `|name: type, ...| return_type`
  (parameter names optional, e.g. `|i32| i32`) and maps it to `TypeAnnotation::Function`,
  which already flowed through the generator, codegen (`fn(i32) -> i32`), and checker.
- **Closure literals are checked against the expected signature**: `check_call` resolves the
  callee's declared parameter types up front and `check_argument` checks closure-literal
  arguments against them, so untyped literals inherit the declared types:
  `apply(|x| x * 2, 21)` type-checks. Return-type mismatches (`expected fn(i32) -> i32,
  got fn(i32) -> String`) and arity mismatches are rejected.
- **Function-type compatibility**: `compatible` gained a `Function`/`Function` arm that
  compares params and return structurally, so typed closures stored in variables
  (`var double := |x: i32| x * 2; apply(double, 21)`) pass as function-typed arguments.
- Verified end-to-end: `--check` passes, generated Rust builds and runs (`combine(|a, b| a + b, 2, 3)`
  prints `5`), and the playground wasm was rebuilt (332K) with all 6 examples transpiling.
- **Array iteration is confirmed supported**: `for x in xs` over `Vec<T>` and `String` already
  inferred element types in the checker; added explicit tests and removed it from the known
  limitations list.
- Version bumped to 1.7.1 with matching CHANGELOG entries.

## Bug-fix pass (2026-08-13)

Audit fixes landing with v1.7.0:

- **`while` loops now allow `break`/`continue`**: the checker only bumped
  `loop_depth` for `for`/`loop` ranges, so valid `while ... break` programs
  failed `--check`. The checker now treats if-expressions without an else
  block (the parser's `while` encoding) as loops while checking the body.
- **Nested functions now type-check**: `fn` inside a function body is
  registered as a callable and its body is checked, so `inner(x)` calls from
  the enclosing body no longer report `unknown function`.
- **Builtin `Option`/`Some`/`None` support in the checker**: value
  construction (`Some(42)`, `None`), bare `None,` patterns, and
  `Some(v),` patterns all type-check against `Option<T>` scrutinees;
  mismatches still error.
- **`Map<K,V>`/`Set<T>` empty initializers emit `HashMap::new()`/
  `HashSet::new()`** instead of `vec![]`, so the documented
  `var m Map<ustring, i32> := []` compiles.
- **REPL multi-line input works**: statements that open a block
  (`fn`/`if`/`while`/`for`/`loop`/`match`/`struct`/`enum`/`trait`/`impl`)
  stay buffered until the matching `end <keyword>`, and trailing-`\`
  continuation markers are stripped before parsing (previously the `\`
  reached the lexer and every multi-line entry failed).
- **`poly --check` verifies async programs**: `#[tokio::main]` output is now
  checked in a temporary Cargo project with tokio declared, instead of plain
  `rustc` (which could not resolve the crate). `examples/async_await.poly`
  now passes `--check`.
- **Version bumped to 1.7.0** to match the CHANGELOG (CLI/REPL previously
  reported 1.6.0).
- Rebuilt `playground/poly.wasm` with the fixed checker (332K).

All 20 repo examples (including `async_await.poly`) now pass `poly --check`.

## Completed work

### 1. Span-precise AST

- Top-level statements wrapped in `Spanned<Statement>` with byte spans computed
  from parser token positions (underflow-guarded).
- Nested statement blocks (function bodies, if/match/loop blocks, module bodies,
  unsafe blocks) wrapped in `Spanned<Statement>` via a `Block` alias, so byte
  spans exist for every block in the AST.
- Wired through:
  - `--source-map` registers symbols at exact declaration columns.
  - LSP type-check diagnostics point at the offending statement via
    `statement_symbol_range` instead of scanning source text.
  - LSP positions now use UTF-16 code units per the LSP spec (was counting chars).
- Locked in by `statement_spans_cover_source` test in poly-parser.

### 2. Real `for` loops (end-to-end)

Previously the parser **discarded** the for-loop body (`let _body = ...`).
Now:

- New `ForLoop` AST node (`variable` / `iterable` / `body`) in poly-parser.
- New intermediate-representation variant mapped by the generator.
- Codegen emits idiomatic Rust: `for i in 0..5 { ... }`.
- Optimizer folds the loop body.
- Type checker handles `for` loops; they pass `--check`.
- Macro substitution (`substitute_expression`) handles `ForLoop`.

### 3. Type-checker gaps fixed

- `Type::method(...)` associated-function calls resolve against struct methods.
  Arity is computed correctly for `self` receivers via a new
  `FunctionSignature.has_self` field (the parser declares `self` params
  untyped, so the checker can detect it by name).
- Calling closures stored in variables (`var square := |x| x * x; put square(4)`)
  now type-checks via scope lookup.
- Verified: repo examples and playground examples pass `--check`.

### 4. Web playground: real compiler in the browser

- New dependency-free `poly-wasm` crate exposing `poly_alloc` / `poly_free` /
  `transpile` / `result_*` / `free_result` as `#[no_mangle] extern "C"` exports.
- Playground loads `playground/poly.wasm` and runs the real transpiler, with a
  JS demo fallback when WebAssembly is unavailable.
- Added `compiler/scripts/build_playground_wasm.sh` to rebuild the bundle.
- Playground examples upgraded to the now-supported constructs: `for` loops,
  called closures, and structs with `impl` methods + associated functions
  (`Point::new(...)`, `p1.distance_sq(p2)`).

#### Wasm bundle size: 551KB -> 328KB

- Size optimizations scoped to the wasm build only via `--config` in the build
  script (`opt-level = "z"`, LTO, `codegen-units = 1`, `strip`,
  `panic = "abort"`). The CLI and LSP keep default release settings.

#### Bugs caught during wasm work

- **libc interposition SIGSEGV**: exports were named `alloc`/`free`, colliding
  with libc on native test binaries; renamed to `poly_alloc`/`poly_free`.
- **Pointer truncation**: result block stored `ptr as u32`, breaking native
  64-bit tests; now stores native `usize` fields (works on wasm32 and 64-bit).
- **wasm-only compile error**: `read_usize` used a fixed 8-byte buffer; fixed
  with `[u8; PTR_SIZE]`.
- Added 3 native round-trip tests (Rust output, type errors, unicode) that
  would have caught both bugs instantly.

### 5. VS Code extension (`vscode/`)

- TextMate grammar (`syntaxes/poly.tmLanguage.json`, scope `source.poly`):
  keywords, types, strings, comments, numbers, I/O commands, and
  function/struct/enum/trait/impl/variable scopes.
- Language configuration: line comments (`#`), bracket pairs, indentation rules
  for `if/for/while/loop/match/fn/struct/...` blocks.
- Language-client extension (`client/extension.js`) that launches `poly-lsp`
  with diagnostics, completion, hover, and document symbols; a
  `Poly: Restart Language Server` command; and a `poly.lsp.path` setting with a
  real binary-existence warning.
- `vscode-languageclient` is in `dependencies` so packaged `.vsix` builds work.

### 6. Language server (poly-lsp) documentation

- Full section in `POLY_INTEGRATION_GUIDE.md`: features table (diagnostics /
  completion / hover / symbols), build instructions, ready-to-paste VS Code and
  Neovim config snippets, and a pointer to the shipped VS Code extension.

### 7. Docs & metadata

- `CHANGELOG.md`: 1.7.0 entries for all of the above.
- `README.md`: project structure fixed (was mangled) and updated with
  `poly-wasm`, `poly-lsp`, and `vscode/` entries.

## Files touched (summary)

~~~txt
compiler/Cargo.toml
compiler/scripts/build_playground_wasm.sh
compiler/crates/poly-parser/src/ast.rs
compiler/crates/poly-parser/src/parser.rs
compiler/crates/poly-transpiler/src/checker.rs
compiler/crates/poly-intermediate-representation/src/intermediate_representation.rs
compiler/crates/poly-intermediate-representation/src/generator.rs
compiler/crates/poly-intermediate-representation/src/codegen.rs
compiler/crates/poly-intermediate-representation/src/optimizer.rs
compiler/crates/poly-wasm/Cargo.toml            (new)
compiler/crates/poly-wasm/src/lib.rs             (new)
playground/poly.wasm                              (rebuilt bundle)
playground/index.html
vscode/package.json                               (new)
vscode/language-configuration.json                (new)
vscode/syntaxes/poly.tmLanguage.json              (new)
vscode/client/extension.js                        (new)
vscode/client/check-deps.js                       (new)
vscode/README.md                                  (new)
POLY_INTEGRATION_GUIDE.md
CHANGELOG.md
README.md
PROGRESS.md                                        (this file)
~~~

## Known limitations / future work

- `^` is a bitwise operator, not exponentiation; float powers need a library
  call or `sqrt`-style helper.
- `Type::method(receiver, args)` (explicit-receiver call via `::`) hits the
  associated-function arity path; the error message can be confusing.
- The playground wasm could shrink further with `wasm-opt` if installed.

## Project review (2026-08-15)

### Verified working

- `cargo test --workspace -- --test-threads=1` passes across the workspace (260 listed test cases, including parser recovery, checker, IR, LSP, WASM native round trips, and integration tests).
- `cargo clippy --workspace --all-targets -- -D warnings` passes.
- `cargo fmt --all -- --check` passes.
- `scripts/check_markdown.py` passes: 35 Markdown files and 1,252 tilde fence markers.
- `scripts/check_poly_examples.py` passes: 537 Poly blocks, 378 complete examples parsed, 159 marked fragments, and 0 unmarked parse failures.
- Every current `examples/*.poly` file passes `poly --check`, including `higher_order_functions.poly` and `async_await.poly`.
- The parser/checker/codegen support explicit loop bindings, inclusive ranges, collection iteration, closures, function-typed parameters and return values, vector/string higher-order methods, `Option`, nested functions, async syntax, LSP features, and the native WASM ABI tests.

### Verified failures and risks

- `cargo test --workspace --all-targets` fails because `poly-cli/benches/performance_benchmarks.rs` still contains retired `=>` match-arm syntax. CI currently runs workspace tests without `--all-targets`, so this regression is not caught.
- The optimized `--ir` path can emit invalid Rust for closure-returning functions: inlining `make_adder` loses the captured `n`. The optimizer also folds boolean comparisons into integer literals and the IR codegen maps bitwise-not to logical-not.
- Runtime APIs including `http_get`, `tcp_connect`, `db_execute`, `delay`, `eof`, and `get_line` are placeholder implementations; generated file/input operations use `unwrap()` and can panic. `spawn` lowers to `spawn_task` without a complete visible runtime implementation.
- Generic type checking is permissive: generic type variables are commonly represented as `Unknown`, generic substitutions are not inferred at call sites, and generic bounds are not semantically validated.
- Source-map line mappings remain best-effort heuristics, despite precise statement spans being available in the AST.
- LSP diagnostics convert to UTF-16, but hover/completion position handling still uses character indexes; non-ASCII text can therefore produce incorrect locations. The custom JSON parser also lacks full surrogate-pair escape handling.
- `README.md` and `playground/index.html` still advertise v1.6.0 while the compiler workspace is v1.7.2.
- CI does not rebuild/verify the checked-in WASM bundle, run browser tests, package the VS Code extension, or execute benchmark targets.

### Prioritized follow-up

1. Update benchmark fixtures to comma match syntax and add `--all-targets` to CI.
2. Disable unsafe optimizer inlining for closures/generics/async functions and compile-check optimized output; fix boolean constant folding and bitwise-not codegen.
3. Decide which runtime APIs are supported versus experimental, then implement them or reject them during checking.
4. Synchronize version metadata and add WASM/VS Code artifact validation to CI.
5. Implement generic substitution/bound checking, UTF-16-safe LSP positions, and source-location propagation through the IR.
6. Add runtime tests for higher-order functions, async behavior, file errors, generic misuse, and optimized output.
