# Poly v2 Quick Reference

This is a current v2 reference. Historical syntax belongs in the migration documents listed by [POLY_DOCUMENTATION_INDEX.md](POLY_DOCUMENTATION_INDEX.md).

## Declarations and Operators

~~~poly
var count i32 := 0
var name ustring := unicode "Poly"
let label: ustring := unicode "current"
const limit := 3
count := count + 1

if count = limit
    put name
else
    warn unicode "not finished"
end if
~~~

Use `:=` for initialization and assignment. Use `=` for equality. `==` is rejected. `var` writes an optional type without a colon; `let` type annotations use a colon.

## Output

~~~poly
put unicode "stdout"
error unicode "error"
warn unicode "warning"
info unicode "diagnostic"
put unicode "replace" to "output.txt"
put unicode "append" to "output.txt" -append
~~~

`put` always adds a newline. Rust supports file redirects. C supports stdout/stderr output and rejects file redirects with a diagnostic.

## Rust Input

~~~poly
var line ustring := get
var prompted ustring := get unicode "Name: "
var number i32 := get --as i32
var fallback ustring := get --default unicode "anonymous"
var hidden ustring := get --mask unicode "*"
var field ustring := get --until unicode ","
var header bytes := get from "input.bin" --bytes 8
~~~

`get` and its input flags are Rust-backend features. The C backend rejects `get`; provide input through a target-language helper in `#c`.

## Control Flow

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
~~~

Range endpoints are inclusive. `while condition ... end while`, `break`, and `continue` are also supported. C currently supports one numeric range per loop and a limited scalar collection form.

## Functions and Foreign Blocks

~~~poly
#rust
fn double_value(value: i32) -> i32 { value * 2 }
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

Foreign blocks must be at program scope and are emitted verbatim for the selected target. Add `extern rust fn ...` or `extern c fn ...` at program scope when Poly-side argument and return checks are useful; declarations are not emitted. `#cpp` is rejected because no C++ backend exists.

## CLI

~~~bash
poly --target rust --check program.poly
poly --target c --check program.poly
poly --emit-rust program.poly
poly --target c --emit-c program.poly
poly --tokens program.poly
poly --ast program.poly
~~~

Rust is the default target. `--check` runs parsing, semantic checking, transpilation, and native compilation for the selected target.

## Common Fixes

- Close blocks with `end if`, `end loop`, `end while`, or `end fn`.
- Keep `#rust` and `#c` blocks at top level.
- Use `put value to "file" -append`, not legacy redirection operators.
- Move unsupported target-specific work into a foreign helper and call it from Poly.
