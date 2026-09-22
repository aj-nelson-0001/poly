# Poly v2 Support Matrix

**Status:** Frozen preview contract, current through `2.0.0-preview.17`

This matrix is the implementation contract for the current Rust, C, assembly, and JavaScript targets. A construct marked `Parsed` may exist in the parser and AST without being runnable on every backend.

## Target Summary

| Area | Rust target | C target | Asm target | JS target | Notes |
|---|---|---|---|---|---|
| Default target | Yes | No | No | No | Rust is selected unless `--target c`, `--target asm`, or `--target js` is supplied. |
| Native validation | `rustc` or temporary Cargo project | C11 compiler | GNU assembler + linker | `node --check` + execution | `POLY_CC` selects the C compiler for CLI checks/builds. The asm target emits and assembles x86-64 `.S` sources on Linux. The JS target emits ES2020 with no runtime dependencies. |
| Foreign block | `#rust` | `#c` | `#asm` | `#js` | Blocks are top-level, opaque, and emitted verbatim. |
| Explicit foreign signature | `extern rust fn ...` | `extern c fn ...` | `extern asm fn ...` | `extern js fn ...` | Signature is checked by Poly and never emitted. |
| External dependencies | `dep name = "version"` | Rejected with a warning | Rejected with a warning | Rejected with a warning | `dep` declarations are Rust-target only: `--project` emits them into `Cargo.toml` and `--check` resolves them via a temporary Cargo project. The C, asm, and JS backends warn that `dep` statements are ignored. |
| C++ | Rejected | Rejected | Rejected | Rejected | `#cpp` remains reserved; no C++ backend exists. |

## Language Surface

| Construct | Rust | C | Asm | JS | Notes |
|---|---:|---:|---:|---:|---|
| Scalar declarations and assignment | Yes | Yes | Limited | Yes | Primitive values and inferred scalar declarations. |
| Boolean rendering (`put`, `.to_string()`, concat) | Yes | Yes | Yes | Yes | All targets render `true`/`false` text (the C/asm 1/0 output was normalized to the Rust reference behavior). C uses `poly_bool_str`; asm uses `_print_bool`/`_bool_to_string`. |
| Constants and `let` | Yes | Yes | Limited | Yes | C uses native `const`/local declarations. |
| Arithmetic, comparison, logical, bitwise operators | Yes | Yes | Limited | Yes | C uses C11-compatible scalar expressions; integer `/` truncates via `Math.trunc` in JS. Integer overflow is defined on every target: default-width (`i32`) arithmetic wraps two's-complement (rust emits `wrapping_*`, JS masks with `\| 0`, C and asm wrap naturally at 32 bits), and division/modulo by `-1` is guarded in both widths (`INT_MIN / -1` is `INT_MIN`, mod is `0` — no SIGFPE). Declared-`i64` arithmetic stays exact 64-bit on rust/C/asm; JS is exact to the 2^53 double limit. Big integer literals (outside i32) are i64-class on every target. |
| Strings and string concatenation | Yes | Yes | Yes | Yes | All targets support `+` concat in value and `put` position plus `.len()`. C materializes concat with an emitted `poly_concat` helper and `to_string()` on scalars with `poly_int_to_string`/`poly_float_to_string`; asm carves results from a static bump arena (`_str_arena`). String interpolation lowers to concatenation on every target. Asm string method calls beyond `.len()` are rejected with guidance. |
| `put`, `error`, `warn`, `info` | Yes | Yes | Limited | Yes | C supports scalar output and diagnostics; JS maps the diagnostics to `console` streams. |
| File output redirects | Yes | No | No | No | C must call a `#c` helper. |
| `get`, stdin, file input | Yes | Limited | No | No | C supports plain `get` (with optional prompt) via an emitted runtime helper; file input and input flags need a `#c` helper. JS rejects all stdin input as browser-hostile. |
| Typed input flags | Yes | No | No | No | `--as`, `--default`, `--mask`, `--until`, `--timeout`, and `--bytes` are Rust runtime behavior. |
| `if`/`else` | Yes | Yes | Limited | Yes | |
| `while` | Yes | Yes | Limited | Yes | |
| Infinite loops | Yes | Yes | Limited | Yes | |
| Inclusive numeric loops | Yes | Yes | Limited | Yes | C currently supports one numeric range per loop. `step` is supported everywhere: literal steps fix the direction at compile time, and variable steps dispatch on the step's runtime sign (a zero step terminates) — identical iteration semantics on all four targets. |
| Multi-range loops | Yes | No | No | No | C and JS reject multiple range parts. |
| Collection loops | Yes | Limited | Limited | Yes | C only supports a shallow C-array form. Asm supports `for x in <vector>` and `for x in <string>`. JS lowers both `for x in` and `loop x in` to `for...of`. |
| `break`/`continue` | Yes | Yes | Limited | Yes | |
| Simple Poly functions | Yes | Yes | Limited | Yes | C functions cannot be async or generic. |
| Structs | Yes | Plain only | Yes | Plain only | C rejects methods and generics; struct literals lower to C99 compound literals. Asm supports program-scope structs: field access, struct-literal variables, struct parameters (passed by reference), and struct-returning functions. JS supports plain structs as factory-style constructor functions; methods and generics are rejected. |
| Enum declarations and variant values | Yes | Limited | Limited | No | C emits `typedef enum` with qualified enumerators; simple variants work in expressions and `match` patterns, payload-carrying variants are rejected. Asm supports unit variants in expressions and `match`; payload-carrying variants are rejected. |
| Traits, impls, modules, aliases | Yes | No | No | No | Use a foreign helper or the Rust target. |
| Generics | Yes | No | No | No | C rejects generic Poly declarations. |
| Closures and higher-order operations | Yes | Limited | No | No | C compiles non-capturing closures (static function + typed pointer); capturing closures are rejected with guidance. |
| Tuples | Yes | Yes | No | Yes | C tuples are anonymous structs with `_N` fields; `.N` index access, nesting, and match over them are supported. JS tuples lower to arrays with `.N` index access. |
| `match` statements | Yes | Limited | No | Yes | C lowers literal, wildcard, range, and identifier arms to an if/else-if chain; structured patterns and guards are rejected. JS lowers literal, wildcard, and unit-enum arms to `switch`. |
| `match` expressions (value position) | Yes | Limited | Limited | Limited | All four targets support literal and wildcard arms in initializers and other value positions. C lowers to a ternary chain (also range and unit-enum patterns; qualified `Enum::Variant` patterns only — the C subset cannot resolve bare variant names) and aborts on an unmatched scrutinee, mirroring the Rust target's panic. JS lowers to an IIFE `switch` (unit-enum patterns subject to the JS enum row) and throws on an unmatched scrutinee. Asm treats the last arm as always-matching. Block arms and guards are rejected in value position on C/JS/asm. |
| `Option`/`Result` | Yes | No | No | No | |
| Vectors | Yes | No | Limited | Limited | Asm vectors are static descriptor-backed data; `push`/`pop`/`len` switch to a growable in-place runtime (static bump arena). JS supports array literals and iteration. |
| Async/await and task spawning | Yes | No | No | No | Rust uses Tokio where required. |
| SQLite `db_execute` | Yes | No | No | No | Generated Rust projects add `rusqlite` automatically. |
| Network helpers | Yes | No | No | No | Rust helpers use Tokio TCP. |
| LSP and VS Code tooling | Target-neutral | — | — | — | The language server analyzes Poly syntax and checker behavior, not foreign bodies. |

The asm column reflects the narrow Linux x86-64 subset documented in the release notes: only constructs the backend can emit without a cross-language runtime representation are marked, and everything else is rejected with guidance.

## Foreign Interface Rules

Opaque foreign calls remain supported for compatibility:

~~~poly fragment
#c
int double_value(int value) { return value * 2; }
#endc

var result i32 := double_value(21)
~~~

Use an explicit declaration when Poly-side argument and return checks are desirable:

~~~poly fragment
extern c fn double_value(value: i32): i32

#c
int double_value(int value) { return value * 2; }
#endc

var result i32 := double_value(21)
~~~

Rules:

- `extern rust fn`, `extern c fn`, `extern asm fn`, and `extern js fn` declarations must be top-level.
- The target name must match the selected backend; declarations for other targets are removed before checking and generation.
- Explicit declarations validate arity and Poly-visible types.
- The native compiler remains authoritative for the foreign definition, ABI, pointer layout, ownership, lifetimes, calling convention, and target-specific types. For the JS target the browser or Node runtime is authoritative.
- An explicit declaration does not emit a duplicate prototype or function body.
- By default, existing calls without an explicit declaration remain permissive and are validated by `rustc` or the C compiler. `poly --strict` rejects such calls at check time instead; use it in CI when every foreign call should be declared.

## Unsupported Behavior

Unsupported target constructs fail with a diagnostic during `--check` or generation. They must not silently lower to placeholder values. The preview intentionally prefers a clear rejection and a foreign helper boundary over an invented cross-language runtime representation.

## Verification Contract

The repository CI must run:

~~~bash
python3 scripts/check_markdown.py
python3 scripts/check_poly_examples.py README.md POLY_SPEC_v2.md POLY_ROADMAP_v2.md POLY_C_BLOCKS.md POLY_JS_BLOCKS.md POLY_V2_AUDIT.md POLY_MIGRATION_GUIDE_v2.md POLY_V2_SUPPORT_MATRIX.md POLY_PREVIEW_RELEASE_CHECKLIST.md POLY_V2_PREVIEW_RELEASE_NOTES.md
cd compiler
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace -- --test-threads=1
cargo check --workspace --all-targets
~~~

The C target is additionally checked and executed on every supported CI operating system with `POLY_CC` set to the platform compiler. The asm target fixture (`tests/extern_asm_deref.poly`) is checked, compiled, executed, and its output verified on Linux.
