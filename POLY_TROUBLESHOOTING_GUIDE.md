# Poly v2 Troubleshooting Guide

This guide covers failures that can be reproduced with the current compiler. For the authoritative syntax, see [POLY_GRAMMAR.md](POLY_GRAMMAR.md) and [POLY_DOCUMENTATION_INDEX.md](POLY_DOCUMENTATION_INDEX.md).

## Parse Errors

### `==` is rejected

Poly v2 uses `=` for equality and `:=` for initialization and assignment:

~~~poly
var value i32 := 1
if value = 1
    put unicode "equal"
end if
value := value + 1
~~~

The parser retains `==` only to report a migration diagnostic. Replace it rather than adding another compatibility spelling.

### Declaration initializer errors

Use the type without a colon in `var` declarations. `let` may use a type annotation with a colon:

~~~poly
var count i32 := 0
var inferred := 42
let name: ustring := unicode "Alice"
const limit := 10
~~~

Do not write `var count: i32 = 0`, `let name = ...`, or `const limit = ...` in v2 examples.

### Block closure errors

Every block has an explicit terminator:

~~~poly
fn greet(name: ustring): ustring
    return unicode "Hello, " + name
end fn

if true
    put unicode "yes"
else
    put unicode "no"
end if
~~~

The parser can recover from several errors, but a missing `end` often causes later statements to be reported in the wrong context. Fix the first diagnostic first.

## Output Problems

`put` always writes one newline. The `-n` flag and output capture syntax are not part of the maintained v2 contract.

~~~poly
put unicode "Line 1"
put unicode "Line 2"
error unicode "stderr error"
warn unicode "stderr warning"
info unicode "stderr info"
~~~

For file output, use the explicit redirect form:

~~~poly
put unicode "replace" to "output.txt"
put unicode "append" to "output.txt" -append
~~~

The Rust backend implements redirects. The C backend intentionally rejects them because its current contract is stdout/stderr orchestration only; move file behavior into a `#c` helper.

## Input Problems

`get` is implemented by the Rust backend. It reads stdin or a file and can apply the supported flags:

~~~poly
var line ustring := get
var number i32 := get --as i32
var fallback ustring := get --default unicode "fallback"
var field ustring := get --until unicode ","
var bytes_value bytes := get from "data.bin" --bytes 8
~~~

The C backend rejects `get`. Use a foreign C function for input when targeting C.

If a Rust input program appears to hang, it is waiting for stdin. Provide input through the terminal or redirect a file at the process level. Do not rely on undocumented `is_input_available`, network, or line-editor helpers.

## Type-Checking Problems

A declaration type must agree with its initializer. For numeric input, make the conversion explicit:

~~~poly
var age i32 := get --as i32
var total i32 := age + 10
~~~

Equality and ordering operators require compatible operands. String concatenation is supported for strings and scalar values in the Rust backend:

~~~poly
var name ustring := unicode "Alice"
if name = unicode "Alice"
    put unicode "matched"
end if
put unicode "Hello, " + name
~~~

Unknown functions or variables are reported by the semantic checker. Functions in a selected foreign block are registered as opaque calls; their native signature is still validated by the target compiler. When earlier Poly diagnostics are useful, add a top-level explicit declaration:

~~~poly
extern c fn double_value(value: i32): i32
~~~

The declaration must match the selected target and is not emitted. It checks the Poly-visible interface; the C or Rust compiler still validates the foreign definition and ABI.

## Foreign Blocks and Targets

Use a target-specific block at program scope:

~~~poly
#rust
fn native_value() -> i32 { 42 }
#endrust

var value i32 := native_value()
put value
~~~

Select C with `--target c` and use `#c` instead. A foreign block for another language is rejected. In particular, `#cpp` is not silently ignored and C++ is not currently supported. See [POLY_V2_SUPPORT_MATRIX.md](POLY_V2_SUPPORT_MATRIX.md) for the complete target boundary.

When a target feature is unsupported, the compiler should name the feature and suggest a foreign helper. Treat that diagnostic as a contract boundary rather than changing the generated source by hand.

## CLI Checks

~~~bash
poly --target rust --check program.poly
poly --target c --check program.poly
poly --emit-rust program.poly
poly --target c --emit-c program.poly
poly --tokens program.poly
poly --ast program.poly

# Select a compiler explicitly when `cc` is not the desired executable.
POLY_CC=clang poly --target c --check program.poly
~~~

`--check` runs lexing, parsing, semantic checking, transpilation, and the selected native compiler. `--emit-rust` and `--emit-c` show generated output without compiling it.

## Reporting a Bug

Include:

1. The exact Poly source, preferably reduced to the smallest failing program.
2. The command and target used.
3. The complete diagnostic output.
4. The generated output from `--emit-rust` or `--emit-c` when relevant.
5. Whether the same source behaves differently with the other target.

This information distinguishes parser, semantic checker, backend, and native-compiler failures quickly.
