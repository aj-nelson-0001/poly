# Poly

**A minimalist, assembly-inspired systems programming language that transpiles to safe, idiomatic Rust.**

![Version](https://img.shields.io/badge/version-1.6.0-green)
![Status](https://img.shields.io/badge/status-Active%20Development-blue)
![Backend](https://img.shields.io/badge/backend-Rust-black)

---

## Overview

Poly is designed as a minimalist, low-overhead system programming language with a clean, highly explicit syntax reminiscent of assembly language. It provides fine-grained control over low-level memory and ASCII/Unicode data primitives while transpiling directly into safe, idiomatic Rust.

### Core Principles

1. **Explicit is Better than Implicit** — All operations are clear and predictable
2. **Assembly-Inspired Syntax** — Familiar to systems programmers
3. **Safe Transpilation** — Leverages Rust's memory safety guarantees
4. **Zero-Cost Abstractions** — High-level features compile to efficient Rust
5. **Gradual Complexity** — Simple features for beginners, powerful features for experts

---

## What's New in v1.6.0

🎉 **New If Statement Syntax!** We've simplified if statements to use commas instead of `then`:

**Before:** `if x > 0 then ... end if`
**After:** `if x > 0, ... end if`

This change makes Poly code more concise and familiar to developers from other languages.

[Read the full blog post](BLOG_POST_if_syntax.md)

---

## Quick Start

### Hello, World!

```poly
put "Hello, World!"
```

### Variables & Output

```poly
var name: ustring = u"Poly"
var version: i32 = 1
put "Language: " + name + ", Version: " + version
```

### Input with Validation

```poly
put -n "Enter your age: "
var age: i32 = get with validate |x| x > 0 && x < 150
put "You are " + age + " years old."
```

### Error Handling

```poly
enum FileError
    NotFound
    PermissionDenied
end enum

fn read_config(path: ustring): Result<ustring, FileError>
    var content = try open_file(path)
    return Ok(content)
end fn

match read_config(u"config.txt")
    Ok(content) => put "Config loaded: " + content
    Error(NotFound) => error "Config file not found"
    Error(PermissionDenied) => error "Permission denied"
end match
```

### Loop Ranges (SuperBASIC-inspired)

```poly
# Simple range
loop: 0..10
    put i
end loop

# Multiple ranges and specific values
loop: 1..3, 7, 19..20
    put i  # Iterates: 1, 2, 3, 7, 19, 20
end loop

# With step
loop: 0..10 step 2
    put i  # Iterates: 0, 2, 4, 6, 8
end loop
```

---

## Features

### I/O System

| Feature | Syntax | Description |
|---------|--------|-------------|
| Basic output | `put expr` | Print with newline |
| No newline | `put -n expr` | Print without newline |
| File write | `put expr > "file"` | Write/overwrite file |
| File append | `put expr >> "file"` | Append to file |
| Error output | `error expr` | Print to stderr with `[ERROR]` |
| Warning output | `warn expr` | Print to stderr with `[WARN]` |
| Debug output | `info expr` | Print to stderr with `[INFO]` |
| Basic input | `var x: Type = get` | Read from stdin |
| Input with prompt | `var x: Type = get "prompt"` | Prompt,read |
| Default value | `get --default u"value"` | Fallback on empty input |
| Masked input | `get --mask u"*"` | Hide password input |
| Timeout | `get --timeout 3000` | Timeout in milliseconds |
| Validation | `get with validate \|x\| x > 0` | Validate with closure |
| Completion | `get with complete [u"a", u"b"]` | Autocomplete options |
| File input | `var x = get < "file"` | Read from file |
| Binary input | `var x: bytes = get < "file"` | Read as bytes |

### Types

| Poly Type | Size | Rust Mapping |
|-----------|------|--------------|
| `bool` | 1 byte | `bool` |
| `i8` / `u8` | 1 byte | `i8` / `u8` |
| `i16` / `u16` | 2 bytes | `i16` / `u16` |
| `i32` / `u32` | 4 bytes | `i32` / `u32` |
| `i64` / `u64` | 8 bytes | `i64` / `u64` |
| `i128` / `u128` | 16 bytes | `i128` / `u128` |
| `f32` | 4 bytes | `f32` |
| `f64` | 8 bytes | `f64` |
| `char` | 1 byte | `u8` |
| `string` | 1 byte/char | `Vec<u8>` / `&[u8]` |
| `uchar` | 4 bytes | `char` |
| `ustring` | 1-4 bytes/char | `String` / `&str` |
| `byte` | 1 byte | `u8` |
| `bytes` | Variable | `Vec<u8>` |
| `ptr T` | Pointer size | `*const T` |

### Language Constructs

- **Variables**: `var name: Type = value` (mutable), `let name: Type = value` (immutable)
- **Constants**: `const NAME = value`
- **Functions**: `fn name(params): ReturnType ... end fn`
- **Structs**: `struct Name ... end struct`
- **Enums**: `enum Name ... end enum`
- **Traits**: `trait Name ... end trait`
- **Impls**: `impl Trait for Type ... end impl`
- **Modules**: `module name ... end module`
- **Closures**: `|params| expr` or `|params| ... end`
- **If/Else**: `if cond,... else ... end if`
- **While**: `while cond ... end while`
- **Loop**: `loop ... end loop` (infinite), `loop: range ... end loop` (range)
- **Match**: `match expr ... pattern => expr ... end match`
- **Error handling**: `Result<T, E>` with `Ok(val)` / `Error(err)`, `try` for propagation
- **Unsafe**: `unsafe ... end unsafe`
- **Assembly ops**: `add var`, `sub var`, `inc var`, `dec var`

---

## Project Structure

```
Poly/
├── README.md                          # This file
├── POLY_LANGUAGE_SPECIFICATION_EXPANDED.md  # Full language specification
├── POLY_CHEATSHEET.md                 # Quick syntax reference
├── POLY_QUICK_REFERENCE.md            # Concise reference card
├── POLY_TUTORIAL.md                   # Beginner tutorial
├── POLY_API_REFERENCE.md              # API documentation
├── POLY_BEST_PRACTICES.md             # Idiomatic patterns
├── POLY_TESTING_GUIDE.md              # Testing strategies
├── POLY_PERFORMANCE_GUIDE.md          # Optimization tips
├── POLY_SECURITY_GUIDE.md             # Security considerations
├── POLY_COMPATIBILITY_GUIDE.md        # Cross-platform notes
├── POLY_DEPLOYMENT_GUIDE.md           # Build & deploy
├── POLY_VERSIONING_GUIDE.md           # Version management
├── POLY_PROFILING_GUIDE.md            # Profiling techniques
├── POLY_INTEGRATION_GUIDE.md          # External integrations
├── POLY_TROUBLESHOOTING_GUIDE.md      # Common issues
├── POLY_MIGRATION_GUIDE.md            # Migration paths
├── POLY_FUTURE_MIGRATION_GUIDE.md     # Future migration plans
├── POLY_GRAMMAR.md                    # Formal BNF grammar
├── examples/
│   ├── prime_numbers.poly             # Loop ranges, basic functions
│   ├── interactive_menu.poly          # Menus, match, get flags
│   ├── file_processing.poly           # File I/O, CSV, binary
│   ├── error_handling.poly            # Custom errors, try propagation
│   └── loop_ranges.poly               # SuperBASIC-style iteration
├── tests/
│   ├── output_tests.poly              # put command tests
│   ├── input_tests.poly               # get command tests
│   └── comprehensive_io_tests.poly    # Combined I/O tests
└── compiler/                          # Rust transpiler (in progress)
    ├── Cargo.toml                     # Workspace root
    ├── crates/
    │   ├── poly-lexer/                # Tokenizer
    │   ├── poly-parser/               # AST builder
    │   ├── poly-types/                # Type system
    │   ├── poly-transpiler/           # Rust code generator
    │   └── poly-cli/                  # Command-line interface
    └── grammar/
        └── poly.bnf                   # Formal grammar
```

---

## Documentation

| Guide | Description |
|-------|-------------|
| [Language Specification](POLY_LANGUAGE_SPECIFICATION_EXPANDED.md) | Complete language reference |
| [Tutorial](POLY_TUTORIAL.md) | Getting started guide |
| [Quick Reference](POLY_QUICK_REFERENCE.md) | Syntax at a glance |
| [Cheat Sheet](POLY_CHEATSHEET.md) | Compact syntax reference |
| [API Reference](POLY_API_REFERENCE.md) | Standard library APIs |
| [Best Practices](POLY_BEST_PRACTICES.md) | Idiomatic Poly patterns |
| [Testing Guide](POLY_TESTING_GUIDE.md) | How to test Poly programs |
| [Performance Guide](POLY_PERFORMANCE_GUIDE.md) | Optimization techniques |
| [Security Guide](POLY_SECURITY_GUIDE.md) | Security considerations |
| [Grammar](POLY_GRAMMAR.md) | Formal BNF grammar |

---

## Transpilation

Poly transpiles to Rust. Every Poly construct has a direct Rust equivalent:

| Poly | Rust |
|------|------|
| `var x: i32 = 0` | `let mut x: i32 = 0;` |
| `const MAX = 100` | `const MAX: i32 = 100;` |
| `put "hello"` | `println!("{}", "hello");` |
| `get` | Standard input reading |
| `if x > 0,` | `if x > 0 {` |
| `loop: 0..10` | `for i in 0..10 {` |
| `fn add(a: i32, b: i32): i32` | `fn add(a: i32, b: i32) -> i32` |
| `struct Point` | `struct Point` |
| `enum Shape` | `enum Shape` |
| `match x { ... }` | `match x { ... }` |
| `try expr` | `expr?` |

---

## Examples

### Prime Numbers

```poly
fn is_prime(n: i32): bool
    if n <= 1,
        return false
    end if
    var i: i32 = 2
    while i * i <= n
        if n % i == 0,
            return false
        end if
        add i
    end while
    return true
end fn

put u"First 20 prime numbers:"
var count: i32 = 0
loop: 2..200
    if is_prime(num),
        put num
        add count
        if count >= 20,
            break
        end if
    end if
end loop
```

### Interactive Menu

```poly
fn main()
    var running: bool = true
    while running
        put "Menu:"
        put "1. Greet user"
        put "2. Exit"
        put -n "Choose: "
        var choice: i32 = get
        match choice
            1 => greet_user()
            2 => running = false
            _ => warn "Invalid choice"
        end match
    end while
end fn
```

---

## Building the Transpiler

The transpiler is implemented in Rust as a Cargo workspace:

```bash
cd compiler
cargo build
cargo test
./target/debug/poly-cli ../examples/prime_numbers.poly
```

### CLI Options

| Option | Description |
|--------|-------------|
| `poly <file.poly>` | Transpile a Poly file to Rust |
| `poly --tokens <file>` | Print tokens and exit |
| `poly --ast <file>` | Print AST and exit |
| `poly --check <file>` | Validate code and verify Rust compilation |
| `poly --format <file>` | Format output with rustfmt |
| `poly --diff <file>` | Show diff between unformatted and formatted |
| `poly --watch <file>` | Watch file and re-transpile on changes |
| `poly --repl` | Start interactive REPL |
| `poly --help` | Show help message |
| `poly --version` | Show version information |

---

## Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

---

## License

This project is open source. See the repository for license details.

---

## Acknowledgments

- Inspired by assembly language syntax
- Loop ranges inspired by Sinclair QL SuperBASIC
- Error handling patterns inspired by Rust
- I/O syntax inspired by shell scripting
