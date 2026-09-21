# Poly JavaScript Backend

**Status:** Current for Poly 2.0.0-preview.14

This document describes the implemented JavaScript target. It is not a proposal for a future general-purpose JS translator.

## Target Model

Poly lowers its orchestration statements to an ES2020 JavaScript module. A selected `#js ... #endjs` block is copied verbatim at module scope (before the generated code), then Poly-generated declarations and a `function main() { ... }` wrapper are emitted, followed by a `main();` call. Node.js and browsers validate the copied declarations and any calls from `main`.

~~~poly fragment
#js
function double_value(n) {
    return n * 2;
}
#endjs

var answer i32 := double_value(21)
put answer
~~~

Select the backend explicitly when a file contains multiple foreign blocks:

~~~bash
poly --target js --check program.poly
poly --target js --emit-js program.poly
poly --target js program.poly
poly --target js --project build-dir program.poly
~~~

Only `#js` blocks are emitted for the JS target. `#rust`, `#c`, and `#asm` blocks are ignored by JS code generation. `extern js fn ...` declarations are checker-only interface contracts and are not emitted. The generated code has no runtime dependencies and runs on any ES2020 engine (Node.js 14+, modern browsers).

## Supported Poly Surface

The JS backend currently supports:

- scalar variable, `let`, and `const` declarations
- primitive JS-compatible types: booleans, integers (emitted as `Number`; `/` on integers is wrapped in `Math.trunc` to preserve Poly's truncated division), `f32`/`f64`, `string`, and `ustring`
- simple Poly functions without async or generics, emitted as top-level `function` declarations
- plain structs without methods or generics, emitted as factory-style constructor functions, with struct literals lowering to `new Point({ x: 3, y: 4 })`
- tuple literals and typed tuple declarations (arrays with `.N` index access)
- assignments, arithmetic, comparisons, logical and bitwise operators
- `if`/`else`, `while`, inclusive numeric `loop` ranges (with optional `step`), infinite loops, and `break`/`continue`
- `for x in <iterable>` and `loop x in <collection>` lowering to `for (const x of ...)`
- `match` statements lowered to a `switch` over the scrutinee; `match` expressions in value position lower to an IIFE `switch` (literal, wildcard, and unit-enum patterns)
- calls to functions defined in `#js` blocks
- `put` for scalar values and string concatenation in output expressions (numbers print through `String(...)`; strings print directly); string interpolation lowers to concatenation

A JS target source must keep unsupported operations in a `#js` helper and call that helper from supported Poly orchestration, or use the Rust target instead.

## Rejected Constructs

Unsupported constructs are rejected with guidance rather than miscompiled:

- enum declarations, payload-carrying enum variants, and structured/guarded match patterns
- async/await and generators
- closures (capturing and non-capturing)
- file I/O (`to "file"`, `-append`) and all stdin `get` input (browser-hostile)
- vectors and arrays with backend-specific operations beyond indexing/iteration
- method calls, `try` propagation, `if` expressions in value position, and unsafe blocks

## Type Mapping

| Poly | JavaScript |
|---|---|
| `bool` | `boolean` |
| `i8`..`i64`/`u8`..`u64` | `number` (integer division via `Math.trunc`) |
| `f32`/`f64` | `number` |
| `string`/`ustring` | `string` |
| `char` | single-character `string` |
| `(T, U)` tuple | array with `.N` index access |

## Strict Mode

`poly --target js --check --strict program.poly` rejects calls to foreign functions without an explicit `extern js fn name(param: type): type` declaration. Default (permissive) checking defers call validation to the JS engine.

~~~poly fragment
extern js fn fetch_score(id: i32): i32
#js
function fetch_score(id) {
    return id * 10;
}
#endjs

put fetch_score(7)
~~~
