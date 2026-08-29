# Poly v1.5.0: Major Transpiler Improvements and New CLI Features

**Published: August 8, 2026**

We're excited to announce the release of Poly v1.5.0! This release brings significant improvements to the transpiler, new CLI features, and better type mapping. Here's what's new:

## New CLI Features

### `--format` Flag
Format your generated Rust code with rustfmt automatically:

~~~bash
poly --format examples/prime_numbers.poly
~~~

### `--diff` Flag
See the difference between unformatted and formatted code:

~~~bash
poly --diff examples/prime_numbers.poly
~~~

### `--watch` Flag
Watch a file for changes and re-transpile automatically:

~~~bash
poly --watch examples/prime_numbers.poly
~~~

### `--check` Flag
Validate your code and verify that the generated Rust compiles:

~~~bash
poly --check examples/prime_numbers.poly
~~~

## Transpiler Improvements

### Type Mapping
Poly types now map correctly to Rust types:

| Poly Type | Rust Type |
|-----------|-----------|
| `ustring` / `string` | `String` |
| `uchar` | `char` |
| `byte` | `u8` |
| `bytes` | `Vec<u8>` |

### File Operations
File write and append operations now generate correct Rust code:

~~~poly
# Write to file
put "Hello, World!" to "output.txt"

# Append to file
put "More content" to "output.txt" -append
~~~

Generates:

~~~rust
std::fs::write("output.txt", format!("{}", "Hello, World!")).unwrap();
{ let mut f = std::fs::OpenOptions::new().append(true).create(true).open("output.txt").unwrap(); writeln!(f, "{}", "More content").unwrap(); }
~~~

### Match Expressions
Fixed codegen for match arms (removed extra semicolons):

~~~poly fragment
match x
    1, put "One"
    _, put "Other"
end match
~~~

Now generates valid Rust:

~~~rust
match x {
    1 => println!("{}", "One"),
    _ => println!("{}", "Other"),
}
~~~

### Loop Variables
Loop variables now work correctly for collection iteration:

~~~poly fragment
loop record in records
    put record
end loop
~~~

Generates:

~~~rust
for record in records {
    println!("{}", record);
}
~~~

## New Examples

We've added new example files to demonstrate various language features:

- `closures.poly` - Closure syntax
- `pattern_matching.poly` - Match expressions
- `structs_enums.poly` - Struct and enum usage

## Bug Fixes

- Fixed `break` statement generating `/* statement */` instead of `break;`
- Fixed `return` statements missing semicolons in generated Rust code
- Fixed `put` statements missing semicolons in inline contexts
- Fixed file write operations not being transpiled correctly
- Fixed string type mapping (`ustring` → `String`)
- Fixed loop variable naming for collection iteration

## Testing

All 84+ tests are passing, including:

- Unit tests for the transpiler
- Integration tests for all examples
- Error recovery tests

## What's Next?

In future releases, we plan to:

- Add more complete file I/O support (BufReader for line-by-line reading)
- Improve error messages for transpilation errors
- Add more language features (generics, traits, async/await)
- Create a web-based playground

## Getting Started

~~~bash
# Build the compiler
cd compiler
cargo build --release

# Transpile an example
../target/release/poly ../examples/prime_numbers.poly

# Check your code
../target/release/poly --check ../examples/prime_numbers.poly
~~~

## Feedback

We'd love to hear your feedback! Open an issue on GitHub or start a discussion.

---

*Poly is a minimalist, assembly-inspired systems programming language that transpiles to safe, idiomatic Rust.*
