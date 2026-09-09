# Poly C11 Backend

**Status:** Current for Poly 2.0.0-preview.2

This document describes the implemented C target. It is not a proposal for a future general-purpose C translator.

## Target Model

Poly lowers its orchestration statements to a C11 translation unit. A selected `#c ... #endc` block is copied verbatim at file scope, then Poly-generated code is placed in `int main(void)`. The C compiler validates the copied declarations and any calls from `main`.

~~~poly fragment
#c
#include <stdint.h>

int double_value(int value) {
    return value * 2;
}
#endc

var answer i32 := double_value(21)
put answer
~~~

Select the backend explicitly when a file contains multiple foreign blocks:

~~~bash
poly --target c --check program.poly
poly --target c --emit-c program.poly
poly --target c program.poly
poly --target c --project build-dir program.poly
~~~

Only `#c` blocks are emitted for the C target. `#rust` blocks are ignored by C code generation. `extern c fn ...` declarations are checker-only interface contracts and are not emitted. `#cpp` is rejected explicitly because no C++ backend exists.

## Supported Poly Surface

The C backend currently supports:

- scalar variable, `let`, and `const` declarations
- primitive C-compatible types: booleans, signed/unsigned integers through `i64`/`u64`, `f32`, `f64`, `char`, `string`, and `ustring`
- plain structs without methods or generics, plus struct literals lowering to C99 compound literals
- enum declarations emitting `typedef enum` with qualified enumerators; simple variant values and match patterns (payload-carrying variants are rejected)
- tuple types (anonymous structs with `_N` fields), `.N` index access, nested tuples, and printing of tuple indexes
- simple Poly functions without async or generics
- assignments, arithmetic, comparisons, logical and bitwise operators
- `if`/`else`, `while`, inclusive numeric `loop` ranges, infinite loops, and `break`/`continue`
- `match` statements lowered to an if/else-if chain over a match-scoped `const` copy of the scrutinee; literal, wildcard, range, and identifier arms are supported (structured patterns and guards are rejected with guidance)
- non-capturing closures compiled to a static function plus a typed function pointer (capturing closures are rejected with guidance)
- calls to functions defined in `#c` blocks
- `put`, `error`, `warn`, and `info` for scalar values and string concatenation in output expressions
- plain `get` (with optional prompt) reading a line of stdin through an emitted runtime helper

A C target source must keep unsupported operations in a `#c` helper and call that helper from supported Poly orchestration, or use the Rust target instead.

## Type Mapping

| Poly | C11 |
|---|---|
| `bool` | `bool` |
| `i8`/`u8` | `int8_t`/`uint8_t` |
| `i16`/`u16` | `int16_t`/`uint16_t` |
| `i32`/`u32` | `int32_t` |
| `i64`/`u64` | `int64_t`/`uint64_t` |
| `f32`/`f64` | `float`/`double` |
| `char`/`uchar` | `char` |
| `string`/`ustring` | `const char *` |
| `usize` | `size_t` |

The mapping is intentionally shallow. Poly does not add ownership, bounds checking, allocation, or Unicode normalization to C values. The C program is responsible for the lifetime and encoding rules of foreign data.

## Unsupported Features

The C backend rejects these Poly constructs with diagnostics:

- file input, typed input flags (`--as`, `--bytes`, `--timeout`, `--mask`, `--until`), and `get ... from ...`
- `put ... to ...` redirects
- vectors, arrays, maps, sets, and option/result values
- capturing closures, method calls, async, generic functions, and generic structs
- payload-carrying enum variants and structured/guarded match patterns
- file I/O flags and Rust-only runtime builtins

A `#c` block may contain any valid C needed by the program, but Poly still treats it as opaque text and does not type-check its declarations. Invalid or incompatible C is reported by `cc`/`clang` during `--check` or build.

## Diagnostics and Validation

`poly --target c --check file.poly` runs the generated source through the available C compiler with C11 syntax and warning checks. The CLI build path emits a `.c` file and invokes the system C compiler. CI requires a C compiler, runs the checked-in C fixture, executes it, and exercises C project generation.

The C backend is a preview subset. Its source of truth is the implementation in `compiler/crates/poly-c-codegen/src/lib.rs`, the frozen [v2 support matrix](POLY_V2_SUPPORT_MATRIX.md), and the integration coverage in `compiler/crates/poly-cli/tests/integration_tests.rs` and `tests/c_target_tests.poly`.

## Cross-Platform Compiler Selection

The CLI looks for a C compiler in this order:

- `POLY_CC`, when set, is used exactly as configured.
- On Windows, `gcc`, then `clang`, then `cc`.
- On Unix-like systems, `cc`, then `clang`, then `gcc`.

The selected compiler must accept C11 flags. CI sets `POLY_CC` explicitly so the platform toolchain is visible in the test logs. Local users can set `POLY_CC` when the compiler is not named `cc`.
