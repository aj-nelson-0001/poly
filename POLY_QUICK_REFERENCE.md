# Poly v2 Quick Reference

This is a current v2 reference. Historical syntax belongs in the migration documents listed by [POLY_DOCUMENTATION_INDEX.md](POLY_DOCUMENTATION_INDEX.md).

## Declarations and Operators

~~~poly
fn main()
    var count i32 := 0                        # mutable; the type is written without a colon
    var name ustring := unicode "Poly"        # inference form: var name := value
    let label: ustring := unicode "current"   # let is immutable; its type uses a colon
    const limit := 3                          # untyped consts infer i32
    count := count + 1                        # := assigns; = compares

    if count = limit
        put name
    else
        warn unicode "not finished"           # writes a [WARN] line to stderr
    end if
end fn
~~~

Use `:=` for initialization and assignment. Use `=` for equality. `==` is rejected. Logical, bitwise, and remainder operators are keyword-spelled: `and`, `or`, `not`, `xor`, `mod`, `bitand`, `bitor`, `bitnot`, and `shift left` / `shift right` (`<<` and `>>` remain valid). `and`/`or`/`not` take booleans; `xor`, `bitand`, `bitor`, `bitnot`, and the shifts are integer operations (the checker rejects them on `bool` operands). The retired symbol spellings `&&`, `||`, `!`, `^`, `%`, `&`, `|`, `~` are rejected with migration diagnostics. `var` writes an optional type without a colon; `let` type annotations use a colon.

## Output

~~~poly
fn main()
    put unicode "stdout"                       # stdout, always with a trailing newline
    error unicode "error"                      # [ERROR] prefix, stderr
    warn unicode "warning"                     # [WARN] prefix, stderr
    info unicode "diagnostic"                  # [INFO] prefix, stderr
    put unicode "replace" to "output.txt"      # write (truncate)
    put unicode "append" to "output.txt" -append   # keep contents, add to the end
end fn
~~~

`put` always adds a newline. Rust supports file redirects. C supports stdout/stderr output and rejects file redirects with a diagnostic.

## Rust Input

~~~poly
fn main()
    var line ustring := get                        # reads one line from stdin
    var prompted ustring := get unicode "Name: "    # inline prompt
    var number i32 := get --as i32                 # parse the line as an integer
    var fallback ustring := get --default unicode "anonymous"   # used on empty input
    var hidden ustring := get --mask unicode "*"   # echo * instead of typed keys
    var field ustring := get --until unicode ","   # stop at the delimiter (excluded)
    var header bytes := get from "input.bin" --bytes 8   # read 8 bytes from a file
end fn
~~~

`get` and its input flags are Rust-backend features. The C backend supports plain `get` — with an optional prompt — reading a line of stdin through an emitted runtime helper; file sources and input flags require a target-language helper in `#c`.

## Control Flow

~~~poly
fn main()
    loop i 0..5                    # endpoints inclusive: 0 through 5
        put i
    end loop

    loop i 10..1 step -1           # negative step counts down
        put i
    end loop

    var values := [10, 20, 30]
    loop value in values           # yields each element in order
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

# Foreign blocks are opaque to Poly and emitted verbatim
# for the selected target.

fn main()
    var result i32 := double_value(21)   # call into the foreign definition
    put result
end fn
~~~

~~~poly
#c
int double_value(int value) { return value * 2; }
#endc

fn main()
    var result i32 := double_value(21)   # requires --target c
    put result
end fn
~~~

Foreign blocks must be at program scope and are emitted verbatim for the selected target. Add `extern rust fn ...` or `extern c fn ...` at program scope when Poly-side argument and return checks are useful; declarations are not emitted. `#cpp` is rejected because no C++ backend exists.

## CLI

~~~bash
poly --target rust --check program.poly    # parse, check, transpile, compile
poly --target c --check program.poly
poly --emit-rust program.poly              # print the generated Rust source
poly --target c --emit-c program.poly      # print the generated C source
poly --tokens program.poly                 # show lexer tokens
poly --ast program.poly                    # show the parsed AST
~~~

Rust is the default target. `--check` runs parsing, semantic checking, transpilation, and native compilation for the selected target.

## Common Fixes

- Close blocks with `end if`, `end loop`, `end while`, or `end fn`.
- Keep `#rust` and `#c` blocks at top level.
- Use `put value to "file" -append`, not legacy redirection operators.
- Move unsupported target-specific work into a foreign helper and call it from Poly.
- Annotate Poly code with `#` comments; `//` and `/* ... */` are also accepted.

## Comments

~~~poly
# A hash comment runs from `#` to the end of the line.
// A C++-style line comment also runs to the end of the line.
/* A block comment can span
   several lines. */

fn main()
    var attempts i32 := 0    # A comment may trail a line of code
    put attempts
end fn
~~~

`#` starts a comment unless it names a foreign block: a line beginning `#rust`,
`#c`, `#asm`, or `#js` opens a foreign block instead. Comments inside a foreign
block belong to the target language and are copied verbatim.
