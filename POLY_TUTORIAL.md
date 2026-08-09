# Poly Language Tutorial: I/O and Error Handling

## Introduction

This tutorial covers the fundamental I/O operations and error handling in Poly. By the end, you'll be able to read input, display output, and handle errors gracefully.

---

## Part 1: Output with `put`

### Basic Output

The `put` command outputs text to the console with a newline character:

```poly
put "Hello, World!"           # Output with newline
put 42                         # Output number
put 3.14159                    # Output float
put true                       # Output boolean
```

### Output Without Newline

Use the `-n` flag to suppress the trailing newline:

```poly
put -n "Loading"
put -n "."
put -n "."
put "."                        # This one adds a newline
put "Done!"
```

Output:
```
Loading...
Done!
```

### File Output

Write to files using redirection operators:

```poly
put "Line 1" > "output.txt"    # Write (truncate)
put "Line 2" >> "output.txt"   # Append
```

### Error/Warning Output

Use `error`, `warn`, and `info` for different output levels:

```poly
error "Something went wrong"   # Error message (stderr)
warn "Deprecated feature"      # Warning message (stderr)
info "Debug information"       # Debug info (stderr)
```

---

## Part 2: Input with `get`

### Basic Input

Read a line from the user:

```poly
put -n "Enter your name: "
var name: ustring = get
put "Hello, " + name + "!"
```

### Typed Input

Poly automatically parses input based on the variable type:

```poly
put -n "Enter your age: "
var age: i32 = get
put "In 10 years you will be: " + (age + 10)
```

### Input with Default Values

Use `--default` for optional input:

```poly
put -n "Enter color (or press Enter for default): "
var color: ustring = get --default u"blue"
put "Color: " + color
```

### Password Input

Use `--mask` to hide input:

```poly
put -n "Enter password: "
var password: ustring = get --mask u"*"
put "Password length: " + password.len()
```

### Input with Timeout

Use `--timeout` to prevent hanging:

```poly
match get --timeout 3000
    Ok(input) => put "You typed: " + input
    Timeout => warn "Too slow!"
    Error(e) => error "Error: " + e
end match
```

### Input with Validation

Use `with validate` to validate input:

```poly
var age: i32 = get with validate |x| x >= 1 && x <= 150
```

### Delimiter-Based Input

Use `--until` to read until a delimiter:

```poly
put -n "Enter CSV line: "
var line: ustring = get --until u","
put "First field: " + line
```

---

## Part 3: Error Handling

### Result Type

Poly uses `Result<T, E>` for operations that can fail:

```poly
enum Result<T, E>
    Ok(T)
    Error(E)
end enum
```

### Basic Error Handling

```poly
fn divide(a: f64, b: f64): Result<f64, ustring>
    if b == 0.0,
        return Error(u"Division by zero")
    end if
    return Ok(a / b)
end fn

match divide(10.0, 2.0)
    Ok(result) => put "Result: " + result.to_string()
    Error(e) => error "Error: " + e
end match
```

### Custom Error Types

Define specific error types for better error handling:

```poly
enum FileError
    NotFound
    PermissionDenied
    InvalidData
end enum

fn read_file(path: ustring): Result<ustring, FileError>
    if path.len() == 0,
        return Error(FileError::NotFound)
    end if
    return Ok(u"File content")
end fn
```

### Error Propagation

Use `try` to propagate errors up the call stack:

```poly
fn process_file(): Result<ustring, FileError>
    var content = try read_file(u"config.txt")  # Propagates error
    return Ok(content)
end fn
```

### Pattern Matching with Data

Extract data from error variants:

```poly
enum ValidationError
    EmptyInput
    TooShort(min: i32)
    TooLong(max: i32)
end enum

fn validate_name(name: ustring): Result<ustring, ValidationError>
    if name.len() == 0,
        return Error(ValidationError::EmptyInput)
    end if
    if name.len() < 2,
        return Error(ValidationError::TooShort(2))
    end if
    return Ok(name)
end fn

match validate_name(u"John")
    Ok(valid_name) => put "Valid: " + valid_name
    Error(EmptyInput) => error "Name cannot be empty"
    Error(TooShort(min)) => error "Name too short, minimum " + min.to_string()
    Error(TooLong(max)) => error "Name too long, maximum " + max.to_string()
end match
```

### Wildcard Pattern

Use `_` to catch any error:

```poly
match validate_name(input)
    Ok(name) => put "Valid: " + name
    Error(_) => error "Validation failed"  # Catches any error
end match
```

---

## Part 4: Complete Example

```poly
# User Registration Form

fn main()
    put "=== User Registration ==="
    put ""
    
    # Get name with validation
    put -n "Enter your name: "
    var name: ustring = get with validate |n| n.len() >= 2
    
    # Get email with validation
    put -n "Enter your email: "
    var email: ustring = get with validate |e| e.contains(u"@")
    
    # Get password with mask
    put -n "Enter password: "
    var password: ustring = get --mask u"*" with validate |p| p.len() >= 8
    
    # Confirm registration
    put ""
    put "Registration successful!"
    put "Name: " + name
    put "Email: " + email
    put "Password: " + "*".repeat(password.len())
end fn
```

---

## Part 5: Loop Ranges (SuperBASIC-inspired)

### Basic Ranges

Poly's `loop` command with colon syntax supports flexible iteration inspired by Sinclair QL SuperBASIC:

```poly
# Simple range
loop: 0..10
    put i
end loop

# Inclusive range
loop: 0..=10
    put i
end loop
```

### Multiple Ranges and Values

The real power comes from combining multiple ranges and specific values:

```poly
# Multiple ranges and specific values
loop: 1..3, 7, 19..21
    put i  # Iterates: 1, 2, 3, 7, 19, 20, 21
end loop

# Specific values only
loop: 1, 5, 10, 100
    put i  # Iterates: 1, 5, 10, 100
end loop

# Complex mix
loop: 1..5, 10, 20..25 step 2, 100
    put i  # Iterates: 1, 2, 3, 4, 5, 10, 20, 22, 24, 100
end loop
```

### Steps

Use `step` to control the increment:

```poly
# Positive step
loop: 0..10 step 2
    put i  # Iterates: 0, 2, 4, 6, 8
end loop

# Negative step (counting down)
loop: 10..1 step -1
    put i  # Iterates: 10, 9, 8, ..., 1
end loop
```

### Collection Iteration

Iterate over collections and with indices:

```poly
# Iterate over collection
var fruits: Vec<ustring> = [u"apple", u"banana", u"cherry"]
loop: fruits
    put fruit
end loop

# Iterate with index
loop: (index, fruit) in fruits.enumerate()
    put index.to_string() + ": " + fruit
end loop
```

### Practical Example

```poly
# Multiplication table
put "Multiplication Table (1..5)"
loop: 1..5
    var row: ustring = ""
    loop: 1..5
        row = row + (i * j).to_string().pad_left(4)
    end loop
    put row
end loop
```

---

## Summary

- **Output**: Use `put` for console output, `put -n` for no newline
- **Input**: Use `get` with flags like `--default`, `--mask`, `--timeout`, `--until`
- **Validation**: Use `with validate` for input validation
- **Errors**: Use `Result<T, E>` with `Ok(value)` and `Error(e)`
- **Pattern Matching**: Use `match` to handle different cases
- **Error Propagation**: Use `try` to propagate errors
- **Loop Ranges**: Use `loop:` with ranges, multiple values, and steps

Practice these concepts by building small programs that read user input, validate it, and handle errors gracefully.
