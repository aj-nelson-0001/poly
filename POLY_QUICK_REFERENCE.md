# Poly v2 Quick Reference

This is a current v2 reference. Historical syntax belongs in the migration documents listed by
[POLY_DOCUMENTATION_INDEX.md](POLY_DOCUMENTATION_INDEX.md).

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

Use `:=` for initialization and assignment. Use `=` for equality. `==` is rejected. `add x, n` / `sub x, n` are
mutation commands that add/subtract `n`, lowering to `x := x ± n`; the comma is required (`add x n` is rejected).
`inc x` / `dec x` are operand-free mutation commands lowering to `x := x ± 1`. Logical, bitwise, and remainder
operators are keyword-spelled: `and`, `or`, `not`, `xor`, `mod`, `bitand`, `bitor`, `bitnot`, and
`shift left` / `shift right` (`<<` and `>>` remain valid). `and`/`or`/`not` take booleans; `xor`, `bitand`,
`bitor`, `bitnot`, and the shifts are integer operations (the checker rejects them on `bool` operands). The
retired symbol spellings `&&`, `||`, `!`, `%`, `&`, `|`, `~` are rejected with migration diagnostics. `^` is
**exponentiation** (`x ^ 2` is `x` squared): it binds tighter than `*`, associates to the right
(`2 ^ 3 ^ 2` is `2 ^ (3 ^ 2)`), and requires matching numeric classes (cast one side with `as` when mixing int
and float). `var` writes an optional type without a colon; `let` type annotations use a colon.

## Output

~~~poly
fn main()
    put unicode "stdout"                       # stdout, always with a trailing newline
    error unicode "error"                      # [ERROR] prefix, stderr
    warn unicode "warning"                     # [WARN] prefix, stderr
    info unicode "diagnostic"                  # [INFO] prefix, stderr
    put unicode "replace" to "output.txt"      # write (truncate)
    put unicode "append" to "output.txt" -append   # keep contents, add to the end
    put unicode "prepend" to "output.txt" -prepend # insert in front of the contents
end fn
~~~

`put` always adds a newline; there is no `-n` flag. Rust supports file redirects, including `-append` and
`-prepend`. C supports stdout/stderr output and rejects file redirects with a diagnostic.

## Input

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

`get` and its input flags are Rust-backend features; `--timeout`, `--default`, `--mask`, `--until`, `--bytes`,
and `--as` are parsed as structured flags and the generated Rust runtime implements the supported forms. The C
backend supports plain `get` — with an optional prompt — reading a line of stdin through an emitted runtime
helper; file sources, input flags, and `with` clauses require a target-language helper in `#c`.

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

    loop (index, value) in values.enumerate()   # (position, element) pairs from 0
        put index
        put value
    end loop
end fn
~~~

Range endpoints are inclusive. `while condition ... end while`, `break`, and `continue` are also supported. The
range start may be any expression (`loop i BUF..TOTAL - 1`); a var-less `loop 0..10` discards the counter. The
C backend supports one numeric range per loop and a limited scalar collection form; complex collection types
may require a `#c` helper. `spawn` and `step` are reserved words and cannot name variables or methods.

## Methods, Movement, and Comparison

Poly moves values into calls. Methods that mutate state take and return `self`, and callers reassign:

~~~poly fragment
s := s.bump()
s := s.reset(10)
~~~

Inside a method, copy `self` fields to locals before passing them to another method of the same value —
`self.shift(self.x)` moves `self` before the argument is evaluated:

~~~poly fragment
fn shift(self, dx: i32): Grid
    var x i32 := self.x
    self := self.move(x, dx)
    return self
end fn
~~~

Comparisons are strictly same-type: `u64 = i64` or `f64 = i64` is a type error. Cast explicitly with `as`
(`bits as i64 = row`), and remember untyped consts infer `i32` — annotate or cast when assigning into a
differently typed variable (`var bits u64 := FONT_A as u64`).

## Strings

~~~poly
fn main()
    var name ustring := "Ada"
    var n i32 := 42

    # Concatenation works in put position and value position on every target.
    put "Hello, " + name + "!"
    var greeting ustring := "Hi " + name

    # Scalar to strings: to_string() on integers, floats, and bools.
    var labelled ustring := "n=" + n.to_string()
    put labelled

    # .len() is the byte length of the string.
    put name.len()

    # Interpolation: {expr} splices values directly into a string.
    put "n is {n}, name is {name}"
end fn
~~~

Rust lowers `s := s + x` in loops to in-place `push_str` (amortized O(1)). C materializes concat with an
emitted `poly_concat` helper and `.len()` via `strlen`; asm uses a static bump arena (`_str_arena`) for concat
results. The JS target maps everything to native string operations. The Goose target lowers concat to
`str(...)` and supports integer `.to_string()`, `.len`, and `.push`; other method calls are rejected with a
`#goose` hint. Interpolation lowers to concatenation on every target (C formats through `printf`).

## External Crates

~~~poly fragment
dep minifb = "0.27"   # program-scope Cargo crates for the Rust target
dep alsa = "0.9"
~~~

Program-scope `dep name = "version"` declarations name the Cargo crates the foreign blocks use. `--project`
emits them into `Cargo.toml`; `--check` resolves them through Cargo so the program validates standalone. They
are ignored by the C, asm, JS, and Goose backends. `--project` also writes a `.poly-generated` marker
recording the owning source path relative to the generated project (e.g. `source=../../tetris.poly`); a
default build only refreshes directories whose marker resolves to the source being built.

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

Foreign blocks must be at program scope and are emitted verbatim for the selected target. The block kinds are
`#rust`, `#c`, `#asm`, `#js`, and `#goose`. Add `extern rust fn ...` or `extern c fn ...` at program scope
when Poly-side argument and return checks are useful; declarations are not emitted. `#cpp` is rejected
because no C++ backend exists.

The Rust target supports the broader parser/checker feature set, including enums, traits, modules, closures,
generic types, async functions, matches, and collections. The C target supports scalar orchestration, simple
functions, plain structs, arithmetic, conditions, loops, and stdout/stderr. The Goose target is a preview
subset that follows the JS surface plus vector literals and mutators — see
[POLY_GOOSE_BLOCKS.md](POLY_GOOSE_BLOCKS.md). The same helper on the Goose target uses `#goose` /
`#endgoose` and an `extern goose fn ...` declaration. Set `POLY_CC` when the C compiler is not named `cc`,
and `POLY_GOOSE` when the Goose compiler is not named `goose`.

## CLI

~~~bash
poly --target rust --check program.poly    # parse, check, transpile, compile
poly --target c --check program.poly
poly --emit-rust program.poly              # print the generated Rust source
poly --target c --emit-c program.poly      # print the generated C source
poly --target asm --emit-asm program.poly  # print the generated x86-64 assembly
poly --target js --emit-js program.poly    # print the generated JavaScript
poly --target goose --emit-goose program.poly  # print the generated Goose source
poly --tokens program.poly                 # show lexer tokens
poly --ast program.poly                    # show the parsed AST
~~~

Rust is the default target. `--check` runs parsing, semantic checking, transpilation, and native compilation
for the selected target.

| Command | Effect |
|---|---|
| `poly file.poly` | Generate and build a Rust Cargo project |
| `poly --check file.poly` | Check the default Rust target |
| `poly --target c --check file.poly` | Check generated C with the selected C11 compiler |
| `POLY_CC=clang poly --target c --check file.poly` | Select `clang` explicitly for C checks |
| `poly --emit-rust file.poly` | Print Rust output |
| `poly --target c --emit-c file.poly` | Print C output |
| `poly --project DIR file.poly` | Generate a Rust project |
| `poly --target c --project DIR file.poly` | Generate and build a C project |
| `poly --target goose --emit-goose file.poly` | Print Goose output |
| `poly --target goose --check file.poly` | Check generated Goose with `goose check` |
| `poly --tokens file.poly` | Print lexer tokens |
| `poly --ast file.poly` | Print the parsed AST |
| `poly --ir file.poly` | Print optimized Rust IR output |
| `poly --source-map file.poly` | Print the best-effort source map summary |
| `poly --repl` | Start the interactive REPL |

## Common Fixes

- Use `:=` to initialize or assign; use `=` to compare.
- Close blocks with `end if`, `end loop`, `end while`, or `end fn`.
- Keep `#rust`, `#c`, `#asm`, `#js`, and `#goose` blocks at top level.
- Use `put value to "file" -append` (or `-prepend`), not legacy redirection operators.
- Depth-limit messages (`Maximum nested ... depth`) mean generated-style nesting; flatten the structure
  instead of re-chaining `else if` arms.
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
`#c`, `#asm`, `#js`, or `#goose` opens a foreign block instead. Comments inside a
foreign block belong to the target language and are copied verbatim.
