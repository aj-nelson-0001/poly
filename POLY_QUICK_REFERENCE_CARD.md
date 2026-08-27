# Poly 2.0 Preview Quick Reference

This card describes the current v2 preview. See [POLY_SPEC_v2.md](POLY_SPEC_v2.md) for the full contract.

## Declarations and Assignment

~~~poly
var count i32 := 0
var message ustring := unicode "hello"
let limit: i32 := 10
const MAX := 100
count := count + 1
if count = MAX
    put "done"
end if
~~~

`:=` assigns. `=` compares. The legacy `==` spelling is rejected.

## Output and Input

~~~poly
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
~~~

`put` always adds a newline. Diagnostic commands write `[ERROR]`, `[WARN]`, or `[INFO]` to stderr. File redirects and the input flags above are implemented by the Rust target; the C target currently rejects redirects and stdin.

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
fn add(a: i32, b: i32): i32
    return a + b
end fn

struct Point
    var x: i32
    var y: i32
end struct

var result := add(2, 3)
put result
~~~

The Rust target supports the broader parser/checker feature set, including enums, traits, modules, closures, generic types, async functions, matches, and collections. The C target supports scalar orchestration, simple functions, plain structs, arithmetic, conditions, loops, and stdout/stderr.

## Foreign Blocks and Targets

~~~poly
#rust
fn helper(x: i32) -> i32 { x * 2 }
#endrust

#c
int helper(int x) { return x * 2; }
#endc

var result i32 := helper(21)
put result
~~~

Foreign blocks must be top-level and their contents are copied as target-language text. Only the block matching the selected target is emitted. Optional `extern rust fn ...` and `extern c fn ...` declarations are top-level checker-only interface contracts.

~~~bash
poly --target rust program.poly
poly --target c --check program.poly
poly --target c --emit-c program.poly
poly --target c program.poly
~~~

`#cpp` is reserved and rejected until a C++ backend exists. Set `POLY_CC` when the C compiler is not named `cc`.

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
| `poly --tokens file.poly` | Print lexer tokens |
| `poly --ast file.poly` | Print the parsed AST |
| `poly --ir file.poly` | Print optimized Rust IR output |
| `poly --source-map file.poly` | Print the best-effort source map summary |
| `poly --repl` | Start the interactive REPL |
