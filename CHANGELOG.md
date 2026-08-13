# Changelog

All notable changes to the Poly language compiler will be documented in this file.

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
