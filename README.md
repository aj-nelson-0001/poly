# Poly

**A minimalist, assembly-inspired systems programming language that transpiles to safe, idiomatic Rust.**

![Version](https://img.shields.io/badge/version-1.7.3-green)
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

## What's New in v1.7.3

🎉 **Higher-order functions!** Functions can return closures (`fn make_adder(n: i32): |x: i32| i32`), vectors and strings support `map`/`filter`/`reduce`/`sort_by`, and closure literals can be stored in variables and passed around.

Match arms use comma syntax: `Ok(data), put data` (previously `=>`). Loop ranges are inclusive and require an explicit binding: `loop: i 0..10`. `get` supports real `--timeout`/`--default`/`--as` behavior, file reading (`get_line`/`eof`) works, and vectors print with `{:?}` formatting.

See [CHANGELOG.md](CHANGELOG.md) for the full history, including v1.7.0 (LSP + in-browser wasm transpiler), v1.7.1 (closure type annotations), and v1.6.0 (explicit `:=`/`=`/`==` syntax).

---

## Quick Start

### Hello, World!

~~~poly
put "Hello, World!"
~~~

### Variables & Output

~~~poly
var name ustring := unicode "Poly"
var version i32 := 1
put "Language: " + name + ", Version: " + version
~~~

### Input with Validation

~~~poly
put -n "Enter your age: "
var age i32 := get with validate |x| x > 0 && x < 150
put "You are " + age + " years old."
~~~

### Error Handling

~~~poly
enum FileError
    NotFound
    PermissionDenied
end enum

fn read_config(path: ustring): Result<ustring, FileError>
    var content := try open_file(path)
    return Ok(content)
end fn

match read_config(unicode "config.txt")
    Ok(content), put "Config loaded: " + content
    Error(NotFound), error "Config file not found"
    Error(PermissionDenied), error "Permission denied"
end match
~~~

### Loop Ranges (SuperBASIC-inspired)

The loop variable must be named explicitly after `loop:`: `loop: <var_name> <ranges>`.
Collection iteration uses `loop: <var_name> in <collection>`. Loop ranges include both endpoints, so `1..3` iterates `1, 2, 3`; `..=` is accepted but redundant for `loop:` ranges.

~~~poly
# Simple range
loop: i 0..10
    put i
end loop

# Multiple ranges and specific values
loop: value 1..3, 7, 19..20
    put value  # Iterates: 1, 2, 3, 7, 19, 20
end loop

# With step
loop: i 0..10 step 2
    put i  # Iterates: 0, 2, 4, 6, 8, 10
end loop
~~~

---

## Features

### I/O System

| Feature | Syntax | Description |
|---------|--------|-------------|
| Basic output | `put expr` | Print with newline (vectors print with `{:?}` debug formatting) |
| No newline | `put -n expr` | Print without newline |
| File write | `put expr > "file"` | Write/overwrite file |
| File append | `put expr >> "file"` | Append to file |
| Error output | `error expr` | Print to stderr with `[ERROR]` |
| Warning output | `warn expr` | Print to stderr with `[WARN]` |
| Debug output | `info expr` | Print to stderr with `[INFO]` |
| Basic input | `var x := get` | Read from stdin |
| Input with prompt | `var x ustring := get unicode "Prompt:"` | Display a Unicode prompt, then read |
| Default value | `get --default unicode "value"` | Fallback on empty input |
| Masked input | `get --mask unicode "*"` | Hide password input |
| Timeout | `get --timeout 3000` | Timeout in milliseconds |
| Validation | `get with validate \|x\| x > 0` | Validate with closure |
| Completion | `get with complete [unicode "a", unicode "b"]` | Autocomplete options |
| File input | `var x := get < "file"` | Read from file |
| Binary input | `var x bytes := get < "file"` | Read as bytes |

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

- **Variables**: `var name Type := value` (mutable; type optional), `let name: Type = value` (immutable)
- **Constants**: `const NAME = value`
- **Operators**: `==` compares values; assignment uses `name = value`; mutation uses `+=` and `-=`
- **Functions**: `fn name(params): ReturnType ... end fn`, with generic parameters: `fn identity<T>(value: T): T`
- **Structs**: `struct Name ... end struct`, with generic parameters: `struct Wrapper<T>`
- **Enums**: `enum Name ... end enum`
- **Traits**: `trait Name ... end trait`
- **Impls**: `impl Trait for Type ... end impl`
- **Modules**: `module name ... end module`
- **Macros**: `macro name(params) ... end macro` (compile-time text expansion with substitution)
- **Closures**: `|params| expr` or `|params| ... end`
- **Function types**: `fn apply(f: |x: i32| i32, v: i32): i32` — closures as
  typed parameters; `fn make_adder(n: i32): |x: i32| i32` returns a closure
- **Higher-order methods**: `xs.map(|x| x * 2)`, `xs.filter(|x| x % 2 == 0)`,
  `xs.reduce(0, |acc, x| acc + x)`, `xs.sort_by(|a, b| a > b)` — callbacks
  receive owned elements; strings iterate their characters (`"hi".map(|c| c)`
  yields `Vec<char>`); `sort_by` returns a sorted copy
- **If/Else**: `if cond ... else ... end if`
- **While**: `while cond ... end while`
- **Loop**: `loop ... end loop` (infinite), `loop: variable range ... end loop` (range), `loop: variable in collection ... end loop`
- **Match**: `match expr ... pattern, expr ... end match`
- **Error handling**: `Result<T, E>` with `Ok(val)` / `Error(err)`, `try` for propagation
- **Math functions**: `abs`, `sqrt`, `pow`, `min`, `max`
- **Generic containers**: `Vec<T>`, `Map<K, V>`, `Set<T>`, `Box<T>`, `Rc<T>`, `Arc<T>`
- **Unsafe**: `unsafe ... end unsafe`
- **Mutation**: `count += 1`, `total -= amount`

---

## Project Structure

~~~
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
├── POLY_DOCUMENTATION_STYLE_GUIDE.md  # Documentation and example conventions
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
├── vscode/                            # VS Code extension (grammar + LSP client)
└── compiler/                          # Rust transpiler (in progress)
    ├── Cargo.toml                     # Workspace root
    ├── crates/
    │   ├── poly-lexer/                # Tokenizer
    │   ├── poly-parser/               # AST builder with source spans
    │   ├── poly-types/                # Type system
    │   ├── poly-intermediate-representation/  # Intermediate representation + optimizer
    │   ├── poly-transpiler/           # Rust code generator (routes through the shared IR pipeline)
    │   ├── poly-wasm/                 # WebAssembly bindings for the playground
    │   ├── poly-lsp/                  # Dependency-free JSON-RPC language server
    │   └── poly-cli/                  # Command-line interface
    └── grammar/
        └── poly.bnf                   # Formal grammar
~~~

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
| [Documentation Style Guide](POLY_DOCUMENTATION_STYLE_GUIDE.md) | Documentation and example conventions |
| [Grammar](POLY_GRAMMAR.md) | Formal BNF grammar |

---

## Transpilation

Poly transpiles to Rust. Every Poly construct has a direct Rust equivalent:

| Poly | Rust |
|------|------|
| `var x i32 := 0` | `let mut x: i32 = 0;` |
| `const MAX = 100` | `const MAX: i32 = 100;` |
| `x = value` | `x = value;` |
| `x += 1` | `x += 1;` |
| `put "hello"` | `println!("{}", "hello");` |
| `get` | Standard input reading |
| `if x > 0` | `if x > 0 {` |
| `loop: i 0..10` | `for i in 0..=10 {` |
| `fn add(a: i32, b: i32): i32` | `fn add(a: i32, b: i32) -> i32` |
| `struct Point` | `struct Point` |
| `enum Shape` | `enum Shape` |
| `match x { ... }` | `match x { ... }` |
| `try expr` | `expr?` |

---

## Examples

### Prime Numbers

~~~poly
fn is_prime(n: i32): bool
    if n <= 1,
        return false
    end if
    var i i32 := 2
    while i * i <= n
        if n % i == 0
            return false
        end if
        i += 1
    end while
    return true
end fn

put unicode "First 20 prime numbers:"
var count i32 := 0
loop: num 2..200
    if is_prime(num)
        put num
        count += 1
        if count >= 20
            break
        end if
    end if
end loop
~~~

### Interactive Menu

~~~poly
fn main()
    var running bool := true
    while running
        put "Menu:"
        put "1. Greet user"
        put "2. Exit"
        put -n "Choose: "
        var choice i32 := get
        match choice
            1, greet_user()
            2, running = false
            _, warn "Invalid choice"
        end match
    end while
end fn
~~~

### Higher-Order Functions

Functions are values: a function can accept a closure through a function-typed
parameter, return a closure that captures locals, and vectors expose `map`,
`filter`, and `reduce`:

~~~poly
fn apply(f: |x: i32| i32, v: i32): i32
    return f(v)
end fn

fn make_adder(n: i32): |x: i32| i32
    return |x| x + n
end fn

fn main()
    put apply(|x| x * 2, 21)          # 42
    var add5 := make_adder(5)
    put add5(10)                      # 15
    var xs := [1, 2, 3, 4, 5]
    put xs.map(|x| x * 2)             # [2, 4, 6, 8, 10] (vectors print with {:?})
    put xs.filter(|x| x % 2 == 0)     # [2, 4]
    put xs.reduce(0, |acc, x| acc + x)  # 15
    put xs.sort_by(|a, b| a > b)      # [5, 4, 3, 2, 1]
    put "hello".map(|c| c)            # ['h', 'e', 'l', 'l', 'o']
end fn
~~~

---

## Building the Transpiler

The transpiler is implemented in Rust as a Cargo workspace:

~~~bash
cd compiler
cargo build
cargo test
./target/debug/poly ../examples/prime_numbers.poly
~~~

The default command creates and builds an isolated Cargo project at `rust_output/<program>/`, containing `Cargo.toml`, `src/main.rs`, and its own `target/` directory. It does not run the program. Use `cargo run --manifest-path rust_output/prime_numbers/Cargo.toml` to run it.

To choose another Cargo project directory, use `--project`:

~~~bash
./target/debug/poly --project ./prime_numbers ../examples/prime_numbers.poly
cd prime_numbers
cargo run
~~~

Use `poly --emit-rust file.poly` when you want the generated Rust on stdout. Async Poly programs automatically receive the Tokio dependency in their generated Cargo project.

### CLI Options

| Option | Description |
|--------|-------------|
| `poly <file.poly>` | Generate and build `rust_output/<program>/` with Cargo |
| `poly --tokens <file>` | Print tokens and exit |
| `poly --ast <file>` | Print AST and exit |
| `poly --check <file>` | Validate code and verify generated Rust compilation |
| `poly --emit-rust <file>` | Print generated Rust to stdout |
| `poly --intermediate-representation <file>` | Print the intermediate representation pipeline output (optimized Rust); `--ir` is an alias |
| `poly --source-map <file>` | Print the generated Poly→Rust source map |
| `poly --format <file>` | Format output with rustfmt |
| `poly --diff <file>` | Show diff between unformatted and formatted |
| `poly --watch <file>` | Watch file and re-transpile on changes |
| `poly --project <dir> <file>` | Generate `<dir>/Cargo.toml` and `<dir>/src/main.rs` |
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
