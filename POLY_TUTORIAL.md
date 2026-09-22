# Poly Language Tutorial: I/O and Error Handling

> This tutorial is current for Poly 2.0.0-preview.17. Every complete example
> parses, type-checks, and compiles with the v2 compiler.

## Introduction

This tutorial covers the fundamental I/O operations and error handling in Poly. By the end, you'll be able to read input, display output, and handle errors gracefully.

---

## Using Comments

Comments are ignored by the compiler and exist for the human reader. Poly
supports three comment styles:

~~~poly
# A hash comment runs from `#` to the end of the line.
# This is the conventional Poly comment style.

// A C++-style line comment also runs to the end of the line.

/* A block comment can span
   several lines. */
~~~

A comment may sit on its own line or trail a line of code:

~~~poly
fn main()
    var attempts i32 := 0    # Track how many times we have asked
    # TODO: cap the number of retries
    put attempts
end fn
~~~

Note that `#` only starts a comment when it does not name a foreign code block:
a line beginning `#rust` or `#c` opens a foreign block instead.

The rest of this tutorial uses `#` comments to explain what each example does.

---

## Part 1: Output with `put`

### Basic Output

The `put` command outputs text to the console with a newline character:

~~~poly
fn main()
    put "Hello, World!"   # Strings print as written
    put 42                # Integers print in decimal
    put 3.14159           # Floats keep their decimal point
    put true              # Booleans print as true / false
end fn
~~~

### Output

Use `put` to print to the console. Each `put` adds a newline:

~~~poly
fn main()
    put "Hello, World!"   # Writes the text, then a newline
    put 42                # Numbers are formatted automatically
end fn
~~~

Output:
~~~
Hello, World!
42
~~~

### File Output

Write to files using redirection operators:

~~~poly
fn main()
    # `to` creates the file if needed and overwrites its contents
    put "Line 1" to "output.txt"
    # `-append` keeps existing contents and adds to the end
    put "Line 2" to "output.txt" -append
end fn
~~~

### Error/Warning Output

Use `error`, `warn`, and `info` for different output levels:

~~~poly
fn main()
    # All three write to stderr; pick the one matching the severity
    error "Something went wrong"   # Failures the user must see
    warn "Deprecated feature"      # Suspicious but non-fatal conditions
    info "Debug information"       # Diagnostics for developers
end fn
~~~

---

## Part 2: Input with `get`

### Basic Input

Read a line from the user:

~~~poly
fn main()
    put "Enter your name: "       # Prompt the user (put adds the newline)
    var name ustring := get       # get reads one line from stdin
    put "Hello, " + name + "!"
end fn
~~~

### Typed Input

Use `--as` to parse the input line as a specific type:

~~~poly
fn main()
    put "Enter your age: "
    var age i32 := get --as i32   # Parse the line as an integer
    put "In 10 years you will be: " + (age + 10)
end fn
~~~

### Input with Default Values

Use `--default` for optional input:

~~~poly
fn main()
    put "Enter color (or press Enter for default): "
    # --default kicks in when the user presses Enter on an empty line
    var color ustring := get --default unicode "blue"
    put "Color: " + color
end fn
~~~

### Password Input

Use `--mask` to hide input; echo is suppressed while typing on Unix terminals,
and the read falls back to plain input elsewhere:

~~~poly
fn main()
    put "Enter password: "
    # --mask echoes the given character instead of the typed keys
    var password ustring := get --mask unicode "*"
    put "Password length: " + password.len()
end fn
~~~

### Input with Timeout

Use `--timeout` to prevent hanging:

~~~poly
fn main()
    # --timeout gives up after 3000 ms; match on the possible outcomes
    match get --timeout 3000
        Ok(input), put "You typed: " + input   # Input arrived in time
        Timeout, warn "Too slow!"              # No input before the deadline
        Error(e), error "Error: " + e          # The read itself failed
    end match
end fn
~~~

### Input with Validation

> **Note:** The `with validate` clause is a parser-only compatibility form and
> is not part of the maintained runnable v2 API — the closure is never executed
> at runtime (see the v2 spec). The word `validate` is reserved for it and
> cannot name variables, functions, or methods. Validate input with a loop
> instead:

~~~poly
fn main()
    put "Enter age: "
    var done bool := false
    var age i32 := 0
    while not done                 # Keep asking until the input is valid
        var input i32 := get --as i32
        if input >= 1 and input <= 150
            age := input           # Accept: record it and stop looping
            done := true
        else
            put "Age must be between 1 and 150"   # Reject: explain, then retry
        end if
    end while
end fn
~~~

### Delimiter-Based Input

Use `--until` to read until a delimiter; input stops at the delimiter (which is
excluded from the result), so `--until unicode ","` on `apple,banana` yields `apple`:

~~~poly
fn main()
    put "Enter CSV line: "
    # --until stops at the delimiter; the delimiter itself is not returned
    var line ustring := get --until unicode ","
    put "First field: " + line
end fn
~~~

---

## Part 3: Error Handling

### Result Type

Poly uses `Result<T, E>` for operations that can fail:

~~~poly fragment
# A Result has two variants: Ok carries the success value,
# Error carries a description of what went wrong.
enum Result<T, E>
    Ok(T)
    Error(E)
end enum
~~~

### Basic Error Handling

~~~poly
fn main()
    fn divide(a: f64, b: f64): Result<f64, ustring>
        # Guard clause: reject the bad case instead of dividing by zero
        if b = 0.0,
            return Error(unicode "Division by zero")
        end if
        return Ok(a / b)           # Ok wraps the successful value
    end fn

    # match handles both variants of the Result
    match divide(10.0, 2.0)
        Ok(result), put "Result: " + result.to_string()
        Error(e), error "Error: " + e
    end match
end fn
~~~

### Custom Error Types

Define specific error types for better error handling:

~~~poly
# A dedicated error enum makes every failure mode explicit
enum FileError
    NotFound
    PermissionDenied
    InvalidData
end enum

fn read_file(path: ustring): Result<ustring, FileError>
    if path.len() = 0,
        # Return an Error variant to fail, Ok(...) to succeed
        return Error(FileError::NotFound)
    end if
    return Ok(unicode "File content")
end fn
~~~

### Error Propagation

Use `try` to propagate errors up the call stack:

~~~poly fragment
fn process_file(): Result<ustring, FileError>
    # If read_file fails, `try` returns the Error from process_file
    # immediately; otherwise content holds the unwrapped Ok value.
    var content := try read_file(unicode "config.txt")
    return Ok(content)
end fn
~~~

### Pattern Matching with Data

Extract data from error variants:

~~~poly
# Variants can carry data, which match arms can bind to names
enum ValidationError
    EmptyInput
    TooShort(min: i32)
    TooLong(max: i32)
end enum

fn validate_name(name: ustring): Result<ustring, ValidationError>
    if name.len() = 0,
        return Error(ValidationError::EmptyInput)
    end if
    if name.len() < 2,
        # Attach the offending limit to the error itself
        return Error(ValidationError::TooShort(2))
    end if
    return Ok(name)
end fn

# Each arm names a variant; payload fields bind as local names
fn main()
match validate_name(unicode "John")
    Ok(valid_name), put "Valid: " + valid_name
    Error(EmptyInput), error "Name cannot be empty"
    Error(TooShort(min)), error "Name too short, minimum " + min.to_string()   # `min` comes from the variant
    Error(TooLong(max)), error "Name too long, maximum " + max.to_string()     # `max` comes from the variant
end match
end fn
~~~

### Wildcard Pattern

Use `_` to catch any error:

~~~poly fragment
# `_` matches anything without binding it — use it as the catch-all arm
match validate_name(input)
    Ok(name), put "Valid: " + name
    Error(_), error "Validation failed"  # Catches any error variant
end match
~~~

---

## Part 4: Match Expressions and Enum Payloads

### Match as an Expression

`match` is not just a statement — it produces a value. Each arm's expression
becomes the result, and arms may group alternatives with a comma-separated
pattern list. A match **expression** must include a wildcard `_` arm so every
possible scrutinee value has a result:

~~~poly
fn main()
    var level i32 := 2
    # The whole match evaluates to the matched arm's expression
    var label := match level
        0, "off"        # alternatives can share an arm: 0 or 1
        1, "low"
        _, "high"       # required: the wildcard covers everything else
    end match
    put label           # high
end fn
~~~

Omitting the wildcard is a compile error — the compiler reports the
unmatched values (for example `non-exhaustive patterns`).

### Enums with Tuple Payloads

Enum variants can carry data as tuple payloads. Declare the payload types in
the variant, construct with `Enum::Variant(values)`, and match binds each
payload position to a name:

~~~poly
# Variants carry tuple payloads: one f64, or two f64s
enum Shape
    Circle(f64)
    Rect(f64, f64)
end enum

fn main()
    # Construct a variant with :: and its payload values
    var s := Shape::Rect(3.0, 4.0)

    # Each arm binds the payload fields positionally
    match s
        Circle(r), put "circle radius " + r.to_string()
        Rect(w, h), put "area " + (w * h).to_string()
        _, put "unknown shape"
    end match
end fn
~~~

### Which Match Form to Use

- **Statement form** (Part 3): arms run statements, and guarded arms
  (`x if x > 5,`) are available. No wildcard is required when earlier arms
  cover every case you care about.
- **Expression form**: arms are single expressions and a `_` arm is
  mandatory. Use it wherever a value is expected — declarations,
  arguments, returns.
- Payloads bind **positionally in declaration order**, whatever the payload
  style: tuple payloads (`Rect(w, h)`), named fields (`Level(n: i32)`, as in
  Part 3's error variants), or `Result`'s `Ok(v)` / `Error(e)` arms. The
  binder name is yours — `Level(x)` binds the declared field `n` to `x`.

---

## Part 5: Complete Example

~~~poly
# User Registration Form

fn main()
    put "=== User Registration ==="
    put ""                          # An empty string prints just the newline

    # Get name (validated with a loop)
    put "Enter your name (2+ characters): "
    var name ustring := ""
    while name.len() < 2
        var input ustring := get
        if input.len() >= 2
            name := input
        else
            put "Name must be at least 2 characters"
        end if
    end while

    # Get email (validated with a loop)
    put "Enter your email: "
    var email ustring := ""
    while email.contains(unicode "@") = false
        var input ustring := get
        if input.contains(unicode "@")
            email := input
        else
            put "Email must contain @"
        end if
    end while

    # Get password (mask suppresses echo on Unix terminals)
    put "Enter password (8+ characters): "
    var password ustring := get --mask unicode "*"
    while password.len() < 8
        put "Password must be at least 8 characters"
        password := get --mask unicode "*"
    end while

    # Confirm registration
    put ""
    put "Registration successful!"
    put "Name: " + name
    put "Email: " + email
    put "Password: " + "*".repeat(password.len())
end fn
~~~

---

## Part 6: Loop Ranges (SuperBASIC-inspired)

### Basic Ranges

Poly's `loop` command supports flexible iteration inspired by Sinclair QL SuperBASIC. Loop ranges include both endpoints, so `1..3` iterates `1, 2, 3`:

~~~poly
fn main()
    # Simple range — both endpoints are included: prints 0 through 10
    loop i 0..10
        put i
    end loop
end fn
~~~

### Multiple Ranges and Values

The real power comes from combining multiple ranges and specific values:

~~~poly
fn main()
    # Ranges and single values mix freely in one loop header
    loop i 1..3, 7, 19..21
        put i  # Iterates: 1, 2, 3, 7, 19, 20, 21
    end loop

    # A plain value list needs no ranges at all
    loop i 1, 5, 10, 100
        put i  # Iterates: 1, 5, 10, 100
    end loop

    # Each step belongs to the range it follows
    loop i 1..5, 10, 20..25 step 2, 100
        put i  # Iterates: 1, 2, 3, 4, 5, 10, 20, 22, 24, 100
    end loop
end fn
~~~

### Steps

Use `step` to control the increment:

~~~poly
fn main()
    # Positive step counts upward through the range
    loop i 0..10 step 2
        put i  # Iterates: 0, 2, 4, 6, 8, 10
    end loop

    # Negative step counts down; start must be past the end
    loop i 10..1 step -1
        put i  # Iterates: 10, 9, 8, ..., 1
    end loop
end fn
~~~

### Collection Iteration

Iterate over collections and with indices:

~~~poly
fn main()
    # `loop x in collection` yields each element in order
    var fruits Vec<ustring> := [unicode "apple", unicode "banana", unicode "cherry"]
    loop fruit in fruits
        put fruit
    end loop

    # .enumerate() yields (position, element) pairs, starting at 0
    loop (index, fruit) in fruits.enumerate()
        put index.to_string() + ": " + fruit
    end loop
end fn
~~~

### Practical Example

~~~poly
fn main()
    # Multiplication table: outer loop picks the row, inner loop the column
    put "Multiplication Table (1..5)"
    loop i 1..5
        var row ustring := ""       # Build each row as one string
        loop j 1..5
            row := row + (i * j).to_string().pad_left(4)   # Align cells to 4 chars
        end loop
        put row                     # Print the finished row
    end loop
end fn
~~~

---

## Part 7: Structs and Methods

### Defining a Struct

A struct groups related fields under one named type. Fields are declared with
a type and no initial value:

~~~poly
# A struct groups related fields under one named type.
struct Point
    x: f64
    y: f64
end struct

fn main()
    # Build a value with a struct literal; read fields with dot syntax
    var p := Point { x: 3.0, y: 4.0 }
    put p.x + p.y
end fn
~~~

### Adding Methods with `impl`

An `impl` block attaches methods to a struct. Poly moves values into calls,
so a method that changes state takes `self` and returns it — callers must
reassign the result:

~~~poly
struct Counter
    n: i32
end struct

# impl adds methods; methods that mutate take and return self
impl Counter
    fn bump(self): Counter
        self.n := self.n + 1
        return self
    end fn
end impl

fn main()
    var c := Counter { n: 0 }
    c := c.bump()   # the call consumes c, so reassign the returned value
    c := c.bump()
    put c.n         # 2
end fn
~~~

Two rules worth remembering:

- Method calls are not in-place: `c.bump()` alone discards the result. Always
  write `c := c.bump()`.
- `spawn` and `step` are reserved words and cannot name fields or methods.

For generics, traits, or default field values, put the definition in a
`#rust` (or `#c`) foreign block and call it from Poly.

---

## Part 8: Async and `spawn`

### Await a Future

An `async fn` returns a future; attach `.await` to suspend until it completes.
Only async functions can use `.await`, and an async entry point is declared
with `async fn main()`:

~~~poly
# Async functions return futures; attach .await to run them.
async fn compute(x: i32): i32
    delay(10).await          # suspend this task for 10 ms
    return x * x
end fn

async fn main()              # an async entry point can .await
    # Sequential: each .await suspends until that future completes
    var a := compute(3).await
    var b := compute(4).await
    put a + b                # 25

    # spawn launches a task without waiting for it (fire-and-forget)
    spawn compute(5)
end fn
~~~

### Spawning Independent Tasks

`spawn expr` launches `expr` as a background task (a Tokio task on the Rust
target) and moves on immediately. Nothing in the language consumes the task's
result, so spawn is for side-effecting work, not for values you need later.
`spawn` is a reserved word and cannot name variables or methods.

### Notes and Limits

- Async is a Rust-target feature. The C, assembly, and JS backends reject
  async functions with a diagnostic that points at foreign blocks.
- `delay(ms)` is the builtin sleep; `.await` it inside an async function.
- Awaiting in a loop runs the futures one at a time; to run work in
  parallel, `spawn` it and `.await` nothing.

Complete async programs live in `examples/async_await.poly` and
`examples/async_demo.poly`.

---

## Summary

- **Output**: Use `put` for console output (always adds a newline)
- **Input**: Use `get` with flags like `--default`, `--mask`, `--timeout`, `--until`
- **Validation**: Validate input with a loop (`with validate` is not part of
  the runnable v2 API)
- **Errors**: Use `Result<T, E>` with `Ok(value)` and `Error(e)`
- **Pattern Matching**: Use `match` to handle different cases
- **Error Propagation**: Use `try` to propagate errors
- **Loop Ranges**: Use `loop` with ranges, multiple values, and steps
- **Comments**: Use `#` for line comments; `//` and `/* ... */` also work
- **Match Expressions**: `match` yields a value; expressions need a `_` arm
- **Enum Payloads**: Tuple payloads bind positionally in match arms
- **Structs**: Define data with `struct`, attach methods with `impl`
- **Methods**: Mutating methods take and return `self`; write `c := c.bump()`
- **Async**: Declare `async fn`, run with `.await`; `spawn` for fire-and-forget
  tasks (Rust target only)

Practice these concepts by building small programs that read user input, validate it, and handle errors gracefully.
