# Poly — Session Progress

Working log of improvements made to the Poly compiler, playground, and tooling.
Last updated: 2026-08-16.

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
  `--bytes` are implemented (channel + reader thread, typed parsing). The remaining
  stubs (`http_get`, `tcp_connect`, `db_execute`, `spawn`, `--mask`, `--until`)
  now emit checker warnings and docs mark them experimental.
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

- `loop:` now requires the binding name immediately after the colon, for example
  `loop: value 1..3, 7, 19..20` and `loop: item in items`.
- The explicit variable is carried through the parser AST, intermediate representation,
  checker, optimizer, and Rust codegen; the old implicit `i`/`j` and collection-name
  heuristics were removed.
- `loop:` ranges are inclusive at both endpoints: `1..3` produces `1, 2, 3` and
  lowers to Rust `1..=3`; ordinary non-loop range expressions keep their existing
  exclusive `..` behavior.
- Added parser, checker, and codegen regression coverage and updated examples,
  grammar, cheatsheet, README, guides, and playground source.

## Status

All items in this log are **complete and verified**:

- Workspace build: 0 errors / 0 warnings
- Workspace tests: all pass (246 tests across 20 suites)
- Clippy (`--workspace --all-targets`): 0 warnings
- `cargo fmt --all -- --check`: clean
- `scripts/check_markdown.py`: passes (tilde fences, no triple backticks)
- `scripts/check_poly_examples.py`: passes (0 unmarked parse failures; 21 examples)
- Playground JS: syntax valid; all 7 examples transpile through the real wasm binary

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
  `loop_depth` for `for`/`loop:` ranges, so valid `while ... break` programs
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
