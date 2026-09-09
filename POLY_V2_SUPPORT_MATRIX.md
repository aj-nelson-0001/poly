# Poly v2 Support Matrix

**Status:** Frozen preview contract for `2.0.0-preview.2`

This matrix is the implementation contract for the current Rust, C, and assembly targets. A construct marked `Parsed` may exist in the parser and AST without being runnable on every backend.

## Target Summary

| Area | Rust target | C target | Asm target | Notes |
|---|---|---|---|---|
| Default target | Yes | No | No | Rust is selected unless `--target c` or `--target asm` is supplied. |
| Native validation | `rustc` or temporary Cargo project | C11 compiler | GNU assembler + linker | `POLY_CC` selects the C compiler for CLI checks/builds. The asm target emits and assembles x86-64 `.S` sources on Linux. |
| Foreign block | `#rust` | `#c` | `#asm` | Blocks are top-level, opaque, and emitted verbatim. |
| Explicit foreign signature | `extern rust fn ...` | `extern c fn ...` | `extern asm fn ...` | Signature is checked by Poly and never emitted. |
| C++ | Rejected | Rejected | Rejected | `#cpp` remains reserved; no C++ backend exists. |

## Language Surface

| Construct | Rust | C | Status |
|---|---:|---:|---|
| Scalar declarations and assignment | Yes | Yes | Primitive values and inferred scalar declarations. |
| Constants and `let` | Yes | Yes | C uses native `const`/local declarations. |
| Arithmetic, comparison, logical, bitwise operators | Yes | Yes | C uses C11-compatible scalar expressions. |
| Strings and string concatenation | Yes | Limited | No | C output can flatten simple string output; value-position concatenation needs a `#c` helper. Asm handles char vectors and `for x in <string>`; general string values are rejected with guidance. |
| `put`, `error`, `warn`, `info` | Yes | Yes | C supports scalar output and diagnostics. |
| File output redirects | Yes | No | C must call a `#c` helper. |
| `get`, stdin, file input | Yes | Limited | No | C supports plain `get` (with optional prompt) via an emitted runtime helper; file input and input flags need a `#c` helper. |
| Typed input flags | Yes | No | `--as`, `--default`, `--mask`, `--until`, `--timeout`, and `--bytes` are Rust runtime behavior. |
| `if`/`else` | Yes | Yes | |
| `while` | Yes | Yes | |
| Infinite loops | Yes | Yes | |
| Inclusive numeric loops | Yes | Yes | C currently supports one numeric range per loop. |
| Multi-range loops | Yes | No | C rejects multiple range parts. |
| Collection loops | Yes | Limited | Limited | C only supports a shallow C-array form. Asm supports `for x in <vector>` and `for x in <string>`. |
| `break`/`continue` | Yes | Yes | |
| Simple Poly functions | Yes | Yes | C functions cannot be async or generic. |
| Structs | Yes | Plain only | No | C rejects methods and generics; struct literals lower to C99 compound literals. |
| Enum declarations and variant values | Yes | Limited | No | C emits `typedef enum` with qualified enumerators; simple variants work in expressions and `match` patterns, payload-carrying variants are rejected. |
| Traits, impls, modules, aliases | Yes | No | No | Use a foreign helper or the Rust target. |
| Generics | Yes | No | No | C rejects generic Poly declarations. |
| Closures and higher-order operations | Yes | Limited | No | C compiles non-capturing closures (static function + typed pointer); capturing closures are rejected with guidance. |
| Tuples | Yes | Yes | No | C tuples are anonymous structs with `_N` fields; `.N` index access, nesting, and match over them are supported. |
| `match` statements | Yes | Limited | No | C lowers literal, wildcard, range, and identifier arms to an if/else-if chain; structured patterns and guards are rejected. |
| `Option`/`Result` | Yes | No | No | |
| Vectors | Yes | No | Limited | Asm vectors are static descriptor-backed data; `push`/`pop`/`len` switch to a growable in-place runtime (static bump arena). |
| Async/await and task spawning | Yes | No | No | Rust uses Tokio where required. |
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

- `extern rust fn`, `extern c fn`, and `extern asm fn` declarations must be top-level.
- The target name must match the selected backend; declarations for other targets are removed before checking and generation.
- Explicit declarations validate arity and Poly-visible types.
- The native compiler remains authoritative for the foreign definition, ABI, pointer layout, ownership, lifetimes, calling convention, and target-specific types.
- An explicit declaration does not emit a duplicate prototype or function body.
- By default, existing calls without an explicit declaration remain permissive and are validated by `rustc` or the C compiler. `poly --strict` rejects such calls at check time instead; use it in CI when every foreign call should be declared.

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

The C target is additionally checked and executed on every supported CI operating system with `POLY_CC` set to the platform compiler. The asm target fixture (`tests/extern_asm_deref.poly`) is checked, compiled, executed, and its output verified on Linux.
