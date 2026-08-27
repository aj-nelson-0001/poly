# Poly v2 Cheatsheet

This page describes the maintained v2 preview contract. The [documentation index](POLY_DOCUMENTATION_INDEX.md) identifies historical guides that may contain older syntax.

## Core Syntax

~~~poly
var count i32 := 0
let greeting: ustring := unicode "Hello"
const limit := 3

loop i 0..limit
    add count
end loop

if count = 4
    put greeting
else
    warn unicode "Unexpected count"
end if

put "count = " + count
~~~

Declarations and assignments use `:=`. Equality uses `=`. The legacy `==` spelling is rejected. `put` always writes a newline; there is no `-n` flag.

## Output

~~~poly
put unicode "stdout"
error unicode "error message"
warn unicode "warning message"
info unicode "diagnostic message"
put unicode "replace" to "output.txt"
put unicode "append" to "output.txt" -append
~~~

`error`, `warn`, and `info` write to stderr with `[ERROR]`, `[WARN]`, and `[INFO]` prefixes. File redirects are implemented by the Rust backend; the C backend reports an actionable unsupported-feature error.

## Input: Rust Backend

~~~poly
var line ustring := get
var prompted ustring := get unicode "Name: "
var number i32 := get --as i32
var defaulted ustring := get --default unicode "anonymous"
var password ustring := get --mask unicode "*"
var field ustring := get --until unicode ","
var header bytes := get from "data.bin" --bytes 8
~~~

`get` is a Rust-backend feature. `--timeout`, `--default`, `--mask`, `--until`, `--bytes`, and `--as` are parsed as structured flags; the generated Rust runtime implements the supported forms. The C backend rejects `get`.

## Loops

~~~poly
loop i 0..5
    put i
end loop

loop i 10..1 step -1
    put i
end loop

var values := [10, 20, 30]
loop value in values
    put value
end loop

loop (index, value) in values.enumerate()
    put index
    put value
end loop
~~~

Range endpoints are inclusive. Collection loops borrow the collection. The C backend supports one numeric range per loop and scalar array iteration; complex collection types may require a `#c` helper.

## Foreign Blocks

~~~poly
#rust
fn double_value(value: i32) -> i32 {
    value * 2
}
#endrust

var result i32 := double_value(21)
put result
~~~

~~~poly
#c
int double_value(int value) { return value * 2; }
#endc

var result i32 := double_value(21)
put result
~~~

Foreign blocks are emitted to the selected target at file scope. The target compiler validates their contents. `#cpp` is rejected explicitly; there is no C++ backend yet.

## Targets

~~~bash
poly --target rust --check program.poly
poly --target c --check program.poly
poly --emit-rust program.poly
poly --target c --emit-c program.poly
~~~

Rust is the default target. The C target is a C11 orchestration subset and supports scalar declarations, calls to foreign C helpers, numeric control flow, and stdout/stderr output. Unsupported C features fail with a diagnostic instead of being silently rewritten.

## Common Diagnostics

- Use `:=` to initialize or assign; use `=` to compare.
- Close blocks with their matching form, such as `end if`, `end loop`, or `end fn`.
- Put foreign blocks at program scope.
- Use `put value to "file" -append` for Rust file append output.
- Use a target-language helper in `#rust` or `#c` when the Poly subset does not express the operation.
