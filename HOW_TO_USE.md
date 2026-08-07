# Poly Language Tools: How-To Guide

## Quick Start

### 1. Build the Compiler

```bash
cd compiler
cargo build --release
```

The binary will be at `target/release/poly`.

### 2. Transpile a Poly File to Rust

```bash
poly hello.poly
```

This outputs Rust code to stdout.

### 3. Run the Generated Rust Code

```bash
poly hello.poly > hello.rs
rustc hello.rs -o hello
./hello
```

---

## CLI Commands

| Command | Description |
|---------|-------------|
| `poly file.poly` | Transpile to Rust |
| `poly --check file.poly` | Validate code (like `rustc --check`) |
| `poly --tokens file.poly` | Show lexer tokens |
| `poly --ast file.poly` | Show AST |
| `poly --repl` | Start interactive REPL |
| `poly --help` | Show help |
| `poly --version` | Show version |

---

## Writing Poly Code

### Hello World

```poly
put "Hello, World!"
```

### Variables

```poly
var x: i32 = 42
var name: ustring = "Alice"
var pi = 3.14
```

### Functions

```poly
fn add(a: i32, b: i32): i32
    return a + b
end fn

var result = add(3, 4)
put result
```

### Control Flow

**If/Else:**
```poly
if x > 0 then
    put "positive"
else
    put "non-positive"
end if
```

**While Loop:**
```poly
var i = 0
while i < 10
    put i
    add i
end while
```

**For Loop (Range):**
```poly
for i in 0..10
    put i
end for
```

### Increment

```poly
var x = 0
add x       # x = x + 1
```

### Pattern Matching

```poly
match direction
    North => put "up"
    South => put "down"
    _ => put "other"
end match
```

---

## Common Patterns

### Read User Input

```poly
var name = get "What is your name? "
put "Hello, " + name + "!"
```

### Read a File

```poly
var content = get < "data.txt"
put content
```

### Write to a File

```poly
put "Hello" > "output.txt"
put "World" >> "output.txt"
```

### Error Handling

```poly
error "Something went wrong!"
warn "This might be a problem"
info "Debug: x = " + x
```

---

## REPL Mode

Start the interactive REPL:

```bash
poly --repl
```

**REPL Commands:**
- `:help` - Show help
- `:tokens` - Show tokens for buffered input
- `:ast` - Show AST for buffered input
- `:clear` - Clear the buffer
- `:quit` - Exit

**Example Session:**
```
poly> var x = 42
poly> put x

// Generated Rust code:
// let mut x = 42;
// println!("{}", x);

poly> :quit
Goodbye!
```

---

## Checking Code

Validate without transpiling:

```bash
poly --check file.poly
```

Output on success:
```
OK: 5 statements parsed
```

---

## Examples

Run the example files:

```bash
poly examples/prime_numbers.poly > primes.rs
rustc primes.rs -o primes
./primes
```

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

1. **Use `--check` first** to validate code before transpiling
2. **Try the REPL** to experiment with syntax
3. **Check generated Rust** if you get compilation errors
4. **Use examples** as reference for common patterns

---

## Getting Help

- Run `poly --help` for CLI options
- See `POLY_QUICK_REFERENCE.md` for syntax reference
- See `POLY_TUTORIAL.md` for detailed tutorials
- See `POLY_CHEATSHEET.md` for a quick syntax cheat sheet
