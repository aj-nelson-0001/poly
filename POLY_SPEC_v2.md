# Poly Language Specification v2.0

*A minimal, assembly-inspired meta-language that transpiles to Rust or C.*

---

## 1. Philosophy

Poly is a **thin syntax layer over any systems language.** It handles the simple, boilerplate-heavy parts of programming with an assembly-inspired syntax. For everything else — generics, async, closures, traits, pattern matching — users provide definitions in foreign language blocks (`#rust`, `#c`, `#cpp`, etc.).

**Core principles:**
1. Poly provides the orchestration. Foreign blocks provide the definitions.
2. One Poly source file produces one complete source file in the target language.
3. No runtime. Poly adds zero overhead — it's just syntax sugar.
4. Explicit is better than implicit.
5. Language-agnostic: Poly can target Rust, C, C++, or others.

---

## 2. Program Structure

A Poly program is a single `.poly` file containing two kinds of sections:

- **Poly sections** — syntax that the Poly transpiler transforms into the target language
- **Foreign blocks** — definitions (functions, structs, enums) in the target language

~~~poly
# Foreign block: provides callable definitions
#rust
use tokio::net::TcpListener;

async fn run_server(port: u32) {
    let listener = TcpListener::bind(format!("0.0.0.0:{}", port)).await.unwrap();
    loop {
        let (socket, _) = listener.accept().await.unwrap();
        tokio::spawn(async move {
            // handle connection
        });
    }
}
#endrust

# Poly: orchestrates the calls
var port i32 := 8080
put "Server starting on port " + port
run_server(port as u32)
~~~

The same model covers `--target c` with `#c` blocks and `extern c fn`
declarations, and `--target asm` with `#asm` blocks and `extern asm fn`
declarations.

### Foreign Blocks

Delimited by `#<lang>` and `#end<lang>`. The content contains target-language definitions at file scope — for example functions, structs, enums, constants, type aliases, impl blocks, traits, use statements, and required helper definitions. Poly does not parse or rewrite the content; the selected native compiler validates it.

~~~poly fragment
#rust
fn complex_function(x: &str) -> Result<Vec<u8>, io::Error> {
    // This is a function definition — Poly can call it
    todo!()
}

struct Config {
    width: u32,
    height: u32,
}
#endrust
~~~

**Rules for foreign blocks:**
- Must appear at the top level (not inside Poly function bodies)
- Content is emitted at module scope in the target language
- Poly generates `fn main()` which calls into these definitions
- No executable statements — only declarations
- Multiple blocks for the selected language are allowed; they are emitted in order.
- `#rust` is selected by `--target rust`; `#c` is selected by `--target c`.
- `#cpp` is reserved for a future backend; any source containing a `#cpp` block is rejected with an explicit unsupported-backend diagnostic.
- `extern rust fn ...` and `extern c fn ...` declarations are optional top-level interface contracts for foreign calls. They are checked by Poly and are not emitted.

### Target selection

~~~text
poly --target rust program.poly
poly --target c program.poly
~~~

### Explicit Foreign Signatures

Opaque foreign calls remain valid for compatibility, but an explicit signature can make Poly check the interface before invoking the native compiler:

~~~poly fragment
extern rust fn native_value(value: i32): i32

#rust
fn native_value(value: i32) -> i32 { value * 2 }
#endrust

var result i32 := native_value(21)
put result
~~~

Use `extern c fn` with `#c` blocks in the same way. The target must match the selected backend, declarations must be top-level, and the native compiler remains authoritative for the foreign definition and ABI.


`--check` invokes the selected native compiler. The C preview supports scalar
orchestration, plain structs, functions, conditions, loops, arithmetic, and
stdout/stderr output; complex definitions belong in `#c` blocks.

Rules:
- `#<lang>` and `#end<lang>` must appear on their own lines (leading whitespace is allowed).
- The content between them is copied character-for-character into the output.
- Poly comments (`# ...`) inside a foreign block are NOT treated as Poly comments; the content is opaque to Poly.
- End markers are recognized only at the start of a line and are opaque inside foreign code strings and comments.

---

## 3. Variables & Constants

### Variable Declaration

~~~poly fragment
var name Type := value     # mutable, with type
var name := value          # mutable, type inferred
~~~

### Constant Declaration

~~~poly fragment
const NAME := value        # immutable, module-scope
~~~

### Immutable Binding

~~~poly fragment
let name: Type := value   # immutable, block-scope; the type is optional
~~~

### Assignment

~~~poly fragment
name := value              # explicit assignment
name = other               # equality comparison, not assignment
~~~

### Mutation

~~~poly fragment
name := name + value       # explicit re-assignment
vector.pop()               # remove and yield the last element
~~~

Compound assignment operators (`+=`, `-=`) and the assembly-style `add` /
`sub` / `inc` / `dec` statements are retired syntax: the parser rejects them
and suggests the explicit `name := name + value` form.

---

## 4. Types

| Poly Type | Rust Equivalent | Description |
|-----------|----------------|-------------|
| `bool` | `bool` | Boolean |
| `i8` / `u8` | `i8` / `u8` | 8-bit integer |
| `i16` / `u16` | `i16` / `u16` | 16-bit integer |
| `i32` / `u32` | `i32` / `u32` | 32-bit integer |
| `i64` / `u64` | `i64` / `u64` | 64-bit integer |
| `i128` / `u128` | `i128` / `u128` | 128-bit integer |
| `f32` | `f32` | 32-bit float |
| `f64` | `f64` | 64-bit float |
| `isize` / `usize` | `isize` / `usize` | Platform integer |
| `char` | `u8` | Single ASCII byte |
| `string` | `String` / `Vec<u8>` | ASCII string |
| `ustring` | `String` | UTF-8 Unicode string |
| `byte` | `u8` | Raw byte |
| `bytes` | `Vec<u8>` | Byte buffer |
| `ptr T` | `*const T` | Raw pointer |

**Complex types** (generics, tuples, enums, traits) should be written in `#rust` blocks using native Rust syntax.

---

## 5. I/O: The `put` Command

`put` writes to stdout with a trailing newline.

~~~poly fragment
put "Hello, World!"
put variable
put "x = " + x + ", y = " + y
~~~

### File Output

~~~poly fragment
put expr to "file.txt"             # write (overwrite)
put expr to "file.txt" -append     # append
~~~

### Stderr

~~~poly
error "something went wrong"       # [ERROR] prefix
warn "deprecated"                  # [WARN] prefix
info "debug info"                  # [INFO] prefix
~~~

### String Interpolation

~~~poly
var name := "Alice"
put "Hello, {name}!"
~~~

---

## 6. I/O: The `get` Command

~~~poly
var x := get                        # read from stdin
var x ustring := get                # typed read
var x := get from "file.txt"        # read from file
var x := get from "file.txt" --bytes 42  # read N bytes
~~~

### Flags

~~~poly
get --timeout 3000                  # timeout in ms
get --default "fallback"            # default on empty
get --mask "*"                      # hide input (passwords)
get --until ","                     # read until delimiter
~~~

### Input Conversion

~~~poly
var age i32 := get --as i32
~~~

The parser retains `with validate`, `with complete`, and `with encoding` as compatibility forms, but they are not part of the maintained runnable v2 API.

---

## 7. Control Flow

### If/Else

~~~poly fragment
if condition,
    # do something
else if other_condition,
    # do other thing
else,
    # default
end if
~~~

### While

~~~poly fragment
while condition
    # loop body
    break           # exit loop
    continue        # skip to next iteration
end while
~~~

### Loop (Infinite)

~~~poly
loop
    # runs forever until break
end loop
~~~

### Loop Ranges

~~~poly
loop i 0..10
    put i
end loop

loop i 0..10 step 2
    put i
end loop

loop i 10..1 step -1
    put i
end loop

loop i 1..3, 7, 19..20
    put i
end loop
~~~

### Collection Iteration

~~~poly fragment
loop item in collection
    put item
end loop
~~~

---

## 8. Functions

~~~poly
fn add(a: i32, b: i32): i32
    return a + b
end fn

fn greet(name: ustring)
    put "Hello, " + name + "!"
end fn
~~~

**Complex functions** (generics, closures, async, trait bounds) go in `#rust` blocks:

~~~rust
#rust
fn process<T: Display + Clone>(items: Vec<T>) -> Vec<String> {
    items.iter().map(|x| format!("{}", x)).collect()
}

async fn fetch(url: &str) -> Result<String, reqwest::Error> {
    Ok(reqwest::get(url).await?.text().await?)
}
#endrust
~~~

---

## 9. Structs

Simple structs with plain fields:

~~~poly
struct Point
    x: f32
    y: f32
end struct
~~~

**Methods and complex structs** go in `#rust` blocks.

---

## 10. Removed Assembly-Style Operators

The assembly-style mutation statements and compound assignment operators from
earlier versions are retired. The parser rejects them with a migration hint
pointing at the explicit form:

| Retired syntax | Replacement |
|------|-------------|
| `add x` | `x := x + 1` |
| `sub x` | `x := x - 1` |
| `inc x` | `x := x + 1` |
| `dec x` | `x := x - 1` |
| `x += n` | `x := x + n` |
| `x -= n` | `x := x - n` |

### Bitwise

| Poly | Rust |
|------|------|
| `a bitand b` | `a & b` |
| `a bitor b` | `a \| b` |
| `a xor b` | `a ^ b` |
| `bitnot a` | `!a` |
| `a shift left n` (or `a << n`) | `a << n` |
| `a shift right n` (or `a >> n`) | `a >> n` |

---

## 11. Literals

| Poly | Description |
|------|-------------|
| `42` | Decimal integer |
| `0xFF` | Hexadecimal |
| `0b1010` | Binary |
| `0o77` | Octal |
| `3.14` | Float |
| `"hello"` | ASCII string |
| `unicode "hello"` | Unicode string |
| `true` / `false` | Boolean |
| `[0x48, 0x65]` | Byte array |

---

## 12. Operators

### Arithmetic
`+` `-` `*` `/` `mod`

### Comparison
`=` `!=` `<` `>` `<=` `>=`

### Logical
`and` `or` `not`

### Bitwise
`bitand` `bitor` `xor` `bitnot` `shift left` / `shift right` (alternatively `<<` `>>`)

### Assignment
`:=` `=` `+=` `-=` `->`

### Range
`..` `..=`

### Path
`::`

---

## 13. Comments

~~~poly
# This is a Poly comment (runs to end of line)
~~~

~~~rust
#rust
// This is a Rust comment inside a Rust block
/* block comment */
#endrust
~~~

---

## 14. Transpilation Model

~~~
source.poly
    │
    ├── #rust blocks ──→ Declarations emitted at module scope
    │
    ├── Poly sections ──→ Lexer → Parser → AST → Codegen → fn main() { ... }
    │
    └── output.rs
            │
            └──→ rustc (or cargo build)
~~~

### Example Transpilation

**Input:**
~~~poly fragment
#rust
fn rust_multiply(x: i32, y: i32) -> i32 {
    x * y
}
#endrust

var count i32 := 0
loop i 0..5
    count := count + 1
end loop
put "Count: " + count
put "Rust says: " + rust_multiply(count, 2)
~~~

**Output:**
~~~rust
// Generated from Poly source code
#![allow(unused_variables, unused_mut, unused_imports, dead_code)]

// --- Declarations from #rust blocks ---
fn rust_multiply(x: i32, y: i32) -> i32 {
    x * y
}

// --- Generated from Poly ---
fn main() {
    let mut count: i32 = 0;
    for i in 0..=5 {
        count += 1;
    }
    println!("{}", format!("Count: {}", count));
    println!("{}", format!("Rust says: {}", rust_multiply(count, 2)));
}
~~~

---

## 15. What Belongs Where

| Feature | Poly | `#rust` block (opaque target-language definitions) |
|---------|------|---------------|
| Variables | ✅ | |
| Constants | ✅ | |
| `put` / `get` I/O | ✅ | |
| File I/O (`to` / `from`) | ✅ | |
| `error` / `warn` / `info` | ✅ | |
| Explicit mutation (`x := x + 1`) | ✅ | |
| `if` / `while` / `loop` | ✅ | |
| Simple `fn` (no generics) | ✅ | |
| Simple `struct` (no methods) | ✅ | |
| Primitives + strings | ✅ | |
| | | `#rust`/`#c` fn (with generics, async, closures) |
| | | `#rust`/`#c` struct (with methods, default values) |
| | | enum (with data, methods) |
| | | trait + impl |
| | | use / mod |
| | | const / type alias |

**Rule of thumb:** Poly orchestrates. Foreign blocks define. No executable code in foreign blocks.

---

## 16. Example: Full Program

~~~poly fragment
# Foreign block: provide the data structures and functions
#rust
use std::collections::HashMap;

struct Record {
    key: String,
    value: String,
}

fn parse_command(input: &str) -> (String, String, String) {
    let parts: Vec<&str> = input.splitn(3, ' ').collect();
    (
        parts[0].to_string(),
        parts.get(1).unwrap_or(&"").to_string(),
        parts.get(2).unwrap_or(&"").to_string(),
    )
}
#endrust

# Poly: orchestrate the CLI
var running bool := true
put "Commands: set <key> <value>, get <key>, quit"

while running
    put "> "
    var input ustring := get
    var cmd, key, value := parse_command(input)

    if cmd = "quit"
        running := false
    else if cmd = "set"
        put "Set " + key + " = " + value
    else if cmd = "get"
        put "Getting: " + key
    else
        warn "Unknown command: " + cmd
    end if
end while

put "Goodbye!"
~~~
