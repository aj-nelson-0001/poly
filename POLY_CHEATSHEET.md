# Poly Language Cheatsheet: I/O and Error Handling

## `put` Command - Output

### Basic Syntax
```poly
put expression               # Output to stdout (with newline)
put -n expression            # Output to stdout (no newline)
put expression > "file"      # Write to file (truncate)
put expression >> "file"     # Append to file
```

### Error/Warning Commands
```poly
error expression             # Error message to stderr
warn expression              # Warning message to stderr
info expression              # Debug/diagnostic info to stderr
```

### Examples
```poly
# Simple output
put "Hello, World!"
put 42
put 3.14159
put true

# Variable output
var name: ustring = u"Alice"
put "Name: " + name

# Formatting
var x: i32 = 42
put "Value: {x}"
put "Pi: {3.14159:.2f}"

# Capture output to variable (capture is a keyword that redirects put output to a string)
var output: ustring = capture put "Computed: " + (2 + 2)
// output now contains "Computed: 4"

# File output
put "Log entry" >> "app.log"
put "Data" > "output.txt"

# No newline output
put -n "Loading..."

# Error messages
error "Error: Something went wrong"
error "Error: File not found"

# Warnings
warn "Warning: Deprecated function"
warn "Warning: Memory usage high"

# Debug info
info "DEBUG: Request took 42ms"
info "DEBUG: Memory usage: 128MB"
```

### Transpilation to Rust
| Poly | Rust |
|------|------|
| `put expr` | `println!("{}", expr);` |
| `put -n expr` | `print!("{}", expr);` |
| `error expr` | `eprintln!("[ERROR] {}", expr);` |
| `warn expr` | `eprintln!("[WARN] {}", expr);` |
| `info expr` | `eprintln!("[INFO] {}", expr);` |
| `put expr > "f"` | `std::fs::write("f", expr);` |
| `put expr >> "f"` | Append with `writeln!` |

---

## `get` Command - Input

### Basic Syntax
```poly
var x: ustring = get                    # Read line from stdin
var x: ustring = get u"prompt: "        # Read with prompt
var x: i32 = get                        # Auto-parse typed input
var x: ustring = get < "file"           # Read from file
var x: bytes = get < "file"            # Read binary from file
var x: ustring = get --timeout 5000     # Read with timeout (ms)
var x: ustring = get --default u"val"   # Read with default value
var x: ustring = get --mask u"*"        # Read with mask (password)
var x: i32 = get --as i32               # Read and convert to type
var x: ustring = get --until u","       # Read until delimiter
```

### Complex Options (using "with" syntax)
```poly
var x: i32 = get with validate |x| x > 0    # Validation closure
var x: ustring = get with complete [u"a", u"b"]  # Completion array
var x: ustring = get with encoding u"utf-8"  # Encoding specification
```

### Examples
```poly
# Simple input
var line: ustring = get
put "You entered: " + line

# Input with prompt
var name: ustring = get "What is your name? "
put "Hello, " + name + "!"

# Typed input (auto-parsing)
put "Enter your age: "
var age: i32 = get
put "In 10 years you will be: " + (age + 10)

# Input with validation
var age: i32 = get with validate |x| x > 0 && x < 150

# Input with default value
var name: ustring = get --default u"Anonymous"

# Input with mask (password)
var password: ustring = get --mask u"*"

# Input with timeout
match get --timeout 3000
    Ok(input) => put "You typed: " + input
    Timeout => put "Too slow!"
    Error(e) => error "Error: " + e
end match
```

---

## Error Handling

### Result Type
```poly
enum Result<T, E>
    Ok(T)
    Error(E)
end enum
```

### Basic Error Handling
```poly
# Define custom error types
enum FileError
    NotFound
    PermissionDenied
    InvalidData
end enum

# Function that can fail
fn read_file(path: ustring): Result<ustring, FileError>
    if path.len() == 0 then
        return Error(FileError::NotFound)
    end if
    return Ok(u"File content")
end fn

# Handle errors with match
match read_file(u"config.txt")
    Ok(content) => process(content)
    Error(FileError::NotFound) => error "File not found"
    Error(FileError::PermissionDenied) => error "Permission denied"
    Error(FileError::InvalidData) => error "Invalid data"
end match
```

### Error Propagation
```poly
fn risky_operation(): Result<T, E>
    var result = try other_operation()  # Propagates error
    return Ok(result)
end fn
```

### Pattern Matching with Data
```poly
enum ValidationError
    EmptyInput
    TooShort(min: i32)
    TooLong(max: i32)
end enum

fn validate_name(name: ustring): Result<ustring, ValidationError>
    if name.len() == 0 then
        return Error(ValidationError::EmptyInput)
    end if
    if name.len() < 2 then
        return Error(ValidationError::TooShort(2))
    end if
    return Ok(name)
end fn

# Match with data extraction
match validate_name(u"John")
    Ok(valid_name) => put "Valid: " + valid_name
    Error(EmptyInput) => error "Name cannot be empty"
    Error(TooShort(min)) => error "Name too short, minimum " + min.to_string()
    Error(TooLong(max)) => error "Name too long, maximum " + max.to_string()
end match
```

### Wildcard Pattern
```poly
match validate_name(input)
    Ok(name) => put "Valid: " + name
    Error(_) => error "Validation failed"  # Catches any error
end match
```

### Transpilation to Rust
| Poly | Rust |
|------|------|
| `Ok(val)` | `Ok(val)` |
| `Error(e)` | `Err(e)` |
| `try expr` | `expr?` |
| `match` | `match` |

---

## Common Patterns

### Interactive Menu
```poly
loop
    put "Menu:"
    put "1. Start"
    put "2. Stop"
    put "3. Exit"
    put -n "Choose: "
    var choice: i32 = get
    match choice
        1 => start_process()
        2 => stop_process()
        3 => break
        _ => put "Invalid choice"
    end match
end loop
```

### Loop Ranges (SuperBASIC-style)
```poly
// Simple range
loop: 0..10
    put i
end loop

// Multiple ranges and specific values
loop: 1..3, 7, 19..20
    put i  // Iterates: 1, 2, 3, 7, 19, 20
end loop

// With step
loop: 1..10 step 2
    put i  // Iterates: 1, 3, 5, 7, 9
end loop

// Negative step (counting down)
loop: 10..1 step -1
    put i  // Iterates: 10, 9, 8, ..., 1
end loop

// Iterate over collection
loop: items
    put item
end loop
```

### Error Recovery
```poly
var valid_number: i32 = loop
    put -n "Enter a positive number: "
    match get
        Ok(input) =>
            var num: i32 = input.parse::<i32>()
            if num > 0 then
                break num
            else
                warn "Please enter a positive number"
            end if
        Error(e) => error "Invalid input: " + e
    end match
end loop
```
