# Poly Language API Reference

## Output Commands

### `put` - Standard Output

```poly
put expression               # Output with newline
put -n expression            # Output without newline
put expression > "file"      # Write to file (truncate)
put expression >> "file"     # Append to file
```

**Parameters:**
- `expression`: Any value (string, number, boolean, etc.)
- `-n` flag: Suppress trailing newline

**Returns:** Nothing

**Examples:**
```poly
put "Hello, World!"
put 42
put -n "Loading..."
put "data" > "output.txt"
put "more" >> "output.txt"
```

---

### `error` - Error Output (stderr)

```poly
error expression
```

**Parameters:**
- `expression`: Error message

**Returns:** Nothing

**Output:** Automatically prefixed with `[ERROR]`

**Examples:**
```poly
error "File not found"
error "Error: " + e
```

---

### `warn` - Warning Output (stderr)

```poly
warn expression
```

**Parameters:**
- `expression`: Warning message

**Returns:** Nothing

**Output:** Automatically prefixed with `[WARN]`

**Examples:**
```poly
warn "Deprecated function"
warn "Memory usage high"
```

---

### `info` - Debug Output (stderr)

```poly
info expression
```

**Parameters:**
- `expression`: Debug information

**Returns:** Nothing

**Output:** Automatically prefixed with `[INFO]`

**Examples:**
```poly
info "Request took 42ms"
info "Memory: 128MB"
```

---

## Input Commands

### `get` - Read Input

```poly
var x = get                          # Read line from stdin
var x = get "prompt: "               # Read with prompt
var x: Type = get                    # Auto-parse typed input
var x = get < "file"                 # Read from file
var x: bytes = get < "file"         # Read binary from file
var x: bytes = get < "file" --bytes 8  # Read specific number of bytes
```

**Parameters:**
- `"prompt: "`: Optional prompt string
- `--bytes N`: Read N bytes (optional)

**Returns:** Input value (type depends on variable)

**Examples:**
```poly
var name: ustring = get
var age: i32 = get
var data: bytes = get < "binary.bin"
var header: bytes = get < "image.png" --bytes 8
```

---

### Input Flags

#### `--timeout` - Read with Timeout

```poly
var x = get --timeout milliseconds
```

**Parameters:**
- `milliseconds`: Timeout in milliseconds

**Returns:** `Result<T, TimeoutError>`

**Examples:**
```poly
match get --timeout 3000
    Ok(input) => process(input)
    Timeout => warn "Too slow!"
    Error(e) => error "Error: " + e
end match
```

---

#### `--default` - Read with Default Value

```poly
var x = get --default value
```

**Parameters:**
- `value`: Default value if input is empty

**Returns:** Input value or default

**Examples:**
```poly
var color: ustring = get --default u"blue"
var count: i32 = get --default 0
```

---

#### `--mask` - Read with Input Mask

```poly
var x = get --mask mask_char
```

**Parameters:**
- `mask_char`: Character to display (e.g., `u"*"`)

**Returns:** Hidden input value

**Examples:**
```poly
var password: ustring = get --mask u"*"
```

---

#### `--as` - Type Conversion

```poly
var x = get --as Type
```

**Parameters:**
- `Type`: Target type for conversion

**Returns:** Converted value

**Examples:**
```poly
var num: i32 = get --as i32
var person: Person = get --as Person
```

---

#### `--until` - Delimiter-Based Input

```poly
var x = get --until delimiter
```

**Parameters:**
- `delimiter`: Delimiter string

**Returns:** Input until delimiter

**Examples:**
```poly
var csv_line: ustring = get --until u","
var field: ustring = get --until u"\n"
```

---

#### `--bytes` - Read Specific Number of Bytes

```poly
var x = get < "file" --bytes count
```

**Parameters:**
- `count`: Number of bytes to read

**Returns:** Bytes value

**Examples:**
```poly
var header: bytes = get < "image.png" --bytes 8
```

---

### Complex Input Options

#### `with validate` - Input Validation

```poly
var x = get with validate closure
```

**Parameters:**
- `closure`: Validation function `(value) -> bool`

**Returns:** Validated value

**Examples:**
```poly
var age: i32 = get with validate |x| x >= 1 && x <= 150
var email: ustring = get with validate |e| e.contains(u"@")
```

---

#### `with complete` - Input Completion

```poly
var x = get with complete array
```

**Parameters:**
- `array`: Array of completion options

**Returns:** Selected value

**Examples:**
```poly
var command: ustring = get with complete [u"start", u"stop", u"pause"]
```

---

#### `with encoding` - Encoding Specification

```poly
var x = get with encoding encoding_name
```

**Parameters:**
- `encoding_name`: Encoding string (e.g., `u"utf-8"`)

**Returns:** Decoded value

**Examples:**
```poly
var text: ustring = get < "file.txt" with encoding u"utf-8"
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

**Variants:**
- `Ok(T)`: Success with value of type `T`
- `Error(E)`: Failure with error of type `E`

---

### Error Propagation

```poly
var result = try risky_operation()
```

**Behavior:**
- If `Ok(value)`: Unwraps to `value`
- If `Error(e)`: Propagates error up the call stack

**Examples:**
```poly
fn process(): Result<ustring, Error>
    var data = try read_file(u"config.txt")  # Propagates error
    return Ok(data)
end fn
```

---

### Pattern Matching

```poly
match result
    Ok(value) => handle_success(value)
    Error(e) => handle_error(e)
end match
```

**Examples:**
```poly
match read_file(u"config.txt")
    Ok(content) => process(content)
    Error(FileError::NotFound) => error "File not found"
    Error(FileError::PermissionDenied) => error "Permission denied"
    Error(e) => error "Unknown error"
end match
```

---

### Wildcard Pattern

```poly
match result
    Ok(value) => process(value)
    Error(_) => error "Something went wrong"
end match
```

---

## Custom Error Types

```poly
enum MyError
    NotFound
    InvalidInput(message: ustring)
    Timeout(ms: i32)
end enum

fn validate(): Result<ustring, MyError>
    if invalid,
        return Error(MyError::InvalidInput(u"Bad data"))
    end if
    return Ok(u"valid")
end fn
```

---

## Transpilation Reference

### Output Commands

| Poly | Rust |
|------|------|
| `put expr` | `println!("{}", expr);` |
| `put -n expr` | `print!("{}", expr);` |
| `error expr` | `eprintln!("[ERROR] {}", expr);` |
| `warn expr` | `eprintln!("[WARN] {}", expr);` |
| `info expr` | `eprintln!("[INFO] {}", expr);` |
| `put expr > "f"` | `std::fs::write("f", expr);` |
| `put expr >> "f"` | Append with `writeln!` |

### Input Commands

| Poly | Rust |
|------|------|
| `var x = get` | `stdin().read_line()` |
| `var x: i32 = get` | `read_line() + parse()` |
| `get --timeout 5000` | Thread with timeout |
| `get --default u"val"` | `unwrap_or_default()` |
| `get --mask u"*"` | Terminal raw mode |
| `get --as i32` | Type conversion |
| `get --until u","` | Read until delimiter |
| `get --bytes 8` | Read N bytes |
| `get with validate \|x\| ...` | Validation loop |
| `get with complete [...]` | Line editor completion |

### Error Handling

| Poly | Rust |
|------|------|
| `Ok(val)` | `Ok(val)` |
| `Error(e)` | `Err(e)` |
| `try expr` | `expr?` |
| `match` | `match` |
