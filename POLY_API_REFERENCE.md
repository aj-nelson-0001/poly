# Poly v2 API Reference

This reference documents syntax implemented by the current compiler. Backend-specific behavior is called out explicitly.

## Declarations and Assignment

~~~poly
var count i32 := 0
var inferred := 42
let name: ustring := unicode "Alice"
const limit := 10
count := count + 1
if count = limit
    put unicode "done"
end if
~~~

`:=` initializes declarations and assigns existing bindings. `=` compares values. The legacy `==` spelling is rejected. `var` types are written without a colon; `let` type annotations use a colon.

## `put`

~~~poly
put unicode "Hello"
put 42
put "value = " + 42
put unicode "replace" to "output.txt"
put unicode "append" to "output.txt" -append
~~~

`put` writes to stdout and always adds a newline. Rust supports `to "file"` and `-append`. The C backend supports stdout output but rejects file redirects; use a `#c` helper for C file I/O.

## `error`, `warn`, and `info`

~~~poly
error unicode "fatal diagnostic"
warn unicode "non-fatal diagnostic"
info unicode "debug diagnostic"
~~~

All three commands write to stderr. The generated Rust and C backends prefix the messages with `[ERROR]`, `[WARN]`, or `[INFO]`.

## `get` and Input Flags

`get` is a Rust-backend input operation. The C backend rejects it.

~~~poly
var line ustring := get
var prompted ustring := get unicode "Name: "
var from_file ustring := get from "input.txt"
var number i32 := get --as i32
var fallback ustring := get --default unicode "fallback"
var hidden ustring := get --mask unicode "*"
var field ustring := get --until unicode ","
var header bytes := get from "input.bin" --bytes 8
~~~

Supported flags:

| Form | Meaning |
|------|---------|
| `--as TYPE` | Convert input to the requested type |
| `--default VALUE` | Use a fallback when input is empty |
| `--mask VALUE` | Suppress terminal echo on supported Rust Unix terminals |
| `--until VALUE` | Read through stdin until the delimiter or EOF |
| `--bytes N` | Read a fixed number of bytes from a file |
| `--timeout N` | Rust input timeout behavior; target support is backend-specific |

The parser also represents `with validate`, `with complete`, and `with encoding` clauses for compatibility with the broader language model, but they are not part of the maintained runnable v2 API. Do not use them in current examples.

## Functions and Control Flow

~~~poly
fn double(value: i32): i32
    return value * 2
end fn

var value i32 := double(21)

if value > 20
    put value
else
    warn unicode "small value"
end if

loop i 0..3
    put i
end loop

loop i 10..1 step -1
    put i
end loop
~~~

Range endpoints are inclusive. `while condition ... end while` is supported by the parser and lowers to a loop. `break` and `continue` are valid inside loops.

Collection iteration:

~~~poly
var values := [10, 20, 30]
loop value in values
    put value
end loop

loop (index, value) in values.enumerate()
    put index
    put value
end loop
~~~

## Foreign Blocks

Foreign blocks are selected by target and emitted at target-language scope. Keep them at program scope.

~~~poly
#rust
fn native_value() -> i32 { 42 }
#endrust

var value i32 := native_value()
put value
~~~

~~~poly
#c
int native_value(void) { return 42; }
#endc

var value i32 := native_value()
put value
~~~

`#cpp` is explicitly rejected. Foreign calls without declarations remain opaque to Poly semantic checking; Rust or C validates the actual call after generation. Add an optional target-aware declaration when you want Poly to check the interface first:

~~~poly
extern c fn native_value(value: i32): i32
~~~

The declaration is checker-only, must match the selected target, and does not replace native compiler validation.

## Targets and CLI

~~~bash
poly --target rust --check program.poly
poly --target c --check program.poly
poly --emit-rust program.poly
poly --target c --emit-c program.poly
poly --tokens program.poly
poly --ast program.poly
poly --intermediate-representation program.poly
~~~

Rust is the default target. The C backend currently supports scalar declarations, numeric expressions and control flow, calls to foreign C helpers, and stdout/stderr diagnostics. It rejects Rust-specific input, file redirects, tuples, complex collection operations, and unsupported expression forms with diagnostics. See [POLY_V2_SUPPORT_MATRIX.md](POLY_V2_SUPPORT_MATRIX.md) for the frozen target contract.

## Errors and Results

The parser and semantic checker report errors with source context. Target-native errors are returned after the generated target source is compiled. `Result` and pattern matching remain valid Poly model constructs, but only the Rust backend currently provides the complete support expected for them.

Use `--check` when validating a program; use `--emit-rust` or `--emit-c` to inspect the generated target source.
