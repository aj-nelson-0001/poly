# Poly 2.0 Preview Quick Reference

This card describes the current v2 preview. See [POLY_SPEC_v2.md](POLY_SPEC_v2.md) for the full contract.

## Declarations and Assignment

~~~poly
fn main()
    var count i32 := 0
    var message ustring := unicode "hello"
    let limit: i32 := 10
    const MAX := 100
    count := count + 1
    if count = MAX
        put "done"
    end if
end fn
~~~

`:=` assigns. `=` compares. The legacy `==` spelling is rejected.

## Output and Input

~~~poly
fn main()
    put "hello"
    put "log entry" to "app.log"
    put "more" to "app.log" -append
    error "failure"
    warn "warning"
    info "details"

    var name ustring := get
    var age i32 := get --as i32
    var field ustring := get --until unicode ","
    var password ustring := get --mask unicode "*"
    var header bytes := get from "data.bin" --bytes 8
end fn
~~~

`put` always adds a newline. Diagnostic commands write `[ERROR]`, `[WARN]`, or
`[INFO]` to stderr. File output redirects and the typed input flags above are
Rust-only; the C target supports plain `get` (with an optional prompt) via an
emitted runtime helper, and needs a `#c` helper for file input, the typed
flags, and file output redirects.

## Control Flow

~~~poly fragment
if count > 0
    put count
else
    warn "empty"
end if

while count < 10
    count := count + 1
end while

loop i 1..5
    put i
end loop

loop i 10..1 step -2
    put i
end loop

var items := [10, 20, 30]
loop item in items
    put item
end loop
~~~

Loop ranges include both endpoints. Collection loops borrow their source collection.

## Functions and Data

~~~poly
fn sum(a: i32, b: i32): i32
    return a + b
end fn

struct Point
    var x: i32
    var y: i32
end struct

fn main()
    var result := sum(2, 3)
    put result
end fn
~~~

The Rust target supports the broader parser/checker feature set, including enums,
traits, modules, closures, generic types, async functions, matches, and
collections. The C target supports scalar orchestration, simple functions, plain
structs, arithmetic, conditions, loops, and stdout/stderr. The Goose target is a
preview subset that follows the JS surface plus vector literals and mutators —
see [POLY_GOOSE_BLOCKS.md](POLY_GOOSE_BLOCKS.md).

## Foreign Blocks and Targets

The two blocks below show the same helper for each target; keep only the
block matching your selected target in a real program.

~~~poly
#rust
fn helper(x: i32) -> i32 { x * 2 }
#endrust

fn main()
    var result i32 := helper(21)
    put result
end fn
~~~

~~~poly fragment
#c
int helper(int x) { return x * 2; }
#endc

fn main()
    var result i32 := helper(21)
    put result
end fn
~~~

Foreign blocks must be top-level and their contents are copied as target-language
text. Only the block matching the selected target is emitted. Optional `extern
rust fn ...` and `extern c fn ...` declarations are top-level checker-only
interface contracts.

~~~bash
poly --target rust program.poly
poly --target c --check program.poly
poly --target c --emit-c program.poly
poly --target c program.poly
poly --target goose --check program.poly
poly --target goose --emit-goose program.poly
poly --target goose program.poly
~~~

The same helper on the Goose target uses `#goose` / `#endgoose` and an
`extern goose fn ...` declaration.

`#cpp` is reserved and rejected until a C++ backend exists. Set `POLY_CC` when the C compiler is not named `cc`, and
`POLY_GOOSE` when the Goose compiler is not named `goose`.

## CLI

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
