# Poly Goose Backend

**Status:** Current for Poly 2.0.0-preview.17

This document describes the implemented Goose target. It is not a proposal for a future general-purpose Goose translator.

Together with [POLY_JS_BLOCKS.md](POLY_JS_BLOCKS.md), this file is the backend document for the Goose target; the frozen
cross-target contract lives in [POLY_V2_SUPPORT_MATRIX.md](POLY_V2_SUPPORT_MATRIX.md).

## Target Model

Poly lowers its orchestration statements to Goose source that passes `goose check` and runs under `goose run`. A selected
`#goose ... #endgoose` block is copied verbatim at file scope, then Poly-generated code is placed in `fn main()`. The `goose`
compiler validates the copied declarations and any calls from `main`.

~~~poly fragment
extern goose fn double_value(n: i32): i32

#goose
fn double_value(n: i32) -> i32 {
    return n * 2;
}
#endgoose

fn main()
    var answer i32 := double_value(21)
    put answer
end fn
~~~

Select the backend explicitly when a file contains multiple foreign blocks:

~~~bash
poly --target goose --check program.poly
poly --target goose --emit-goose program.poly
poly --target goose program.poly
poly --target goose --project build-dir program.poly
~~~

Only `#goose` blocks are emitted for the Goose target. `#rust` blocks are ignored by Goose code generation.
`extern goose fn ...` declarations are checker-only interface contracts and are not emitted. `#cpp` is rejected explicitly
because no C++ backend exists. `dep` declarations are ignored, as on the other non-Rust targets.

## Supported Poly Surface

The Goose backend currently supports:

- scalar variable, `let`, and `const` declarations (`var` needs a type, an initializer, or both)
- plain structs without methods or generics, plus struct literals and field access
- simple Poly functions without async, generics, or parameter defaults
- assignments, arithmetic, comparisons, logical and bitwise operators
- `if`/`else`, `while`, numeric `loop` ranges (inclusive, descending, stepped, and runtime-step), value loops
  (`loop k in [...]`), infinite loops, and `break`/`continue`
- `match` statements with literal and wildcard arms; value-position `match` with expression arms
- calls to functions defined in `#goose` blocks through `extern goose fn ...`
- `put` output lowered to `print(...)`, with string concatenation lowered to the generic `str(...)`
- `.to_string()` lowered to `str(...)`, `.len` lowered to the `.len` member, and `.push` lowered to `.push`
- vector literals and mutators (`Vec<T>` lowers to a growable slice)
- array literals, indexing, `Option` values, and `as` casts lowered to the unchecked `as!` cast
- integer `^`, lowered to an emitted `__poly_pow` helper, and `mod`, lowered to an emitted `__poly_mod_trunc` helper (delegating to `std.rem` via a file-level `import std;`: Goose's native `%` is Euclidean — result in `[0, |b|)` — while the frozen rust/c/asm/js row demands the truncated sign-of-dividend remainder, and `std.rem` is the documented truncated partner of `/`). Both helpers take untyped parameters so the real Goose compiler specializes them per operand width and signedness,
  both matching the shared arithmetic contract of the other backends

The Goose subset follows the JavaScript surface plus vector literals and mutators. Anything with a faithful `#goose` escape
hatch is rejected with a message that names both the limitation and that escape hatch, so no construct is silently
miscompiled.

## Type Mapping

| Poly | Goose |
|---|---|
| `bool` | `bool` |
| integer and float scalars (`i32`, `u8`, `f64`, ...) | passed through unchanged |
| `string`/`ustring`/`bytes` | `u8[>..]` |
| `Vec<T>` | `T[>..]` |
| `[T; N]` | `T[N]` (literal length required) |
| `Option<T>` | `T?` |
| struct names | passed through unchanged |

Poly strings are UTF-8 bytes, so they lower to a byte slice; other arrays keep their element type and slice spelling. The
mapping is intentionally shallow. Poly does not add bounds checking, allocation, or Unicode normalization to Goose values.

## Unsupported Features

The Goose backend rejects these Poly constructs with diagnostics that name the limitation and the `#goose` escape hatch:

- enum declarations and enum variants — declare the enum in a `#goose` block
- tuples and tuple indexing — use a struct or a `#goose` block
- closures — use a named function or a `#goose` block
- async functions — move the async logic into a `#goose` block or use plain sync functions
- generic functions, generic structs, struct methods, and parameter defaults
- match guards and binding or structured patterns — handle the condition with an `if` inside a `#goose` block
- `get` input (plain, prompted, and file input) — read the value as a constant or use a `#goose` block
- `put ... to ...` and `put ... from ...` redirects — use a `#goose` block for file I/O
- `error`, `warn`, and `info` (Goose has no stderr writer here) — use `put` or a `#goose` block
- unicode character literals — use a string literal
- method calls other than `.to_string()`, `.len`, and `.push`
- float `^` — cast both sides to an integer or handle the power in a `#goose` block
- `?` try expressions and `unsafe` blocks — handle the error explicitly or use a `#goose` block
- raw pointer dereferences, if-expressions and loop-expressions in value position
- more than one range per `loop`, and array types with non-literal lengths

A `#goose` block may contain any valid Goose needed by the program, but Poly treats it as opaque text and does not type-check
its declarations. Invalid or incompatible Goose is reported by `goose check` during `--check` or by `goose run`.

## Diagnostics and Validation

`poly --target goose --check file.poly` transpiles the file and runs the generated source through `goose check` in a private
temporary directory. The `goose` compiler must be on `PATH`; set `POLY_GOOSE` when it is named differently. Without the
toolchain the check fails with a `goose compilation error` naming the missing binary, while
`poly --target goose program.poly` still emits `main.goose` and warns (`Warning: Goose typecheck unavailable`, then
`Warning: could not run goose`) instead of failing, so emission stays usable on machines without the interpreter.

`poly --target goose program.poly` executes the generated artifact with `goose run` and streams the program's output.
`--project build-dir` generates the standalone layout — the emitted `.goose` artifact, the original Poly source, and a
README — because there is no build system to generate; `goose run main.goose` runs the project.

The Goose backend is a preview subset. Its source of truth is the implementation in
`compiler/crates/poly-goose-codegen/src/lib.rs`, the frozen [v2 support matrix](POLY_V2_SUPPORT_MATRIX.md), the snapshots in
`compiler/crates/poly-transpiler/tests/snapshots/` (including `fixture_goose_target_tests.snap`), and the integration coverage
in `compiler/crates/poly-transpiler/tests/capability_parity.rs`,
`compiler/crates/poly-transpiler/tests/doc_claim_guards.rs`, and `tests/goose_target_tests.poly`.
