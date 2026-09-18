# Poly v2 Quick Reference

This is a current v2 reference. Historical syntax belongs in the migration documents listed by [POLY_DOCUMENTATION_INDEX.md](POLY_DOCUMENTATION_INDEX.md).

## Declarations and Operators

~~~poly
fn main()
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
end fn
~~~

Use `:=` for initialization and assignment. Use `=` for equality. `==` is rejected. Logical, bitwise, and remainder operators are keyword-spelled: `and`, `or`, `not`, `xor`, `mod`, `bitand`, `bitor`, `bitnot`, and `shift left` / `shift right` (`<<` and `>>` remain valid). `and`/`or`/`not` take booleans; `xor`, `bitand`, `bitor`, `bitnot`, and the shifts are integer operations (the checker rejects them on `bool` operands). The retired symbol spellings `&&`, `||`, `!`, `^`, `%`, `&`, `|`, `~` are rejected with migration diagnostics. `var` writes an optional type without a colon; `let` type annotations use a colon.

## Output

~~~poly
fn main()
    put unicode "stdout"
    error unicode "error"
    warn unicode "warning"
    info unicode "diagnostic"
    put unicode "replace" to "output.txt"
    put unicode "append" to "output.txt" -append
end fn
~~~

`put` always adds a newline. Rust supports file redirects. C supports stdout/stderr output and rejects file redirects with a diagnostic.

## Rust Input

~~~poly
fn main()
    var line ustring := get
    var prompted ustring := get unicode "Name: "
    var number i32 := get --as i32
    var fallback ustring := get --default unicode "anonymous"
    var hidden ustring := get --mask unicode "*"
    var field ustring := get --until unicode ","
    var header bytes := get from "input.bin" --bytes 8
end fn
~~~

`get` and its input flags are Rust-backend features. The C backend rejects `get`; provide input through a target-language helper in `#c`.

## Control Flow

~~~poly
fn main()
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
end fn
~~~

Range endpoints are inclusive. `while condition ... end while`, `break`, and `continue` are also supported. The range start may be any expression (`loop i BUF..TOTAL - 1`); a var-less `loop 0..10` discards the counter. C currently supports one numeric range per loop and a limited scalar collection form.

## State and Methods

~~~poly fragment
s := s.bump()          # methods take and return self; reassign the result
~~~

`spawn` and `step` are reserved words and cannot name variables or methods.

## Functions and Foreign Blocks

~~~poly
#rust
fn double_value(value: i32) -> i32 { value * 2 }
#endrust

fn main()
    var result i32 := double_value(21)
    put result
end fn
~~~

~~~poly
#c
int double_value(int value) { return value * 2; }
#endc

fn main()
    var result i32 := double_value(21)
    put result
end fn
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
