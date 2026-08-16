# Poly Language Quick Reference Card

## I/O Commands

### Output (`put`)

~~~poly
put expression               # Output with newline
put -n expression            # Output without newline
put expression > "file"      # Write to file
put expression >> "file"     # Append to file
~~~

### Error/Warning Output

~~~poly
error expression             # Error message to stderr
warn expression              # Warning message to stderr
info expression              # Debug info to stderr
~~~

### Input (`get`)

~~~poly
var x := get                  # Read line from stdin
var x := get unicode "prompt: "       # Read with prompt
var x i32 := get             # Auto-parse typed input
var x := get < "file"         # Read from file
var x bytes := get < "file"  # Read binary from file
~~~

### Input Flags

~~~poly
var x := get --timeout 5000       # Read with timeout (ms)
var x := get --default unicode "val"     # Read with default value
var x := get --mask unicode "*"          # Experimental: mask parses but is not applied (warning)
var x := get --as i32             # Read and convert to type
var x := get --until unicode ","         # Experimental: until parses but reads to end of line (warning)
var x := get < "file" --bytes 8   # Read specific number of bytes
~~~

### Complex Input Options

~~~poly
var x := get with validate |x| x > 0       # Validation closure
var x := get with complete [unicode "a", unicode "b"]    # Completion array
var x := get with encoding unicode "utf-8"        # Encoding specification
~~~

---

## Error Handling

### Result Type

~~~poly fragment
enum Result<T, E>
    Ok(T)
    Error(E)
end enum
~~~

### Pattern Matching

~~~poly
match result
    Ok(value), process(value)
    Error(e), handle_error(e)
end match
~~~

### Wildcard Pattern

~~~poly
match result
    Ok(value), process(value)
    Error(_), error "Something went wrong"  # Catches any error
end match
~~~

### Error Propagation

~~~poly
fn risky_operation(): Result<T, E>
    var result := try other_operation()  # Propagates error
    return Ok(result)
end fn
~~~

### Custom Error Types

~~~poly fragment
enum MyError
    NotFound
    InvalidInput(message: ustring)
    Timeout(ms: i32)
end enum

fn validate(): Result<ustring, MyError>
    if invalid,
        return Error(MyError::InvalidInput(unicode "Bad data"))
    end if
    return Ok(unicode "valid")
end fn
~~~

---

## Control Flow

### If/Else

~~~poly
if condition,
    // code
else if other_condition,
    // code
else,
    // code
end if
~~~

### While Loop

~~~poly
while condition
    // code
end while
~~~

### Loop (Infinite)

~~~poly
loop
    // code
end loop
~~~

### Loop Ranges (SuperBASIC-style)

Loop ranges include both endpoints. Thus `1..3` iterates `1, 2, 3`; `..=` is accepted but redundant for `loop:` ranges.

~~~poly
loop: i 0..10
    // code
end loop

loop: value 1..3, 7, 19..20
    // iterates: 1, 2, 3, 7, 19, 20
    // code
end loop

loop: i 1..10 step 2
    // iterates: 1, 3, 5, 7, 9 (10 is not on the step)
    // code
end loop

loop: item in collection
    // code
end loop
~~~

### Match

~~~poly
match value
    pattern1, expression1
    pattern2, expression2
    _, default_expression
end match
~~~

---

## Functions

### Basic Function

~~~poly
fn function_name(param: Type): ReturnType
    // code
    return value
end fn
~~~

### Function with Error Handling

~~~poly
fn risky_function(): Result<T, E>
    // code that might fail
    return Ok(value)
    // or
    return Error(error_value)
end fn
~~~

---

## Variables

### Declaration

~~~poly
var name Type := value        # Mutable variable
let name: Type := value        # Immutable variable
const NAME := value            # Constant
~~~

### Type Inference

~~~poly
var x := 42                    # Inferred as i32
var s := unicode "hello"              # Inferred as ustring
var b := true                  # Inferred as bool
~~~

---

## Data Types

### Primitives

~~~poly fragment
i32, i64, u32, u64           # Integers
f32, f64                     # Floats
bool                         # Boolean
char, uchar                  # Characters
ustring                      # Unicode strings
bytes                        # Byte arrays
~~~

### Collections

~~~poly fragment
Vec<T>                       # Dynamic array
Map<K, V>                    # Hash map
Set<T>                       # Hash set
~~~

### Compound Types

~~~poly
struct Name
    field: Type
end struct

enum Name
    Variant1
    Variant2(data: Type)
end enum
~~~

---

## Common Patterns

### Interactive Input

~~~poly
put -n "Enter your name: "
var name ustring := get --default unicode "Anonymous"
put "Hello, " + name + "!"
~~~

### Validated Input

~~~poly
var age i32 := get with validate |x| x > 0 && x < 150
~~~

### Timeout Handling

~~~poly
match get --timeout 3000
    Ok(input), process(input)
    Timeout, warn "Too slow!"
    Error(e), error "Error: " + e
end match
~~~

### File Operations

~~~poly
# Write to file
put "data" > "output.txt"
put "more" >> "output.txt"

# Read from file
var content ustring := get < "input.txt"
var data bytes := get < "binary.bin"
~~~

### Error Recovery

~~~poly fragment
var valid i32 := loop
    put -n "Enter a number: "
    match get
        Ok(input),
            var num i32 := input.parse::<i32>()
            if num > 0,
                break num
            else
                warn "Please enter a positive number"
            end if
        Error(e), error "Invalid input: " + e
    end match
end loop
~~~

---

## Async/Await

### Async Functions

~~~poly
async fn function_name(param: Type): ReturnType
    // async code
    var result := some_async_op().await
    return result
end fn
~~~

### Await Expressions

~~~poly
var result := async_function().await
var data := fetch(url).await
match async_op().await
    Ok(val), process(val)
    Error(e), handle_error(e)
end match
~~~

### Struct with Async Methods

~~~poly
struct Client
    var url: ustring
end struct

impl Client
    async fn fetch(self): Result<ustring, ustring>
        var response := http_get(self.url).await
        return Ok(response)
    end fn
end impl
~~~

### Traits with Async Methods

~~~poly
trait DataFetcher
    async fn fetch(self, key: ustring): Result<ustring, ustring>
end trait

impl DataFetcher for MyClient
    async fn fetch(self, key: ustring): Result<ustring, ustring>
        var data := self.query(key).await
        return Ok(data)
    end fn
end impl
~~~

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
| `async fn name()` | `async fn name()` |
| `expr.await` | `expr.await` |
| `trait T \n async fn m()` | `trait T { async fn m(); }` |
| `impl T for X \n async fn m()` | `impl T for X { async fn m() {} }` |
