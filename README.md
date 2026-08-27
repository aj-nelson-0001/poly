# Poly

**A minimal, assembly-inspired meta-language for any systems language.**

![Version](https://img.shields.io/badge/version-2.0--preview-green)
![Status](https://img.shields.io/badge/status-Active%20Development-blue)
![Backend](https://img.shields.io/badge/backends-Rust%20%7C%20C-black)

---

## Overview

Poly is a **thin syntax layer over any systems language.** It handles the simple, boilerplate-heavy parts of programming with an assembly-inspired syntax. For target-specific or advanced work, users provide definitions in foreign language blocks (`#rust` or `#c`). The parser models additional Rust-oriented constructs, but the supported backend and target determine what is runnable.

**Key principle:** Foreign blocks provide target-language definitions. Poly provides the orchestration. Foreign blocks must be top-level; the native compiler validates their contents.

**One Poly source file → one complete source file in the selected target language → compiled by the target compiler.**

### Core Principles

1. **Poly does the simple stuff. Target languages do the complex stuff.** — No feature creep
2. **Assembly-Inspired Syntax** — Familiar to systems programmers
3. **Zero Runtime** — Poly adds no overhead; it's just syntax sugar
4. **Explicit is Better than Implicit** — All operations are clear and predictable
5. **Passthrough Architecture** — Selected foreign blocks are emitted verbatim

---

## Foreign Language Blocks

Poly's key innovation is the `#rust` block. Provide definitions in Rust, and orchestrate them in Poly:

~~~poly fragment
#rust
use std::collections::HashMap;

fn process_data(input: &str) -> HashMap<String, Vec<i32>> {
    // Declaration only — Poly calls this
    todo!()
}
#endrust

# Poly: orchestrate the calls
var name ustring := unicode "Alice"
put "Hello, " + name + "!"
# The target compiler validates the foreign function signature.
var result := process_data(name)
put result
~~~

Foreign blocks provide target-language definitions at file scope. Poly does not parse their bodies; the selected native compiler validates them. Poly owns the generated entry point (`fn main()` for Rust or `int main(void)` for C).

This model currently supports `#rust` for Rust and `#c` for C. Optional `extern rust fn ...` and `extern c fn ...` declarations add Poly-side interface checks for foreign calls. `#cpp` syntax is reserved and rejected until a C++ backend is designed.

See [POLY_SPEC_v2.md](POLY_SPEC_v2.md) for the full specification, [POLY_V2_SUPPORT_MATRIX.md](POLY_V2_SUPPORT_MATRIX.md) for the frozen target contract, and [POLY_ROADMAP_v2.md](POLY_ROADMAP_v2.md) for the implementation plan.

---

## What's New in v2.0 Preview

Poly 2.0 adds target selection with `--target rust` and `--target c`. Rust and C
foreign blocks are selected by the target, emitted at file scope, and kept
opaque to Poly's type checker; the native target compiler validates them.

~~~poly fragment
#c
int double_value(int x) { return x * 2; }
#endc

var answer i32 := double_value(21)
put answer
~~~

Use `poly --target c --check program.poly` to validate generated C, or
`poly --target c program.poly` to emit and compile the C executable.

## Historical Release Notes

The v1.x notes below describe prior releases and are retained for migration context. They are not the v2 preview contract.

## What's New in v1.8.0

🔁 **Tuple power-ups.** Assign to tuple elements (`pair.0 := 99`,
`pair.1 = false`), destructure in `for` loops (`for (idx, val) in
items.enumerate()`), and read them by position (`pair.0`, `nested.1.0`).

## What's New in v1.7.6

🧩 **Tuple index access.** Read tuple elements by position: `pair.0`, and
chained `nested.1.0` — the lexer handles `.N` after a dot exactly like Rust.

## What's New in v1.7.5

🛠️ **The remaining input stubs are real.** `get --until unicode ","` reads up to the delimiter (returns everything before it) and `get --mask unicode "*"` suppresses terminal echo while typing (best-effort on Unix, plain-read fallback elsewhere). A mini test framework (`assert(cond)`, `pass(msg)`, `fail(msg)`), checked indexing (`xs.get(i)` → `Option<T>`, `"text".get(i)` → `Option<char>`), `Result` accessors (`is_ok()`/`is_error()`/`unwrap()`/`unwrap_or(v)`), and string interpolation (`put "Name: {name}, Age: {age}"`) all work now. CI gained a `cargo audit` dependency job.

🌐 **Historical v1.7.5 note.** Network builtins and async task support described here belong to the v1 implementation history. In v2, consult the target-specific compiler behavior and use `#rust` or `#c` helpers when a backend does not support the operation.

See [POLY_DOCUMENTATION_INDEX.md](POLY_DOCUMENTATION_INDEX.md) for the maintained v2 documents. Older release notes remain historical and are not normative for v2.

---

## Quick Start

### Hello, World!

~~~poly
put "Hello, World!"
~~~

### Poly + Foreign Definitions

~~~poly fragment
#rust
use std::collections::HashMap;

fn complex_algorithm(data: &[i32]) -> Vec<i32> {
    data.iter().filter(|&&x| x > 0).map(|&x| x * 2).collect()
}
#endrust

# Poly orchestrates the calls
var count i32 := 0
loop i 0..5
    add count
end loop
put "Count: " + count

var data := [1, -2, 3, 4, -5]
var result := complex_algorithm(data)
put "Rust result: " + result
~~~

### Variables & Output

~~~poly
var name ustring := unicode "Poly"
var version i32 := 1
put "Language: " + name + ", Version: " + version
~~~

### Typed Input

~~~poly
put "Enter your age: "
var age i32 := get --as i32
put "You are " + age + " years old."
~~~

### Error Handling

~~~poly fragment
enum FileError
    NotFound
    PermissionDenied
end enum

fn read_config(path: ustring): Result<ustring, FileError>
    var content := try read_file(path)  // `read_file` returns a Result
    return Ok(content)
end fn

match read_config(unicode "config.txt")
    Ok(content), put "Config loaded: " + content
    Error(NotFound), error "Config file not found"
    Error(PermissionDenied), error "Permission denied"
end match
~~~

### Loop Ranges (SuperBASIC-inspired)

The loop variable must be named explicitly after `loop`: `loop <var_name> <ranges>`. `loop` without a variable is not part of the current v2 syntax.
Collection iteration uses `loop <var_name> in <collection>`. Loop ranges include both endpoints, so `1..3` iterates `1, 2, 3`; `..=` is accepted but redundant for `loop` ranges.

~~~poly
# Simple range
loop i 0..10
    put i
end loop

# Multiple ranges and specific values
loop value 1..3, 7, 19..20
    put value  # Iterates: 1, 2, 3, 7, 19, 20
end loop

# With step
loop i 0..10 step 2
    put i  # Iterates: 0, 2, 4, 6, 8, 10
end loop
~~~

---

## Features

### Foreign Language Blocks

| Feature | Syntax | Description |
|---------|--------|-------------|
| Rust block | `#rust ... #endrust` | Rust definitions emitted at module scope |
| C block | `#c ... #endc` | C declarations emitted at file scope |
| C++ block | `#cpp ... #endcpp` | Reserved; rejected until a backend exists |

**Rule:** Foreign blocks are top-level target-language definitions and are opaque to Poly. Poly always generates `fn main()` for Rust or `int main(void)` for C. The native compiler validates foreign contents.

### I/O System

| Feature | Syntax | Description |
|---------|--------|-------------|
| Basic output | `put expr` | Print with newline (vectors print with `{:?}` debug formatting) |
| File write | `put expr to "file"` | Write/overwrite file |
| File append | `put expr to "file" -append` | Append to file |
| Error output | `error expr` | Print to stderr with `[ERROR]` |
| Warning output | `warn expr` | Print to stderr with `[WARN]` |
| Debug output | `info expr` | Print to stderr with `[INFO]` |
| Basic input | `var x := get` | Read from stdin |
| Input with prompt | `var x ustring := get unicode "Prompt:"` | Display a Unicode prompt, then read |
| Default value | `get --default unicode "value"` | Fallback on empty input |
| Masked input | `get --mask unicode "*"` | Hide password input |
| Timeout | `get --timeout 3000` | Timeout in milliseconds |
| Type conversion | `get --as i32` | Parse input as a target type |
| Delimited input | `get --until unicode ","` | Read through a delimiter |
| File input | `var x := get from "file"` | Read from file |
| Binary input | `var x bytes := get from "file"` | Read as bytes |

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
| `char` | 4 bytes | `char` |
| `string` | byte-oriented | `Vec<u8>` / `&[u8]` |
| `uchar` | 4 bytes | `char` |
| `ustring` | 1-4 bytes/char | `String` / `&str` |
| `byte` | 1 byte | `u8` |
| `bytes` | Variable | `Vec<u8>` |
| `ptr T` | Pointer size | `*const T` |

### Language Constructs

- **Variables**: `var name Type := value` (mutable; type optional), `let name: Type := value` (immutable)
- **Constants**: `const NAME := value`
- **Operators**: `=` compares values; `:=` initializes and assigns; `==` is rejected legacy syntax
- **Functions**: `fn name(params): ReturnType ... end fn`; generic syntax is modeled for Rust-oriented programs
- **Structs**: `struct Name ... end struct`, with generic parameters: `struct Wrapper<T>`
- **Enums**: `enum Name ... end enum`
- **Traits**: `trait Name ... end trait`
- **Impls**: `impl Trait for Type ... end impl`
- **Modules**: `module name ... end module`
- **Macros**: `macro name(params) ... end macro` (compile-time text expansion with substitution)
- **Closures**: `|params| expr` or `|params| ... end`
- **Function types**: `fn apply(f: |x: i32| i32, v: i32): i32` — closures as
  typed parameters; `fn make_adder(n: i32): |x: i32| i32` returns a closure
- **Rust-oriented higher-order methods**: vector iterator chains are supported by the Rust backend; use `#c` helpers for C-specific collection algorithms
- **If/Else**: `if cond ... else ... end if`
- **While**: `while cond ... end while`
- **Loop**: `loop ... end loop` (infinite), `loop variable range ... end loop` (range), `loop variable in collection ... end loop`
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

Poly currently targets Rust by default and supports a documented C11 orchestration subset with `--target c`. Every supported Poly construct has a direct equivalent in the selected target:

| Poly | Rust |
|------|------|
| `var x i32 := 0` | `let mut x: i32 = 0;` |
| `const MAX := 100` | `const MAX: i32 = 100;` |
| `x := value` | `x = value;` |
| `x := x + 1` | `x += 1;` |
| `put "hello"` | `println!("{}", "hello");` |
| `get` | Standard input reading |
| `if x > 0` | `if x > 0 {` |
| `loop i 0..10` | `for i in 0..=10 {` |
| `fn add(a: i32, b: i32): i32` | `fn add(a: i32, b: i32) -> i32` |
| `struct Point` | `struct Point` |
| `enum Shape` | `enum Shape` |
| `match x { ... }` | `match x { ... }` |
| `try expr` | `expr?` |

---

## Examples

### Prime Numbers

~~~poly fragment
fn is_prime(n: i32): bool
    if n <= 1,
        return false
    end if
    var i i32 := 2
    while i * i <= n
        if n % i = 0
            return false
        end if
        i += 1
    end while
    return true
end fn

put unicode "First 20 prime numbers:"
var count i32 := 0
loop num 2..200
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

~~~poly fragment
fn main()
    var running bool := true
    while running
        put "Menu:"
        put "1. Greet user"
        put "2. Exit"
        put "Choose: "
        var choice i32 := get
        match choice
            1, greet_user()
            2, running := false
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
    put xs.filter(|x| x % 2 = 0)     # [2, 4]
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
| `poly --check <file>` | Validate code and verify the selected target compilation |
| `poly --emit-rust <file>` | Print generated Rust to stdout |
| `poly --intermediate-representation <file>` | Print the intermediate representation pipeline output (optimized Rust); `--ir` is an alias |
| `poly --source-map <file>` | Print the generated Poly→Rust source map |
| `poly --format <file>` | Format output with rustfmt |
| `poly --diff <file>` | Show diff between unformatted and formatted |
| `poly --watch <file>` | Watch file and re-transpile on changes |
| `poly --project <dir> <file>` | Generate a Rust Cargo project or a C project with `--target c` |
| `poly --repl` | Start interactive REPL |
| `poly --help` | Show help message |
| `poly --version` | Show version information |
| `poly --target rust --check file.poly` | Validate the Rust backend |
| `poly --target c --emit-c file.poly` | Emit C source to stdout |
| `poly --target c --check file.poly` | Validate the C backend |
| `poly --target c file.poly` | Generate and build a C program |

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
