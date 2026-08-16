# Poly Language Tools: How-To Guide

## Quick Start

### 1. Build the Compiler

~~~bash
cd compiler
cargo build --release
~~~

The binary will be at `target/release/poly`.

### 2. Compile a Poly File

~~~bash
poly hello.poly
cargo run --manifest-path rust_output/hello/Cargo.toml
~~~

This generates and builds `rust_output/hello/` as an isolated Cargo project. The generated Rust is in `rust_output/hello/src/main.rs`; the built executable is in `rust_output/hello/target/debug/hello`.

To print the generated Rust instead, use:

~~~bash
poly --emit-rust hello.poly
~~~

### 3. Run the Generated Rust Code

~~~bash
cargo run --manifest-path rust_output/hello/Cargo.toml
~~~

---

## CLI Commands

| Command | Description |
|---------|-------------|
| `poly file.poly` | Generate and build `rust_output/<program>/` with Cargo |
| `poly --emit-rust file.poly` | Print transpiled Rust |
| `poly --intermediate-representation file.poly` | Print the intermediate representation pipeline output (optimized Rust); `--ir` is an alias |
| `poly --source-map file.poly` | Print the generated Poly→Rust source map |
| `poly --check file.poly` | Validate code and verify generated Rust compilation |
| `poly --tokens file.poly` | Show lexer tokens |
| `poly --ast file.poly` | Show AST |
| `poly --repl` | Start interactive REPL |
| `poly --help` | Show help |
| `poly --version` | Show version |

---

## Writing Poly Code

### Hello World

~~~poly
put "Hello, World!"
~~~

### Variables

~~~poly
var x i32 := 42
var name ustring := "Alice"
var pi := 3.14
~~~

### Functions

~~~poly fragment
fn add(a: i32, b: i32): i32
    return a + b
end fn

var result := add(3, 4)
put result
~~~

### Control Flow

**If/Else:**
~~~poly
if x > 0
    put "positive"
else
    put "non-positive"
end if
~~~

**While Loop:**
~~~poly
var i := 0
while i < 10
    put i
    i += 1
end while
~~~

**For Loop (Range):**
~~~poly
for i in 0..10
    put i
end for
~~~

### Increment

~~~poly
var x := 0
x += 1       # increment x by one
~~~

### Pattern Matching

~~~poly
match direction
    North, put "up"
    South, put "down"
    _, put "other"
end match
~~~

---

## Common Patterns

### Read User Input

~~~poly
var name := get unicode "What is your name? "
put "Hello, " + name + "!"
~~~

### Read a File

~~~poly
var content := get < "data.txt"
put content
~~~

### Write to a File

~~~poly
put "Hello" > "output.txt"
put "World" >> "output.txt"
~~~

### Error Handling

~~~poly
error "Something went wrong!"
warn "This might be a problem"
info "Debug: x = " + x
~~~

---

## REPL Mode

Start the interactive REPL:

~~~bash
poly --repl
~~~

The REPL keeps a **stateful session**: variables and functions you declare persist across lines, and the whole session is re-transpiled after every submission. On Linux/macOS terminals a raw-mode line editor provides Tab completion, arrow-key history navigation, and cursor movement; elsewhere it falls back to plain line reading.

**REPL Commands:**
- `:help` - Show help
- `:tokens` - Show tokens for the session or pending buffer
- `:ast` - Show AST for the session or pending buffer
- `:vars` - List session statements and bindings
- `:history` - Show command history
- `:clear` - Clear the pending buffer
- `:reset` - Reset the session (drop all variables)
- `:clear-history` - Clear saved history
- `:quit` - Exit

**Example Session:**
~~~
poly> var x := 42
poly> put x

// Generated Rust code:
// let mut x = 42;
// println!("{}", x);

poly> var y := x * 2   # x is still in scope
poly> put y

poly> :vars
Session:
  1  var x := 42
  2  put x
  3  var y := x * 2
  4  put y
Bindings: x, y

poly> :quit
Goodbye!
~~~

---

## Checking Code

Validate without transpiling:

~~~bash
poly --check file.poly
~~~

Output on success:
~~~
OK: 5 statements parsed
~~~

---

## Examples

Run the example files:

~~~bash
poly examples/prime_numbers.poly
cargo run --manifest-path rust_output/prime_numbers/Cargo.toml
~~~

Available examples:
- `prime_numbers.poly` - Calculate prime numbers
- `interactive_menu.poly` - Console menu system
- `error_handling.poly` - Error handling patterns
- `file_processing.poly` - File I/O operations
- `loop_ranges.poly` - Range-based loops
- `input_validation.poly` - Input validation

---

## Type System

| Poly Type | Rust Equivalent |
|-----------|-----------------|
| `i32` | `i32` |
| `f32` | `f32` |
| `bool` | `bool` |
| `ustring` | `String` |
| `bytes` | `Vec<u8>` |

---

## Tips

1. **Use `--check` first** to validate code before compiling
2. **Use `--emit-rust`** when you need to inspect generated Rust
3. **Try the REPL** to experiment with syntax
4. **Check generated Rust** if you get compilation errors
5. **Use examples** as reference for common patterns

---

## Getting Help

- Run `poly --help` for CLI options
- See `POLY_QUICK_REFERENCE.md` for syntax reference
- See `POLY_TUTORIAL.md` for detailed tutorials
- See `POLY_CHEATSHEET.md` for a quick syntax cheat sheet
