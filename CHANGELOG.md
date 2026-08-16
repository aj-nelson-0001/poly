# Changelog

All notable changes to the Poly language compiler will be documented in this file.

## [1.7.3] - 2026-08-16

Audit pass fixing optimizer soundness, runtime stubs, and checker holes.

### Fixed

- **Optimizer soundness**: functions returning closures are no longer inlined (previously the optimized `--ir` path emitted `|x| (x + n)` with a dangling capture); integer comparisons now fold to `bool` literals instead of `int` (`let flag: bool = 1;` no longer generated); logical `not` folds only over booleans while `~` keeps bitwise semantics. The CLI `--ir` path now verifies that the optimized Rust actually compiles before printing it.
- **Runtime APIs**: `open` lowers to a `BufReader` and `get_line`/`eof` are implemented (`while not f.eof()` no longer loops forever, `get_line` no longer returns an empty string). `sleep` lowers to `std::thread::sleep` and `delay` to `tokio::time::sleep`. `exit(n)` lowers to `std::process::exit`. `get --timeout`/`--default`/`--as`/`--bytes` are implemented (real timeout via channel + reader thread, typed parsing, bounded raw reads). `http_get`/`tcp_connect`/`db_execute`/`spawn`/`--mask`/`--until` remain stubs but now emit compiler warnings instead of silently misbehaving.
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
- Range and collection loops now require an explicit binding: `loop: value 1..3, 7, 19..20` and `loop: item in items`; implicit `i`, `j`, and collection-name-derived bindings are no longer generated. Loop-range endpoints are inclusive, so the range example visits `1, 2, 3, 7, 19, 20` and lowers to inclusive Rust ranges.

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

- `break`/`continue` now work inside `while` loops (the type checker only registered loop depth for `for` and `loop:` ranges, so valid programs were rejected).
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
- Added file write operations (`put "content" > "file"`)
- Added file append operations (`put "content" >> "file"`)
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
