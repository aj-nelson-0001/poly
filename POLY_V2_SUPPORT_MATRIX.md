# Poly v2 Support Matrix

**Status:** Frozen preview contract for `2.0.0-preview.1`

This matrix is the implementation contract for the current Rust and C targets. A construct marked `Parsed` may exist in the parser and AST without being runnable on every backend.

## Target Summary

| Area | Rust target | C target | Notes |
|---|---|---|---|
| Default target | Yes | No | Rust is selected unless `--target c` is supplied. |
| Native validation | `rustc` or temporary Cargo project | C11 compiler | `POLY_CC` selects the C compiler for CLI checks/builds. |
| Foreign block | `#rust` | `#c` | Blocks are top-level, opaque, and emitted verbatim. |
| Explicit foreign signature | `extern rust fn ...` | `extern c fn ...` | Signature is checked by Poly and never emitted. |
| C++ | Rejected | Rejected | `#cpp` remains reserved; no C++ backend exists. |

## Language Surface

| Construct | Rust | C | Status |
|---|---:|---:|---|
| Scalar declarations and assignment | Yes | Yes | Primitive values and inferred scalar declarations. |
| Constants and `let` | Yes | Yes | C uses native `const`/local declarations. |
| Arithmetic, comparison, logical, bitwise operators | Yes | Yes | C uses C11-compatible scalar expressions. |
| Strings and string concatenation | Yes | Limited | C output can flatten simple string output; value-position concatenation needs a `#c` helper. |
| `put`, `error`, `warn`, `info` | Yes | Yes | C supports scalar output and diagnostics. |
| File output redirects | Yes | No | C must call a `#c` helper. |
| `get`, stdin, file input | Yes | No | C must call a `#c` helper. |
| Typed input flags | Yes | No | `--as`, `--default`, `--mask`, `--until`, `--timeout`, and `--bytes` are Rust runtime behavior. |
| `if`/`else` | Yes | Yes | |
| `while` | Yes | Yes | |
| Infinite loops | Yes | Yes | |
| Inclusive numeric loops | Yes | Yes | C currently supports one numeric range per loop. |
| Multi-range loops | Yes | No | C rejects multiple range parts. |
| Collection loops | Yes | Limited | C only supports a shallow C-array form; vectors and collection values are unsupported. |
| `break`/`continue` | Yes | Yes | |
| Simple Poly functions | Yes | Yes | C functions cannot be async or generic. |
| Structs | Yes | Plain only | C rejects methods and generics. |
| Enums, traits, impls, modules, aliases | Yes | No | Use a `#c` helper or Rust target. |
| Generics | Yes | No | C rejects generic Poly declarations. |
| Closures and higher-order operations | Yes | No | C rejects closures and method-based collection operations. |
| Tuples | Yes | No | |
| `Option`/`Result` and pattern matching | Yes | No | |
| Async/await and task spawning | Yes | No | Rust uses Tokio where required. |
| SQLite `db_execute` | Yes | No | Generated Rust projects add `rusqlite` automatically. |
| Network helpers | Yes | No | Rust helpers use Tokio TCP. |
| LSP and VS Code tooling | Yes | Target-neutral | The language server analyzes Poly syntax and checker behavior, not foreign bodies. |

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

- `extern rust fn` and `extern c fn` declarations must be top-level.
- The target name must match the selected backend; declarations for other targets are removed before checking and generation.
- Explicit declarations validate arity and Poly-visible types.
- The native compiler remains authoritative for the foreign definition, ABI, pointer layout, ownership, lifetimes, calling convention, and target-specific types.
- An explicit declaration does not emit a duplicate prototype or function body.
- Existing calls without an explicit declaration remain permissive and are validated by `rustc` or the C compiler.

## Unsupported Behavior

Unsupported target constructs fail with a diagnostic during `--check` or generation. They must not silently lower to placeholder values. The preview intentionally prefers a clear rejection and a foreign helper boundary over an invented cross-language runtime representation.

## Verification Contract

The repository CI must run:

~~~bash
python3 scripts/check_markdown.py
python3 scripts/check_poly_examples.py README.md POLY_SPEC_v2.md POLY_ROADMAP_v2.md POLY_C_BLOCKS.md POLY_V2_AUDIT.md POLY_MIGRATION_GUIDE_v2.md POLY_V2_SUPPORT_MATRIX.md POLY_PREVIEW_RELEASE_CHECKLIST.md POLY_V2_PREVIEW_RELEASE_NOTES.md
cd compiler
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace -- --test-threads=1
cargo check --workspace --all-targets
~~~

The C target is additionally checked and executed on every supported CI operating system with `POLY_CC` set to the platform compiler.
