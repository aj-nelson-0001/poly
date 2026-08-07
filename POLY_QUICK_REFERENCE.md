# Poly Language Quick Reference Card

## I/O Commands

### Output (`put`)

```poly
put expression               # Output with newline
put -n expression            # Output without newline
put expression > "file"      # Write to file
put expression >> "file"     # Append to file
```

### Error/Warning Output

```poly
error expression             # Error message to stderr
warn expression              # Warning message to stderr
info expression              # Debug info to stderr
```

### Input (`get`)

```poly
var x = get                  # Read line from stdin
var x = get "prompt: "       # Read with prompt
var x: i32 = get             # Auto-parse typed input
var x = get < "file"         # Read from file
var x: bytes = get < "file"  # Read binary from file
```

### Input Flags

```poly
var x = get --timeout 5000       # Read with timeout (ms)
var x = get --default u"val"     # Read with default value
var x = get --mask u"*"          # Read with mask (password)
var x = get --as i32             # Read and convert to type
var x = get --until u","         # Read until delimiter
var x = get < "file" --bytes 8   # Read specific number of bytes
```

### Complex Input Options

```poly
var x = get with validate |x| x > 0       # Validation closure
var x = get with complete [u"a", u"b"]    # Completion array
var x = get with encoding u"utf-8"        # Encoding specification
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

### Pattern Matching

```poly
match result
    Ok(value) => process(value)
    Error(e) => handle_error(e)
end match
```

### Wildcard Pattern

```poly
match result
    Ok(value) => process(value)
    Error(_) => error "Something went wrong"  # Catches any error
end match
```

### Error Propagation

```poly
fn risky_operation(): Result<T, E>
    var result = try other_operation()  # Propagates error
    return Ok(result)
end fn
```

### Custom Error Types

```poly
enum MyError
    NotFound
    InvalidInput(message: ustring)
    Timeout(ms: i32)
end enum

fn validate(): Result<ustring, MyError>
    if invalid then
        return Error(MyError::InvalidInput(u"Bad data"))
    end if
    return Ok(u"valid")
end fn
```

---

## Control Flow

### If/Else

```poly
if condition then
    // code
else if other_condition then
    // code
else
    // code
end if
```

### While Loop

```poly
while condition
    // code
end while
```

### Loop (Infinite)

```poly
loop
    // code
end loop
```

### Loop Ranges (SuperBASIC-style)

```poly
loop: 0..10
    // code
end loop

loop: 1..3, 7, 19..20
    // iterates: 1, 2, 3, 7, 19, 20
    // code
end loop

loop: 1..10 step 2
    // iterates: 1, 3, 5, 7, 9
    // code
end loop

loop: collection
    // code
end loop
```

### Match

```poly
match value
    pattern1 => expression1
    pattern2 => expression2
    _ => default_expression
end match
```

---

## Functions

### Basic Function

```poly
fn function_name(param: Type): ReturnType
    // code
    return value
end fn
```

### Function with Error Handling

```poly
fn risky_function(): Result<T, E>
    // code that might fail
    return Ok(value)
    // or
    return Error(error_value)
end fn
```

---

## Variables

### Declaration

```poly
var name: Type = value        # Mutable variable
let name: Type = value        # Immutable variable
const NAME = value            # Constant
```

### Type Inference

```poly
var x = 42                    # Inferred as i32
var s = u"hello"              # Inferred as ustring
var b = true                  # Inferred as bool
```

---

## Data Types

### Primitives

```poly
i32, i64, u32, u64           # Integers
f32, f64                     # Floats
bool                         # Boolean
char, uchar                  # Characters
ustring                      # Unicode strings
bytes                        # Byte arrays
```

### Collections

```poly
Vec<T>                       # Dynamic array
Map<K, V>                    # Hash map
Set<T>                       # Hash set
```

### Compound Types

```poly
struct Name
    field: Type
end struct

enum Name
    Variant1
    Variant2(data: Type)
end enum
```

---

## Common Patterns

### Interactive Input

```poly
put -n "Enter your name: "
var name: ustring = get --default u"Anonymous"
put "Hello, " + name + "!"
```

### Validated Input

```poly
var age: i32 = get with validate |x| x > 0 && x < 150
```

### Timeout Handling

```poly
match get --timeout 3000
    Ok(input) => process(input)
    Timeout => warn "Too slow!"
    Error(e) => error "Error: " + e
end match
```

### File Operations

```poly
# Write to file
put "data" > "output.txt"
put "more" >> "output.txt"

# Read from file
var content: ustring = get < "input.txt"
var data: bytes = get < "binary.bin"
```

### Error Recovery

```poly
var valid: i32 = loop
    put -n "Enter a number: "
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

---

## Transpilation to Rust

| Poly | Rust |
|------|------|
| `put expr` | `println!("{}", expr);` |
| `put -n expr` | `print!("{}", expr);` |
| `error expr` | `eprintln!("[ERROR] {}", expr);` |
| `warn expr` | `eprintln!("[WARN] {}", expr);` |
| `info expr` | `eprintln!("[INFO] {}", expr);` |
| `get` | `stdin().read_line()` |
| `Error(e)` | `Err(e)` |
| `Ok(val)` | `Ok(val)` |
| `try expr` | `expr?` |
