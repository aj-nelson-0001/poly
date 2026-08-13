# Poly — Session Progress

Working log of improvements made to the Poly compiler, playground, and tooling.
Last updated: 2026-08-13.

## Status

All items in this log are **complete and verified**:

- Workspace build: 0 errors / 0 warnings
- Workspace tests: all pass (221 tests across 14 suites)
- Clippy (`--workspace --all-targets`): 0 warnings
- `cargo fmt --all -- --check`: clean
- `scripts/check_markdown.py`: passes (tilde fences, no triple backticks)
- `scripts/check_poly_examples.py`: passes (0 unmarked parse failures)
- Playground JS: syntax valid; all 6 examples transpile through the real wasm binary

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
  construction (`Some(42)`, `None`), bare `None =>` patterns, and
  `Some(v) =>` patterns all type-check against `Option<T>` scrutinees;
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

- `for` loops iterate ranges; element-type inference for array iteration is not
  yet implemented in the checker (Rust compile step still catches it via
  `--check`).
- Closure type annotations in parameter positions (`fn apply(f: |x: i32| i32)`)
  are not yet parsed; closures are inferred from literals only.
- `^` is a bitwise operator, not exponentiation; float powers need a library
  call or `sqrt`-style helper.
- `Type::method(receiver, args)` (explicit-receiver call via `::`) hits the
  associated-function arity path; the error message can be confusing.
- The playground wasm could shrink further with `wasm-opt` if installed.
