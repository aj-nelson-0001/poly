# Poly Language Specification - Expanded Draft
## Version: 1.5 Draft (Work in Progress)

**Target Backend:** Rust (Cargo Workspace)  
**Status:** Active Definition - Major Expansion  
**Latest Addition:** Updated I/O syntax with flags (`put -n`, `get --timeout`, etc.) and separate error/warning commands

---

## Table of Contents

1. [Introduction & Core Philosophy](#1-introduction--core-philosophy)
2. [Variables, Constants & Assignment](#2-variables-constants--assignment)
3. [Arithmetic Operations](#3-arithmetic-operations)
4. [Data Types: Characters & Strings](#4-data-types-characters--strings)
5. [Data Types: Primitives & Compound Types](#5-data-types-primitives--compound-types)
6. [Structs & Custom Types](#6-structs--custom-types)
7. [Enums & Sum Types](#7-enums--sum-types)
8. [Traits & Interfaces](#8-traits--interfaces)
9. [Modules & Namespaces](#9-modules--namespaces)
10. [Ownership & Borrowing Model](#10-ownership--borrowing-model)
11. [Direct Memory Operations & Raw Pointers](#11-direct-memory-operations--raw-pointers)
12. [Functions & Error Handling](#12-functions--error-handling)
13. [Control Flow & Loops](#13-control-flow--loops)
14. [Closures & Functional Programming](#14-closures--functional-programming)
15. [Async/Await & Concurrency](#15-asyncawait--concurrency)
16. [Pattern Matching](#16-pattern-matching)
17. [Output: The `put` Command](#17-output-the-put-command)
18. [Input: The `get` Command](#18-input-the-get-command)
19. [Standard Library](#19-standard-library)
20. [Transpilation & Target Project Structure](#20-transpilation--target-project-structure)
21. [Implementation Roadmap](#21-implementation-roadmap)

---

## 1. Introduction & Core Philosophy

Poly is designed as a minimalist, low-overhead system programming language with a clean, highly explicit syntax reminiscent of assembly language. It provides fine-grained control over low-level memory and ASCII/Unicode data primitives while transpiling directly into safe, idiomatic Rust.

### Core Principles

1. **Explicit is Better than Implicit** - All operations are clear and predictable
2. **Assembly-Inspired Syntax** - Familiar to systems programmers
3. **Safe Transpilation** - Leverages Rust's memory safety guarantees
4. **Zero-Cost Abstractions** - High-level features compile to efficient Rust
5. **Gradual Complexity** - Simple features for beginners, powerful features for experts

---

## 2. Variables, Constants & Assignment

### Declaration Syntax

```poly
// Variable declaration (requires 'var' and explicit type)
var name: Type = value

// Reassignment ('var' is optional once declared)
name = new_value

// Constants (immutable, compile-time)
const NAME = value
```

### Mutability Rules

| Declaration | Mutable | Scope |
|-------------|---------|-------|
| `var x: i32 = 0` | Yes | Block |
| `const MAX = 100` | No | Module |
| `let x: i32 = 0` | No | Block |

### Examples

```poly
var count: i32 = 0
var status: bool = true
const MAX_BUFFER = 1024

// Reassignment
count = 5
count = count * 2
```

---

## 3. Arithmetic Operations

### Assembly-Style Instructions

| Poly Syntax | Default Step | Transpiled Rust Output |
|-------------|--------------|------------------------|
| `add counter` | 1 | `counter += 1;` |
| `add counter, 10` | Explicit (10) | `counter += 10;` |
| `sub score` | 1 | `score -= 1;` |
| `sub score, 15` | Explicit (15) | `score -= 15;` |
| `inc counter` | Alias for `add` | `counter += 1;` |
| `dec counter` | Alias for `sub` | `counter -= 1;` |

### Compound Assignment Operators

```poly
x += 5      // Addition assignment
x -= 3      // Subtraction assignment
x *= 2      // Multiplication assignment
x /= 4      // Division assignment
x %= 3      // Modulo assignment
x &= 0xFF   // Bitwise AND assignment
x |= 0x01   // Bitwise OR assignment
x ^= 0x10   // Bitwise XOR assignment
x <<= 2     // Left shift assignment
x >>= 1     // Right shift assignment
```

### Bitwise Operations

```poly
var a: i32 = 0b1010
var b: i32 = 0b1100

var result = a & b    // Bitwise AND: 0b1000
var result = a | b    // Bitwise OR: 0b1110
var result = a ^ b    // Bitwise XOR: 0b0110
var result = ~a       // Bitwise NOT
var result = a << 2   // Left shift
var result = a >> 1   // Right shift
```

---

## 4. Data Types: Characters & Strings

### Type Mapping

| Poly Type | Description | Memory Size | Rust Mapping |
|-----------|-------------|-------------|--------------|
| `char` | Single ASCII character | 1 byte (u8) | `u8` |
| `string` | Sequence of ASCII characters | 1 byte/char | `Vec<u8>` / `&[u8]` |
| `uchar` | Single UTF-32 Unicode scalar | 4 bytes | `char` |
| `ustring` | UTF-8 encoded Unicode text | 1-4 bytes/char | `String` / `&str` |
| `byte` | Raw byte value | 1 byte | `u8` |
| `bytes` | Byte buffer | Variable | `Vec<u8>` |

### String Operations

```poly
// ASCII string
var header: string = "HTTP/1.1 200 OK"
var len: i32 = header.len()        // Get length
var first: char = header[0]        // Index access
var sub: string = header[0:4]      // Slicing

// Unicode string
var greeting: ustring = u"Hello, 世界! 🚀"
var char_count: i32 = greeting.len()  // Character count
var byte_count: i32 = greeting.byte_len()  // Byte count
```

### String Manipulation

```poly
var s: ustring = u"hello"
s.append(u" world")           // Concatenation
s.to_upper()                  // In-place uppercase
s.to_lower()                  // In-place lowercase
s.trim()                      // Remove whitespace
s.contains(u"ell")            // Check substring
s.replace(u"l", u"r")         // Replace all occurrences
var parts: Vec<ustring> = s.split(u" ")  // Split by delimiter
```

---

## 5. Data Types: Primitives & Compound Types

### Primitive Types

| Poly Type | Description | Size | Rust Mapping |
|-----------|-------------|------|--------------|
| `bool` | Boolean | 1 byte | `bool` |
| `i8` / `u8` | Signed/Unsigned 8-bit integer | 1 byte | `i8` / `u8` |
| `i16` / `u16` | Signed/Unsigned 16-bit integer | 2 bytes | `i16` / `u16` |
| `i32` / `u32` | Signed/Unsigned 32-bit integer | 4 bytes | `i32` / `u32` |
| `i64` / `u64` | Signed/Unsigned 64-bit integer | 8 bytes | `i64` / `u64` |
| `i128` / `u128` | Signed/Unsigned 128-bit integer | 16 bytes | `i128` / `u128` |
| `f32` | 32-bit floating point | 4 bytes | `f32` |
| `f64` | 64-bit floating point | 8 bytes | `f64` |
| `isize` / `usize` | Platform-dependent integer | Pointer size | `isize` / `usize` |

### Compound Types

```poly
// Arrays (fixed-size)
var arr: [i32; 5] = [1, 2, 3, 4, 5]
var first: i32 = arr[0]

// Tuples
var point: (i32, i32) = (10, 20)
var (x, y) = point  // Destructuring

// Vectors (dynamic arrays)
var vec: Vec<i32> = [1, 2, 3]
vec.push(4)
var last: i32 = vec.pop()

// Option type (Rust-compatible)
var maybe: Option<i32> = Some(42)
var empty: Option<i32> = None

// Result type (for error handling)
var result: Result<i32, Error> = Ok(42)
var error: Result<i32, Error> = Error(Error.InvalidInput)
```

---

## 6. Structs & Custom Types

### Basic Struct Definition

```poly
struct Point
var x: f32
var y: f32
end struct

// Or without 'var' prefix
struct Point
x: f32
y: f32
end struct
```

### Struct with Methods

```poly
struct Point
var x: f32
var y: f32

// Methods
fn new(x: f32, y: f32): Point
    return Point { x: x, y: y }
end fn

fn distance_to(self, other: Point): f32
    var dx = self.x - other.x
    var dy = self.y - other.y
    return sqrt(dx * dx + dy * dy)
end fn

fn translate(self, dx: f32, dy: f32): Point
    return Point { x: self.x + dx, y: self.y + dy }
end fn

end struct
```

### Struct with Default Values

```poly
struct Config
var width: i32 = 800
var height: i32 = 600
var title: ustring = u"Window"
var fullscreen: bool = false
end struct

// Usage with defaults
var config: Config = Config { }
var custom: Config = Config { width: 1920, height: 1080 }
```

### Tuple Structs

```poly
struct Color(u8, u8, u8)  // RGB
struct Meters(f64)        // Newtype pattern

var red: Color = Color(255, 0, 0)
var distance: Meters = Meters(42.5)
```

### Unit Structs

```poly
struct Marker  // No fields - used for type-level programming
end struct
```

---

## 7. Enums & Sum Types

### Basic Enums

```poly
enum Direction
    North
    South
    East
    West
end enum
```

### Enums with Data (Algebraic Data Types)

```poly
enum Shape
    Circle(f32)                           // Tuple variant
    Rectangle { width: f32, height: f32 } // Struct variant
    Triangle { a: f32, b: f32, c: f32 }   // Named fields
end enum

// Usage
var circle: Shape = Shape::Circle(5.0)
var rect: Shape = Shape::Rectangle { width: 10.0, height: 20.0 }
```

### Enums with Methods

```poly
enum TrafficLight
    Red
    Yellow
    Green

fn duration(self): i32
    match self
        TrafficLight::Red => 60
        TrafficLight::Yellow => 5
        TrafficLight::Green => 45
    end match
end fn

fn next(self): TrafficLight
    match self
        TrafficLight::Red => TrafficLight::Green
        TrafficLight::Yellow => TrafficLight::Red
        TrafficLight::Green => TrafficLight::Yellow
    end match
end fn

end enum
```

### Option and Result Enums (Built-in)

```poly
// Built-in Option enum
enum Option<T>
    Some(T)
    None
end enum

// Built-in Result enum
enum Result<T, E>
    Ok(T)
    Error(E)
end enum
```

---

## 8. Traits & Interfaces

### Trait Definition

```poly
trait Drawable
    fn draw(self)
    fn bounding_box(self): Rect
end trait
```

### Traits with Default Implementations

```poly
trait Logger
    fn log(self, message: ustring)
    fn log_error(self, message: ustring)
    
    // Default implementation
    fn log_warning(self, message: ustring)
        self.log(u"[WARNING] " + message)
    end fn
end trait
```

### Implementing Traits

```poly
struct Circle
    var x: f32
    var y: f32
    var radius: f32
end struct

impl Drawable for Circle
    fn draw(self)
        // Draw circle implementation
        draw_circle(self.x, self.y, self.radius)
    end fn

    fn bounding_box(self): Rect
        return Rect {
            x: self.x - self.radius,
            y: self.y - self.radius,
            width: self.radius * 2,
            height: self.radius * 2
        }
    end fn
end impl
```

### Trait Bounds (Generics)

```poly
// Function with trait bound
fn draw_all<T: Drawable>(items: Vec<T>)
    loop: items
        item.draw()
    end loop
end fn

// Multiple trait bounds
fn process<T: Debug + Clone>(item: T): T
    debug(item)
    return item.clone()
end fn

// Where clause
fn complex_function<T, U>(t: T, u: U): bool
where
    T: Display + PartialEq,
    U: Debug + Clone
    // Implementation
end fn
```

### Trait Objects (Dynamic Dispatch)

```poly
// Trait object with dynamic dispatch
var shapes: Vec<Box<dyn Drawable>> = []
shapes.push(Box::new(Circle::new(0.0, 0.0, 5.0)))
shapes.push(Box::new(Rectangle::new(0.0, 0.0, 10.0, 20.0)))

loop: shapes
    shape.draw()
end loop
```

---

## 9. Modules & Namespaces

### Module Definition

```poly
// math.mod (file: math.mod)
module math

pub const PI = 3.141592653589793

pub fn add(a: f64, b: f64): f64
    return a + b
end fn

pub fn multiply(a: f64, b: f64): f64
    return a * b
end fn

// Nested module
pub module advanced
    pub fn power(base: f64, exp: i32): f64
        var result = 1.0
        loop: 0..exp
            result = result * base
        end loop
        return result
    end fn
end module

end module
```

### Using Modules

```poly
// Import entire module
use math

var result = math::add(1.0, 2.0)
var pi = math::PI

// Import specific items
use math::add
use math::PI

var result = add(1.0, 2.0)

// Import with alias
use math::advanced::power as pow

var result = pow(2.0, 10)

// Import all from module
use math::*

var result = add(1.0, 2.0)
var result = multiply(3.0, 4.0)
```

### Module Visibility

```poly
module mymodule
    pub fn public_function() { }     // Public
    fn private_function() { }        // Private
    pub const PUBLIC_CONST = 1       // Public
    const PRIVATE_CONST = 2          // Private
    
    pub struct PublicStruct { }      // Public struct
    struct PrivateStruct { }         // Private struct
end module
```

---

## 10. Ownership & Borrowing Model

### Core Principles

Poly implements a simplified ownership system that transpiles to Rust's ownership model:

1. **Each value has exactly one owner**
2. **When the owner goes out of scope, the value is dropped**
3. **Ownership can be transferred (moved) or borrowed**

### Ownership Transfer (Move Semantics)

```poly
var s1: ustring = u"hello"
var s2 = s1      // s1 is MOVED to s2
// s1 is no longer valid here

var s3 = s2.clone()  // Explicit clone - s2 remains valid
```

### Borrowing (References)

```poly
// Immutable borrow (shared reference)
fn print_string(s: &ustring)
    print(s)
end fn

var name: ustring = u"Alice"
print_string(&name)  // Borrow name
print_string(&name)  // Can borrow again

// Mutable borrow (exclusive reference)
fn append_greeting(s: &mut ustring)
    s.append(u", World!")
end fn

var greeting: ustring = u"Hello"
append_greeting(&mut greeting)  // Mutable borrow
// Only one mutable borrow at a time
```

### Lifetimes (Explicit)

```poly
// Lifetime annotation
fn longest<'a>(x: &'a ustring, y: &'a ustring): &'a ustring
    if x.len() > y.len()
        return x
    else
        return y
    end if
end fn

// Struct with lifetime
struct TextBuffer<'a>
    var data: &'a [u8]
    var position: usize
end struct

impl<'a> TextBuffer<'a>
    fn new(data: &'a [u8]): TextBuffer<'a>
        return TextBuffer { data: data, position: 0 }
    end fn
end impl
```

### Smart Pointers

```poly
// Box - heap allocation
var boxed: Box<i32> = Box::new(42)
var value: i32 = *boxed  // Dereference

// Rc - reference counting (shared ownership)
var shared: Rc<ustring> = Rc::new(u"shared data")
var clone1 = Rc::clone(&shared)
var clone2 = Rc::clone(&shared)

// Arc - atomic reference counting (thread-safe)
var thread_safe: Arc<Mutex<Vec<i32>>> = Arc::new(Mutex::new([]))
```

### Transpilation Rules

| Poly Syntax | Rust Output |
|-------------|-------------|
| `var s = s1` | `let s = s1;` (move) |
| `var s = s1.clone()` | `let s = s1.clone();` |
| `fn foo(x: &i32)` | `fn foo(x: &i32)` |
| `fn foo(x: &mut i32)` | `fn foo(x: &mut i32)` |
| `&value` | `&value` |
| `&mut value` | `&mut value` |
| `Box::new(val)` | `Box::new(val)` |
| `Rc::new(val)` | `Rc::new(val)` |

---

## 11. Direct Memory Operations & Raw Pointers

### Raw Pointer Syntax

```poly
// Raw pointer declaration
var ptr: ptr i32 = null

// Taking address (unsafe)
var value: i32 = 42
var ptr: ptr i32 = addr value

// Dereferencing (unsafe)
var dereferenced: i32 = deref ptr

// Pointer arithmetic
var ptr2: ptr i32 = ptr + 1  // Next i32
var ptr3: ptr i32 = ptr - 1  // Previous i32
```

### Unsafe Blocks

```poly
// Explicit unsafe block
unsafe
    var value: i32 = deref ptr
    ptr = addr some_var
end unsafe

// Function with unsafe operations
fn readHardwareRegister(address: ptr u32): u32
    return deref address
end fn

fn writeHardwareRegister(address: ptr u32, value: u32)
    address = value
end fn
```

### Null Pointer Safety

```poly
// Nullable pointers (safe alternative to raw pointers)
var maybe_ptr: ?ptr i32 = null

if maybe_ptr != null,
    var value: i32 = deref maybe_ptr
end if

// Or with pattern matching
match maybe_ptr
    Some(ptr) => var value = deref ptr
    None => print(u"Null pointer")
end match
```

---

## 12. Functions & Error Handling

### Function Syntax

```poly
// Basic function
fn add(a: i32, b: i32): i32
    return a + b
end fn

// Function with no return
fn print_hello()
    print(u"Hello!")
end fn

// Function with default parameters
fn greet(name: ustring, greeting: ustring = u"Hello"): ustring
    return greeting + u", " + name + u"!"
end fn
```

### Error Handling

#### Key Concepts

1. **Result type**: Use `Result<T, E>` for operations that can fail. `Ok(T)` for success, `Error(E)` for failure.
2. **Error propagation**: Use `try` to propagate errors up the call stack (like Rust's `?` operator).
3. **Pattern matching**: Use `match` to handle both success and error cases.
4. **Custom errors**: Define custom error types using `enum` for specific error conditions.
5. **Readability**: Poly uses `Error(e)` instead of `Err(e)` for clearer, more readable code.

#### Basic Error Handling

```poly
// Result type for error handling
enum FileError
    NotFound
    PermissionDenied
    InvalidData
end enum

fn read_file(path: ustring): Result<ustring, FileError>
    // 'try' handles error propagation (like Rust's ?)
    var file = try open_file(path)
    
    if file.is_valid
        var content = try file.read_all()
        return Ok(content)
    else
        return Error(FileError::NotFound)
    end if
end fn

// Using functions with error handling
fn process_config()
    match read_file(u"config.txt")
        Ok(content) => parse_config(content)
        Error(e) => 
            print(u"Error: " + e.to_string())
            exit(1)
        end
    end match
end fn
```

#### Advanced Error Handling Patterns

```poly
// Custom error types with messages
enum ValidationError
    EmptyInput
    TooShort(min: i32)
    TooLong(max: i32)
    InvalidFormat
end enum

// Function that returns custom errors
fn validate_name(name: ustring): Result<ustring, ValidationError>
    if name.len() == 0,
        return Error(ValidationError::EmptyInput)
    end if
    
    if name.len() < 2,
        return Error(ValidationError::TooShort(2))
    end if
    
    if name.len() > 50,
        return Error(ValidationError::TooLong(50))
    end if
    
    return Ok(name)
end fn

// Handling errors with match
fn process_name()
    match validate_name(u"John")
        Ok(valid_name) => put "Valid name: " + valid_name
        Error(EmptyInput) => error "Name cannot be empty"
        Error(TooShort(min)) => error "Name too short, minimum " + min.to_string() + " characters"
        Error(TooLong(max)) => error "Name too long, maximum " + max.to_string() + " characters"
        Error(InvalidFormat) => error "Invalid name format"
    end match
end fn

// Using wildcard pattern for catch-all
fn handle_any_error()
    match validate_name(input)
        Ok(name) => put "Valid: " + name
        Error(_) => error "Validation failed"  // Catches any validation error
    end match
end fn

// Error propagation with try
fn process_user()
    var name = try validate_name(input)  // Propagates error if validation fails
    var email = try validate_email(input)  // Propagates error if validation fails
    // ... process valid data
end fn
```

### Panic and Unwrap

```poly
// Panic (immediate termination)
fn critical_error()
    panic(u"Critical system failure!")
end fn

// Unwrap (panics on None/Err)
var value: i32 = optional_value.unwrap()  // Panics if None

// Expect (panics with custom message)
var value: i32 = optional_value.expect(u"Value must exist")

// Safe alternatives
match optional_value
    Some(v) => use(v)
    None => handle_missing()
end match
```

### Transpilation Rules

| Poly Syntax | Rust Output |
|-------------|-------------|
| `try expr` | `expr?` |
| `panic(msg)` | `panic!("{}", msg)` |
| `val.unwrap()` | `val.unwrap()` |
| `val.expect(msg)` | `val.expect(&msg)` |

---

## 13. Control Flow & Loops

### If/Else Expressions

```poly
// Basic if/else
if condition,
    do_something()
else,
    do_other()
end if

// If as expression
var result = if x > 0, x else -x end if

// Chained conditions
if score >= 90,
    grade = u"A"
else if score >= 80,
    grade = u"B"
else if score >= 70,
    grade = u"C"
else,
    grade = u"F"
end if
```

### While Loops

```poly
// Basic while loop
var i: i32 = 0
while i < 10
    print(i)
    add i
end while

// While with break/continue
var sum: i32 = 0
var i: i32 = 1
while true
    if i > 100
        break
    end if
    if i % 2 == 0
        add i
        continue
    end if
    sum = sum + i
    add i
end while
```

### Loop (Infinite Loop)

```poly
// Infinite loop
loop
    var input = read_input()
    if input == u"quit"
        break
    end if
    process(input)
end loop
```

### Loop Ranges (Inspired by Sinclair QL SuperBASIC)

Poly's `loop` command with colon syntax supports multiple ranges and specific values, borrowing from Sinclair QL SuperBASIC's flexible `FOR` loop design. Use `loop:` (with colon) for range/value iteration, while `loop` (without colon) remains the infinite loop.

#### Syntax Variants

```poly
// Simple range
loop: 0..10
    print(i)
end loop

// Inclusive range
loop: 0..=10
    print(i)
end loop

// Multiple ranges and specific values (SuperBASIC style)
loop: 1..3, 7, 19..20
    print(i)  // Iterates: 1, 2, 3, 7, 19, 20
end loop

// With step (positive)
loop: 1..10 step 2
    print(i)  // Iterates: 1, 3, 5, 7, 9
end loop

// With step (negative, counting down)
loop: 10..1 step -1
    print(i)  // Iterates: 10, 9, 8, ..., 1
end loop

// Mixed ranges with step
loop: 0..10 step 2, 20..30 step 3
    print(i)  // Iterates: 0, 2, 4, 6, 8, 20, 23, 26, 29
end loop

// Specific values only
loop: 1, 5, 10, 100
    print(i)  // Iterates: 1, 5, 10, 100
end loop

// Complex mix: ranges, values, and steps
loop: 1..5, 10, 20..25 step 2, 100
    print(i)  // Iterates: 1, 2, 3, 4, 5, 10, 20, 22, 24, 100
end loop
```

#### Iterate Over Collections

```poly
// Iterate over collection
var items: Vec<i32> = [1, 2, 3, 4, 5]
loop: items
    print(item)
end loop

// Iterate with index
loop: (index, item) in items.enumerate()
    print(index, item)
end loop

// Iterate over string characters
var text: ustring = u"Hello"
loop: text.chars()
    print(ch)
end loop

// Iterate over bytes
var data: string = "binary"
loop: data.bytes()
    print(byte)
end loop

// Iterate over map entries
var map: Map<ustring, i32> = [u"one": 1, u"two": 2]
loop: map
    print(key, value)
end loop
```

#### Reverse Ranges

```poly
// Reverse using negative step
loop: 10..1 step -1
    print(i)  // Iterates: 10, 9, 8, ..., 1
end loop

// Reverse using .rev()
loop: (0..10).rev()
    print(i)  // Iterates: 9, 8, 7, ..., 0
end loop
```

#### Transpilation Examples

| Poly Syntax | Rust Output |
|-------------|-------------|
| `loop: 0..10` | `for i in 0..10 {` |
| `loop: 0..=10` | `for i in 0..=10 {` |
| `loop: 1..3, 7, 19..20` | `for i in (1..3).chain(std::iter::once(7)).chain(19..20) {` |
| `loop: 1..10 step 2` | `for i in (1..10).step_by(2) {` |
| `loop: 10..1 step -1` | `for i in (1..10).rev() {` |
| `loop: items` | `for item in items {` |
| `loop: items.enumerate()` | `for (index, item) in items.into_iter().enumerate() {` |

### Match Expressions

```poly
// Basic match
match command
    u"start" => start_process()
    u"stop" => stop_process()
    u"pause" => pause_process()
    _ => unknown_command()  // Wildcard/default
end match

// Match with variables
match message
    u"quit" => exit(0)
    u"help" => show_help()
    other => print(u"Unknown: " + other)
end match

// Match on enum
match shape
    Shape::Circle(r) => 
        var area = PI * r * r
        print(area)
    Shape::Rectangle { width, height } => 
        var area = width * height
        print(area)
    _ => print(u"Unknown shape")
end match

// Match with guards
match number
    n if n < 0 => print(u"Negative")
    n if n == 0 => print(u"Zero")
    n if n > 0 => print(u"Positive")
end match

// Exhaustive match
match option_value
    Some(v) => process(v)
    None => handle_none()
    // No wildcard needed - all cases covered
end match
```

### Transpilation Rules

| Poly Syntax | Rust Output |
|-------------|-------------|
| `if x,` | `if x {` |
| `else if x,` | `} else if x {` |
| `else,` | `} else {` |
| `end if` | `}` |
| `while cond` | `while cond {` |
| `end while` | `}` |
| `loop` | `loop {` |
| `end loop` | `}` |
| `loop: 0..10` | `for i in 0..10 {` |
| `loop: items` | `for item in items {` |
| `end loop` | `}` |
| `match expr` | `match expr {` |
| `pattern => expr` | `pattern => expr,` |
| `end match` | `}` |

---

## 14. Closures & Functional Programming

### Closure Syntax

```poly
// Basic closure
var add = |a: i32, b: i32| -> i32
    return a + b
end

// Closure with single expression
var square = |x: i32| x * x

// Closure capturing environment
var multiplier: i32 = 5
var multiply = |x: i32| x * multiplier

// Mutable capture
var counter: i32 = 0
var increment = || 
    counter = counter + 1
end
```

### Closure Types

```poly
// Immutable borrow (default)
var print_value = |x: i32|
    print(x)
end

// Mutable borrow
var accumulate = |total: &mut i32, value: i32|
    *total = *total + value
end

// Move closure (takes ownership)
var data: Vec<i32> = [1, 2, 3]
var process = move ||
    loop: data
        print(item)
    end loop
end
// data is no longer accessible
```

### Higher-Order Functions

```poly
// Map
var doubled = [1, 2, 3].map(|x| x * 2)

// Filter
var evens = [1, 2, 3, 4, 5].filter(|x| x % 2 == 0)

// Reduce/Fold
var sum = [1, 2, 3, 4, 5].reduce(0, |acc, x| acc + x)

// Find
var first_even = [1, 3, 4, 5].find(|x| x % 2 == 0)

// Any/All
var has_negative = [-1, 2, 3].any(|x| x < 0)
var all_positive = [1, 2, 3].all(|x| x > 0)

// Sort with custom comparator
var sorted = [3, 1, 4, 1, 5].sort_by(|a, b| a.compare(b))

// Chained operations
var result = [1, 2, 3, 4, 5]
    .filter(|x| x % 2 == 0)
    .map(|x| x * x)
    .reduce(0, |acc, x| acc + x)
```

### Function Pointers

```poly
// Function type
type MathOp = fn(i32, i32) -> i32

var add: MathOp = |a, b| a + b
var subtract: MathOp = |a, b| a - b

// Passing functions as arguments
fn apply_operation(a: i32, b: i32, op: MathOp): i32
    return op(a, b)
end fn

var result = apply_operation(5, 3, add)
```

### Transpilation Rules

| Poly Syntax | Rust Output |
|-------------|-------------|
| `\|x\| x * 2` | `\|x\| x * 2` |
| `\|x: i32\| x * 2` | `\|x: i32\| x * 2` |
| `\|\| body end` | `\|\| { body }` |
| `move \|\| body end` | `move \|\| { body }` |
| `vec.map(\|x\| expr)` | `vec.iter().map(\|x\| expr).collect()` |
| `vec.filter(\|x\| expr)` | `vec.iter().filter(\|x\| expr).collect()` |

---

## 15. Async/Await & Concurrency

### Overview

Poly supports asynchronous programming through `async fn` and `.await` syntax, which transpiles to Rust's async/await system. This enables non-blocking I/O and concurrent operations while maintaining the language's explicit, readable style.

### Key Concepts

1. **Async functions**: Use `async fn` to declare functions that can be paused and resumed
2. **Await expressions**: Use `.await` postfix syntax to wait for async operations
3. **Tokio runtime**: Async code runs on the Tokio runtime (automatically added `#[tokio::main]`)
4. **Trait support**: Traits can have async methods

### Async Function Declaration

```poly
// Basic async function
async fn fetch_data(url: ustring): ustring
    var response = http_get(url).await
    return response
end fn

// Async function with Result return type
async fn fetch_json(url: ustring): Result<ustring, ustring>
    match http_get(url).await
        Ok(response) => return Ok(response)
        Error(e) => return Error(u"Network error: " + e)
    end match
end fn

// Async function with no return value
async fn log_message(message: ustring)
    put u"Logging: " + message
    // Simulate async I/O
    timer_sleep(100).await
end fn
```

### Await Expressions

The `.await` syntax is postfix, meaning it comes after the async expression:

```poly
// Wait for async function result
var data = fetch_data(u"https://api.example.com").await

// Chain async calls
var result = process_data(
    fetch_data(u"https://api.example.com").await
).await

// Await in variable assignment
var user = db.get_user(user_id).await
var posts = db.get_posts(user.id).await

// Await in match expression
match http_get(url).await
    Ok(response) => process(response)
    Error(e) => handle_error(e)
end match

// Await in function arguments
var data = transform(
    fetch_data(url).await,
    validate(input).await
)
```

### Async with Structs and Enums

```poly
// Struct with async methods
struct HttpClient
    var base_url: ustring
    var timeout: i32
end struct

impl HttpClient
    async fn get(self, path: ustring): Result<ustring, ustring>
        var url = self.base_url + path
        var response = http_get(url).await
        return Ok(response)
    end fn
    
    async fn post(self, path: ustring, body: ustring): Result<ustring, ustring>
        var url = self.base_url + path
        var response = http_post(url, body).await
        return Ok(response)
    end fn
end impl

// Usage
var client = HttpClient { base_url: u"https://api.example.com", timeout: 5000 }
var data = client.get(u"/users").await
```

### Traits with Async Methods

```poly
// Trait with async method
trait DataFetcher
    async fn fetch(self, key: ustring): Result<ustring, ustring>
end trait

// Implementation
impl DataFetcher for Database
    async fn fetch(self, key: ustring): Result<ustring, ustring>
        var result = self.query(key).await
        return Ok(result)
    end fn
end impl

// Function with trait bound
async fn process_fetcher<T: DataFetcher>(fetcher: T, key: ustring): ustring
    match fetcher.fetch(key).await
        Ok(data) => return data
        Error(e) => 
            error e
            return u""
    end match
end fn
```

### Concurrency Patterns

```poly
// Spawn concurrent tasks
async fn main()
    // Run multiple async operations concurrently
    var task1 = fetch_data(u"url1")
    var task2 = fetch_data(u"url2")
    var task3 = fetch_data(u"url3")
    
    // Await all results
    var result1 = task1.await
    var result2 = task2.await
    var result3 = task3.await
    
    put u"All data fetched!"
end fn

// Async iteration
async fn process_items(items: Vec<ustring>): Vec<ustring>
    var results: Vec<ustring> = []
    loop: items
        var result = process_item(item).await
        results.push(result)
    end loop
    return results
end fn

// Error handling in async code
async fn safe_fetch(url: ustring): Option<ustring>
    match fetch_with_timeout(url, 5000).await
        Ok(data) => return Some(data)
        Error(TimeoutError) => 
            warn u"Request timed out"
            return None
        Error(e) => 
            error e
            return None
    end match
end fn
```

### Transpilation Rules

| Poly Syntax | Rust Output |
|-------------|-------------|
| `async fn name()` | `async fn name()` |
| `expr.await` | `expr.await` |
| `trait T \n async fn m() \n end trait` | `trait T { async fn m(); }` |
| `impl T for X \n async fn m() \n end impl` | `impl T for X { async fn m() {} }` |
| (auto-detected) | `#[tokio::main]` on main |

### Example: Complete Async Program

```poly
# HTTP client example

struct ApiClient
    var base_url: ustring
end struct

impl ApiClient
    async fn get_users(self): Result<Vec<ustring>, ustring>
        var response = http_get(self.base_url + u"/users").await
        match response
            Ok(data) => return Ok(parse_json(data))
            Error(e) => return Error(e)
        end match
    end fn
    
    async fn create_user(self, name: ustring): Result<ustring, ustring>
        var body = u"{\"name\": \"" + name + u"\"}"
        var response = http_post(self.base_url + u"/users", body).await
        return response
    end fn
end impl

# Main async function
async fn main_task()
    var client = ApiClient { base_url: u"https://api.example.com" }
    
    # Fetch users
    match client.get_users().await
        Ok(users) => 
            put u"Found " + users.len().to_string() + u" users"
            loop: users
                put user
            end loop
        Error(e) => error e
    end match
    
    # Create new user
    var result = client.create_user(u"Alice").await
    put u"User created: " + result
end fn

# Entry point (auto-generates #[tokio::main])
fn main()
    main_task().await
end fn
```

---

## 16. Pattern Matching

### Basic Patterns

```poly
// Literal matching
match x
    0 => print(u"zero")
    1 => print(u"one")
    2 => print(u"two")
    _ => print(u"other")
end match

// String matching
match command
    u"start" => start()
    u"stop" => stop()
    _ => unknown()
end match
```

### Variable Binding

```poly
// Bind matched value to variable
match message
    u"quit" => exit(0)
    cmd => print(u"Unknown command: " + cmd)
end match

// Ignore value
match data
    _ => process_any()
end match
```

### Destructuring

```poly
// Tuple destructuring
match point
    (0, 0) => print(u"origin")
    (x, 0) => print(u"on x-axis at " + x)
    (0, y) => print(u"on y-axis at " + y)
    (x, y) => print(u"at " + x + u", " + y)
end match

// Struct destructuring
match person
    Person { name: u"Alice", age } => print(u"Alice is " + age)
    Person { name, age: n if n > 60 } => print(name + u" is a senior")
    Person { name, age } => print(name + u" is " + age)
end match

// Enum destructuring
match shape
    Shape::Circle(r) => 
        print(u"Circle with radius " + r)
    Shape::Rectangle { width, height } => 
        print(u"Rectangle " + width + u"x" + height)
    _ => print(u"Unknown shape")
end match
```

### Nested Patterns

```poly
// Nested matching
match data
    Some((x, y)) if x > 0 && y > 0 => print(u"Positive point")
    Some((x, _)) if x < 0 => print(u"Negative x")
    Some(_) => print(u"Other point")
    None => print(u"No point")
end match
```

### Match Guards

```poly
match number
    n if n < 0 => print(u"Negative")
    n if n == 0 => print(u"Zero")
    n if n > 0 && n < 100 => print(u"Small positive")
    n if n >= 100 => print(u"Large positive")
end match
```

### Binding Modes

```poly
// @ binding (bind while matching)
match age
    n @ 0..12 => print(u"Child: " + n)
    n @ 13..17 => print(u"Teenager: " + n)
    n @ 18..64 => print(u"Adult: " + n)
    n @ 65.. => print(u"Senior: " + n)
end match
```

---

## 16. Output: The `put` Command

### Overview

The `put` command is Poly's primary output mechanism, providing a simple syntax for printing to stdout or writing to files. It uses shell-like redirection operators for file output.

### Key Concepts

1. **Newline behavior**: By default, `put` adds a newline (\n) after the output. Use `-n` flag to suppress the newline.
2. **Unicode inference**: The language automatically detects Unicode based on string content. No need for separate `uput` command.
3. **Error/Warning commands**: Use `error`, `warn`, and `info` for stderr output. These commands automatically prepend level indicators.
4. **File output**: Use `>` for write/truncate and `>>` for append operations.

### Syntax Variants

```poly
// Output to stdout (with newline, default)
// A newline is a line feed (\n), not a carriage return (\r)
put expression

// Output without newline (use -n flag)
put -n expression

// Output to file (write/truncate)
put expression > "filename.txt"

// Output to file (append)
put expression >> "filename.txt"

// Error messages (to stderr)
error expression

// Warnings (to stderr)
warn expression

// Debug/diagnostic info (to stderr)
info expression
```

### Basic Examples

```poly
// Simple string output
put "Hello, World!"

// Variable output
var name: ustring = u"Alice"
var age: i32 = 30
put "Name: " + name + ", Age: " + age

// Numeric output
var pi: f64 = 3.14159
put pi

// Boolean output
var active: bool = true
put active

// Multiple values (concatenated)
var x: i32 = 10
var y: i32 = 20
put "x = " + x + ", y = " + y + ", sum = " + (x + y)

// Output without newline (progress indicator)
put -n "Loading..."
put -n "."
put -n "."
put "."  // This one adds a newline

// Error/warning/debug output
error "Error: File not found"
warn "Warning: Deprecated function"
info "DEBUG: Request took 42ms"

// Interactive prompts
put -n "Enter your name: "
var user_name: ustring = get
put "Hello, " + user_name + "!"

// Menu display
put "Menu:"
put "1. Start"
put "2. Stop"
put "3. Exit"
put -n "Choose: "
var choice: i32 = get
```

### Formatting Output

```poly
// Format string with placeholders
var name: ustring = u"World"
var count: i32 = 42
put u"Hello, {name}! Count: {count}"

// Format with expressions
put u"Result: {10 * 5 + 3}"
put u"Pi rounded: {3.14159:.2f}"

// Format with width and alignment
put u"{'left':<10}{'right':>10}{'center':^10}"
// Output: left      right     center   

// Format with padding
put u"{'5':0>5}"    // 00005
put u"{'hello':.10}" // hello*****
```

### File Output

```poly
// Write to file (creates or overwrites)
put u"Line 1\nLine 2\nLine 3" > "output.txt"

// Append to file
put u"Appended line" >> "log.txt"

// Append with newline
put u"New log entry" >> "app.log"

// Write without newline (useful for progress indicators)
put -n u"Writing... " > "output.txt"
put u"done!" >> "output.txt"  // This adds a newline

// Write binary data
var data: bytes = [0x48, 0x65, 0x6C, 0x6C, 0x6F]
data > "binary.bin"

// Write formatted data to file
var records: Vec<ustring> = [u"Alice,30", u"Bob,25"]
loop: records
    put record >> "contacts.csv"
end loop
```

### Error/Warning Output

```poly
// Error messages (to stderr)
error "Error: File not found"
error "Error: Cannot divide by zero"

// Warnings (to stderr)
warn "Warning: Deprecated function"
warn "Warning: Memory usage high"

// Debug/diagnostic info (to stderr)
info "DEBUG: Request took 42ms"
info "DEBUG: Memory usage: 128MB"

// Error with formatting
var error_code: i32 = 404
error "HTTP Error {error_code}: Resource not found"

// Warning with conditional check
if is_deprecated,
    warn "Warning: This function is deprecated"
end if
```

### Advanced Output Features

```poly
// Output with explicit type conversion
var value: f64 = 42.5
put value.to_string()
put value as i32  // Truncate to integer

// Conditional output
if verbose,
    put u"Processing item: " + item.name
end if

// Output in loop
loop: 0..10
    put u"Step {i + 1} of 10"
end loop

// Capture output to variable
var output: ustring = capture put u"Computed: " + (2 + 2)
// output now contains u"Computed: 4"
```

### Transpilation Rules

| Poly Syntax | Rust Output |
|-------------|-------------|
| `put expr` | `println!("{}", expr);` |
| `put -n expr` | `print!("{}", expr);` |
| `error expr` | `eprintln!("[ERROR] {}", expr);` |
| `warn expr` | `eprintln!("[WARN] {}", expr);` |
| `info expr` | `eprintln!("[INFO] {}", expr);` |
| `put expr > "file"` | `std::fs::write("file", expr.to_string()).unwrap();` |
| `put expr >> "file"` | `use std::io::Write; let mut f = std::fs::OpenOptions::new().append(true).open("file").unwrap(); writeln!(f, "{}", expr).unwrap();` |

**Note:** The `error`, `warn`, and `info` commands automatically prepend level indicators (`[ERROR]`, `[WARN]`, `[INFO]`) to the output. This helps with log filtering and debugging.


| Feature | Poly | Rust | Python | Go |
|---------|------|------|--------|----|
| Basic output | `put x` | `println!("{}", x)` | `print(x)` | `fmt.Println(x)` |
| No newline | `put -n x` | `print!("{}", x)` | `print(x, end="")` | `fmt.Print(x)` |
| Error | `error x` | `eprintln!("[ERROR] {}", x)` | `print(x, file=sys.stderr)` | `fmt.Fprintln(os.Stderr, x)` |
| Warning | `warn x` | `eprintln!("[WARN] {}", x)` | `print(x, file=sys.stderr)` | `fmt.Fprintln(os.Stderr, x)` |
| Debug | `info x` | `eprintln!("[INFO] {}", x)` | `print(x, file=sys.stderr)` | `fmt.Fprintln(os.Stderr, x)` |
| File write | `put x > "f"` | `fs::write("f", x)` | `open("f","w").write(x)` | `os.WriteFile("f", x)` |
| File append | `put x >> "f"` | `OpenOptions::append` | `open("f","a").write(x)` | `os.OpenFile("f", APPEND)` |
| Formatting | `put "{x:.2f}"` | `println!("{:.2f}", x)` | `print(f"{x:.2f}")` | `fmt.Printf("%.2f", x)` |

### Advanced Examples

**Note:** These examples use standard library functions like `sleep()`, `pad_right()`, `repeat()`, and `to_string()`. See the Standard Library section for details.

```poly
// Complex output patterns
fn display_progress(current: i32, total: i32)
    var percent: f64 = (current as f64) / (total as f64) * 100.0
    put -n "\rProgress: "
    put -n percent.to_string() + "% "
    
    // Build progress bar
    var bar_width: i32 = 30
    var filled: i32 = (percent / 100.0 * bar_width as f64) as i32
    var empty: i32 = bar_width - filled
    
    put -n "["
    loop: 0..filled
        put -n "#"
    end loop
    loop: 0..empty
        put -n "-"
    end loop
    put "]"
end fn

// Structured logging
fn log_request(method: ustring, path: ustring, status: i32, duration_ms: i32)
    var level: ustring = if status >= 500,u"ERROR"
        else if status >= 400,u"WARN"
        else u"INFO"
    end if
    
    var message: ustring = "{method} {path} -> {status} ({duration_ms}ms)"
    
    match level
        u"ERROR" => error message
        u"WARN" => warn message
        u"INFO" => info message
    end match
end fn

// Dynamic table output
fn display_table(headers: Vec<ustring>, rows: Vec<Vec<ustring>>)
    // Print headers
    loop: headers
        put -n header.pad_right(15)
    end loop
    put ""
    
    // Print separator
    loop: 0..headers.len()
        put -n "-".repeat(15)
    end loop
    put ""
    
    // Print rows
    loop: rows
        loop: row
            put -n cell.pad_right(15)
        end loop
        put ""
    end loop
end fn

// Error reporting with context
fn report_error(context: ustring, error: ustring, suggestion: ustring)
    error "Error in " + context + ": " + error
    warn "Suggestion: " + suggestion
    info "Stack trace available with --verbose flag"
end fn
```

### Edge Cases

```poly
// Empty string output
put ""  // Outputs empty line

// Empty file read
var content: ustring = get < "empty.txt"  // Returns empty string

// Very large numbers
var big: i64 = 9223372036854775807  // Max i64
put big

// Unicode characters
put "日本語"  // Japanese characters
put "🌍"     // Emoji
put "café"   // Accented characters

// Special characters
put "Line1\nLine2\tTab"  // Escape characters
put "Quote: \"Hello\""   // Quotes

// Null bytes in binary
var data: bytes = [0x00, 0xFF, 0x00]
data > "null_bytes.bin"

// Timeout edge cases
match get --timeout 0  // Immediate timeout
    Ok(input) => put input
    Timeout => warn "Immediate timeout"
    Error(e) => error e
end match

// Validation edge cases
var age: i32 = get with validate |x| x > 0  // Rejects 0 and negative
var name: ustring = get with validate |n| n.len() >= 1  // Rejects empty

// File operations edge cases
put "" > "empty.txt"  // Create empty file
put "data" > ""  // Error: empty filename
var content: ustring = get < "nonexistent.txt"  // Error: file not found

// --bytes flag edge cases
put "Hello, World!" > "bytes_test.txt"
var zero_bytes: bytes = get < "bytes_test.txt" --bytes 0  // Empty read
var exact_bytes: bytes = get < "bytes_test.txt" --bytes 5  // Exactly 5 bytes
var too_many: bytes = get < "bytes_test.txt" --bytes 1000  // More than file size

// Error handling edge cases
fn risky(): Result<ustring, ustring>
    return Error(u"")  // Empty error message
end fn

match risky()
    Ok(_) => put "Success"
    Error("") => warn "Empty error"
    Error(e) => error e
end match
```

### Accessibility

```poly
// Screen reader friendly output
put "Enter your name: "  // Clear, descriptive prompt
var name: ustring = get

// Provide text alternatives for visual elements
put "Progress: 50% complete"  // Text alternative to progress bar
put -n "[" 
put -n "#".repeat(5)  // Visual progress bar
put -n "-".repeat(5)
put "]"

// Use consistent naming conventions
put "Enter your email address: "  // Clear, consistent
var email: ustring = get

// Provide clear error messages
put -n "Enter your age: "
var age: i32 = get with validate |x| x > 0 && x < 150
// Error message: "Please enter a valid age between 1 and 150"

// Use semantic output
put "Error: File not found"  // Clear error level
put "Warning: Deprecated feature"  // Clear warning level
put "Info: Processing complete"  // Clear info level

// Provide context for output
put "Processing item 5 of 10"  // Clear progress context
put "Step 1: Reading file"  // Clear step context

// Use appropriate colors (when available)
// Red for errors, yellow for warnings, green for success
error "File not found"  // Would display in red
warn "Deprecated feature"  // Would display in yellow
put "Success!"  // Would display in green

// Provide text alternatives for colors
error "[ERROR] File not found"  // Text prefix for color-blind users
warn "[WARN] Deprecated feature"  // Text prefix for color-blind users
put "[SUCCESS] Operation complete"  // Text prefix for color-blind users
```

### Debugging

```poly
// Debug output with info command
info "Entering function process_data"
info "Parameter x = " + x.to_string()
info "Parameter y = " + y.to_string()

// Variable inspection
var debug_var: ustring = u"test"
info "debug_var = " + debug_var
info "debug_var.len() = " + debug_var.len().to_string()

// Loop debugging
loop: 0..10
    info "Iteration i = " + i.to_string()
    // ... loop body
end loop

// Error debugging
match result
    Ok(value) => 
        info "Success: " + value.to_string()
        process(value)
    Error(e) => 
        error "Error occurred: " + e.to_string()
        info "Error type: " + type_of(e)
        info "Stack trace: " + get_stack_trace()
end match

// Performance debugging
var start = time_now()
// ... code to measure
var duration = time_now() - start
info "Duration: " + duration.to_string() + "ms"

// Memory debugging
var mem_before = get_memory_usage()
// ... code to measure
var mem_after = get_memory_usage()
info "Memory used: " + (mem_after - mem_before).to_string() + " bytes"

// Conditional debugging
var debug_mode: bool = get_env("DEBUG") or u"false" == u"true"
if debug_mode,
    info "Debug mode enabled"
    info "Detailed output here"
end if

// Debug macros
macro debug(expr)
    info stringify(expr) + " = " + expr.to_string()
end macro

// Usage
debug(x)
debug(y)
debug(x + y)
```

### Internationalization

```poly
// Locale detection
var locale: ustring = get_locale()  // e.g., "en-US", "ja-JP", "zh-CN"
var language: ustring = locale.split(u"-")[0]  // e.g., "en", "ja", "zh"

// Translation function
fn translate(key: ustring, locale: ustring): ustring
    var translations: Map<ustring, Map<ustring, ustring>> = {
        u"greeting": {
            u"en": u"Hello",
            u"ja": u"こんにちは",
            u"zh": u"你好"
        },
        u"farewell": {
            u"en": u"Goodbye",
            u"ja": u"さようなら",
            u"zh": u"再见"
        }
    }
    
    return translations[key][language] or key
end fn

// Usage
var greeting: ustring = translate(u"greeting", locale)
put greeting + ", World!"

// Pluralization
fn pluralize(count: i32, singular: ustring, plural: ustring): ustring
    return if count == 1,singular else plural end if
end fn

// Usage
var item_count: i32 = 5
put item_count.to_string() + " " + pluralize(item_count, u"item", u"items")

// Date formatting
fn format_date(date: Date, locale: ustring): ustring
    match locale
        u"en-US" => return date.format(u"MM/DD/YYYY")
        u"en-GB" => return date.format(u"DD/MM/YYYY")
        u"ja-JP" => return date.format(u"YYYY年MM月DD日")
        u"zh-CN" => return date.format(u"YYYY年MM月DD日")
        _ => return date.format(u"YYYY-MM-DD")
    end match
end fn

// Number formatting
fn format_number(number: f64, locale: ustring): ustring
    match locale
        u"en-US" => return number.format(u"#,##0.00")  // 1,234.56
        u"de-DE" => return number.format(u"#.##0,00")  // 1.234,56
        u"ja-JP" => return number.format(u"#,##0.00")  // 1,234.56
        _ => return number.to_string()
    end match
end fn

// Currency formatting
fn format_currency(amount: f64, currency: ustring, locale: ustring): ustring
    var formatted_amount = format_number(amount, locale)
    
    match currency
        u"USD" => return u"$" + formatted_amount
        u"EUR" => return u"€" + formatted_amount
        u"JPY" => return u"¥" + formatted_amount
        u"CNY" => return u"¥" + formatted_amount
        _ => return currency + u" " + formatted_amount
    end match
end fn

// RTL (Right-to-Left) support
fn is_rtl(locale: ustring): bool
    var rtl_locales: Vec<ustring> = [u"ar", u"he", u"fa", u"ur"]
    return rtl_locales.contains(&locale.split(u"-")[0])
end fn

// Usage
var text_alignment = if is_rtl(locale),u"right" else u"left" end if
```

### Concurrency

```poly
// Thread spawning
spawn(|| {
    // Background task
    info "Background task started"
    // ... do work
    info "Background task completed"
})

// Async/await
async fn fetch_data(url: ustring): Result<ustring, ustring>
    var response = await get --timeout 5000 < url
    return Ok(response)
end fn

// Usage
var data = await fetch_data(u"https://api.example.com")

// Mutex for thread-safe access
var counter: Mutex<i32> = Mutex::new(0)

fn increment_counter()
    counter.lock()
    counter.value = counter.value + 1
    counter.unlock()
end fn

// Spawn multiple threads    loop: 0..10
        spawn(|| increment_counter())
    end loop

// Channel for communication
var channel: Channel<ustring> = Channel::new()

// Producer
spawn(|| {
    loop: 0..10
        channel.send(u"Message " + i.to_string())
    end loop
    channel.close()
})

// Consumer
while var message = channel.recv()
    info "Received: " + message
end while

// Async I/O
async fn process_files(filenames: Vec<ustring>): Vec<Result<ustring, ustring>>
    var futures: Vec<Future<Result<ustring, ustring>>> = []
    
    loop: filenames
        futures.push(async read_file(filename))
    end loop
    
    var results: Vec<Result<ustring, ustring>> = []
    loop: futures
        results.push(await future)
    end loop
    
    return results
end fn

// Parallel processing
fn parallel_process(data: Vec<T>): Vec<R>
    var chunk_size = data.len() / num_cpus()
    var chunks = data.chunks(chunk_size)
    
    var futures: Vec<Future<Vec<R>>> = []
    loop: chunks
        futures.push(async process_chunk(chunk))
    end loop
    
    var results: Vec<R> = []
    loop: futures
        results.extend(await future)
    end loop
    
    return results
end fn

// Race condition handling
var shared_data: Arc<Mutex<Vec<ustring>>> = Arc::new(Mutex::new([]))

fn concurrent_append(data: ustring)
    shared_data.lock()
    shared_data.value.push(data)
    shared_data.unlock()
end fn

// Timeout handling
async fn fetch_with_timeout(url: ustring, timeout_ms: i32): Result<ustring, TimeoutError>
    match get --timeout timeout_ms < url
        Ok(response) => return Ok(response)
        Timeout => return Error(TimeoutError::TimedOut)
        Error(e) => return Error(TimeoutError::NetworkError(e))
    end match
end fn
```

### Best Practices

1. **Use `-n` for progress indicators**: Keep output on the same line for loading animations.
2. **Use `error`/`warn`/`info` appropriately**: Reserve for actual errors, warnings, and debug info.
3. **Use `--default` for optional input**: Provide sensible defaults for better UX.
4. **Use `--timeout` for interactive input**: Prevent programs from hanging.
5. **Use `with validate` for data validation**: Catch errors early.

### Grammar Addition

```bnf
<put_stmt> ::= "put" <flag>* <expr> (redir)?
             | "error" <expr>
             | "warn" <expr>
             | "info" <expr>

<flag> ::= "-n"           // No newline
         | "/e"           // (Reserved for future use)

<redir> ::= ">" <string>
          | ">>" <string>
```

**Note:** The `-n` flag suppresses the trailing newline. The `error`, `warn`, and `info` commands always output to stderr with a newline.

---

## 17. Input: The `get` Command

### Overview

The `get` command is Poly's primary input mechanism, providing a simple syntax for reading from stdin or files. It uses shell-like redirection operators for file input and can parse values directly into typed variables.

### Key Concepts

1. **Type inference**: The language automatically parses input based on the variable type. No need for explicit parsing functions.
2. **Flags for simple options**: Use `--timeout`, `--default`, `--mask`, `--as`, `--until`, `--bytes` for common input options.
3. **'with' syntax for complex options**: Use `with validate`, `with complete`, `with encoding` for complex options that need closures or arrays.
4. **File input**: Use `<` operator to read from files. Supports both text and binary input.

### Syntax Variants

```poly
// Read a line from stdin (returns ustring)
var line: ustring = get

// Read with prompt (output prompt before reading)
var name: ustring = get u"Enter name: "

// Read typed value from stdin (auto-parse)
var age: i32 = get
var pi: f64 = get
var active: bool = get

// Read from file (entire file as ustring)
var content: ustring = get < "filename.txt"

// Read from file (line by line)
var line: ustring = get < "data.txt"

// Read binary data from file (as bytes)
var data: bytes = get < "binary.bin"

// Read with timeout (in milliseconds)
var input: ustring = get --timeout 5000

// Read with default value
var input: ustring = get --default u"value"

// Read with mask (password input)
var input: ustring = get --mask u"*"

// Read specific number of bytes
var header: bytes = get < "image.png" --bytes 8

// Read and convert to type
var input: i32 = get --as i32

// Read until delimiter
var input: ustring = get --until u"," 

// Read with validation (uses 'with' syntax for closures)
var input: i32 = get with validate |x| x > 0

// Read with completion (uses 'with' syntax for arrays)
var input: ustring = get with complete [u"start", u"stop"]
```

### Basic Examples

```poly
// Simple input
var line: ustring = get
put "You entered: " + line

// Input with prompt
var name: ustring = get "What is your name? "
put "Hello, " + name + "!"

// Typed input (auto-parsing)
put "Enter your age: "
var age: i32 = get
put "In 10 years you will be: " + (age + 10)

// Multiple inputs
put "Enter first number: "
var a: f64 = get
put "Enter second number: "
var b: f64 = get
put "Sum: " + (a + b)

// Boolean input
put "Enable debug mode? (true/false): "
var debug: bool = get
if debug,
    put "Debug mode enabled"
end if

// Input with default value
var color: ustring = get --default u"blue"
put "Color: " + color

// Input with timeout
match get --timeout 3000
    Ok(input) => put "You typed: " + input
    Timeout => put "Too slow!"
    Error(e) => error "Error: " + e
end match

// Input with validation
var age: i32 = get with validate |x| x >= 1 && x <= 100
put "Valid age: " + age
```

### File Input

```poly
// Read entire file
var config: ustring = get < "config.txt"
put u"Config length: " + config.len()

// Read file line by line
var file = open("data.txt")
while !file.eof()
    var line: ustring = file.get_line()
    put u"Line: " + line
end while

// Read specific number of bytes
var header: bytes = get < "image.png" with bytes 8

// Read with encoding specification
var text: ustring = get < "utf8.txt" with encoding u"utf-8"
```

### Advanced Input Features

```poly
// Input with validation
var age: i32 = get with validate |x| x > 0 && x < 150

// Input with default value (on empty input)
var name: ustring = get --default u"Anonymous"

// Input with mask (for passwords)
var password: ustring = get --mask u"*"

// Input with completion (for interactive shells)
var command: ustring = get with complete [u"start", u"stop", u"pause"]

// Read until delimiter
var csv_line: ustring = get --until u"," 

// Read with timeout
match get --timeout 5000
    Ok(input) => process(input)
    Timeout => error "Input timeout"
    Error(e) => error "Error: " + e
end match

// Read structured input
struct Person
    var name: ustring
    var age: i32
end struct

var person: Person = get as Person

// Input in loop with break condition
var inputs: Vec<ustring> = []
loop
    var input: ustring = get
    if input == u"quit",
        break
    end if
    inputs.push(input)
end loop
```

### Error Handling

```poly
// Input can fail (returns Result)
match get
    Ok(line) => process(line)
    Error(e) => put u"Error reading input: " + e
end match

// Using try for error propagation
fn read_config(): Result<Config, Error>
    var line = try get  // Propagate error
    var config = try parse_config(line)
    return Ok(config)
end fn

// Timeout handling
match get --timeout 1000
    Ok(input) => process(input)
    Timeout => put u"Input timeout"
    Error(e) => put u"Error: " + e
end match
```

### Additional Examples

```poly
// Input with tuple parsing
put u"Enter coordinates (x,y): "
var coords: (i32, i32) = get as (i32, i32)
var (x, y) = coords
put u"X: " + x + u", Y: " + y

// Input with list parsing
put u"Enter numbers separated by spaces: "
var numbers: Vec<i32> = get as Vec<i32>
var sum: i32 = numbers.reduce(0, |acc, n| acc + n)
put u"Sum: " + sum

// Custom validation function
fn is_valid_email(email: ustring): bool
    return email.contains(u"@") && email.ends_with(u".com")
end fn

var email: ustring = get with validate |email| is_valid_email(email)

// Timeout with default fallback
var input: ustring = match get --timeout 5000
    Ok(input) => input
    Timeout => u"default"
    Error(e) => u"default"
end match

// Masked input with validation
var password: ustring = get --mask u"*" with validate |p| p.len() >= 8

// Structured input parsing (JSON-like)
struct Config
    var width: i32
    var height: i32
    var title: ustring
end struct

put u"Enter config as 'width,height,title': "
var config: Config = get as Config

// Error recovery with retry
var valid_input: i32 = loop
    put u"Enter a positive number: "
    match get
        Ok(input) =>
            var num: i32 = input.parse::<i32>()
            if num > 0,
                break num
            else
                put u"Please enter a positive number"
            end if
        Error(e) => put u"Invalid input: " + e
    end match
end loop

// Input from file with encoding and validation
var data: ustring = get < "data.csv" with encoding u"utf-8" with validate |content| content.len() > 0

// Completion with default value
var action: ustring = get with complete [u"start", u"stop", u"pause"] --default u"stop"

// Multiple inputs with validation
put u"Enter start date (YYYY-MM-DD): "
var start: ustring = get with validate |d| d.len() == 10 && d[4] == u'-' && d[7] == u'-'
put u"Enter end date (YYYY-MM-DD): "
var end: ustring = get with validate |d| d.len() == 10 && d[4] == u'-' && d[7] == u'-' && d > start
```

### Transpilation Rules

| Poly Syntax | Rust Output |
|-------------|-------------|
| `var x = get` | `let mut x = String::new(); std::io::stdin().read_line(&mut x).unwrap(); x = x.trim().to_string();` |
| `var x: i32 = get` | `let mut input = String::new(); std::io::stdin().read_line(&mut input).unwrap(); let x: i32 = input.trim().parse().unwrap();` |
| `var x = get u"prompt"` | `print!("prompt"); std::io::stdout().flush().unwrap(); let mut x = String::new(); std::io::stdin().read_line(&mut x).unwrap(); x = x.trim().to_string();` |
| `var x = get < "file"` | `let x = std::fs::read_to_string("file").unwrap();` |
| `var x: bytes = get < "file"` | `let x = std::fs::read("file").unwrap();` |
| `get --timeout 5000` | `// Uses std::thread::spawn with timer and channel to implement timeout. See standard library.` |
| `get --default u"val"` | `// If input empty, returns default value. See standard library.` |
| `get --mask u"*"` | `// Uses terminal raw mode to mask input characters. See standard library.` |
| `get --as i32` | `// Parse as specified type. See standard library.` |
| `get --until u","` | `// Reads until delimiter found in input stream. See standard library.` |
| `get with validate \|x\| ...` | `// Loops until validation closure returns true. See standard library.` |
| `get with complete [...]` | `// Uses line editor library for completion. See standard library.` |

### Comparison with Similar Commands

| Feature | Poly `get` | Rust | Python | Go |
|---------|------------|------|--------|----|
| Basic input | `var x = get` | `stdin().read_line()` | `input()` | `bufio.NewReader()` |
| With prompt | `get u"prompt"` | `print! + read_line` | `input("prompt")` | `fmt.Print + ReadString` |
| Typed input | `var x: i32 = get` | `read_line + parse` | `int(input())` | `Scanf("%d", &x)` |
| File input | `get < "file"` | `fs::read_to_string` | `open().read()` | `os.ReadFile` |
| Binary input | `get < "file"` (bytes) | `fs::read` | `open().read()` | `os.ReadFile` |
| Validation | `get with validate` | Built-in | (manual) | (manual) |
| Default value | `get --default` | Built-in | (manual) | (manual) |
| Timeout | `get --timeout` | Built-in | (manual) | (manual) |
| Mask | `get --mask` | Built-in | (manual) | (manual) |
| Completion | `get with complete` | Built-in | (manual) | (manual) |

### Best Practices

1. **Always provide prompts**: Use `put -n` before `get` to show users what to enter.
2. **Use `--default` for optional fields**: Provide sensible defaults for better UX.
3. **Use `--timeout` for interactive input**: Prevent programs from hanging.
4. **Use `with validate` for data validation**: Catch errors early.
5. **Use `--mask` for sensitive input**: Mask password input.

### Grammar Addition

```bnf
<get_expr> ::= "get" (<prompt>)? (<flag>)* (<modifier>)*

<prompt> ::= <ustring>

<flag> ::= "--timeout" <number>    // Timeout in ms
         | "--default" <expr>      // Default value
         | "--mask" <ustring>      // Input mask
         | "--as" <type>           // Type conversion
         | "--until" <ustring>     // Delimiter
         | "--bytes" <number>      // Byte count

<modifier> ::= "<" <string>             // File input
             | "with" "validate" <closure> // Validation (complex)
             | "with" "complete" <array_expr>  // Completion (complex)
             | "with" "encoding" <ustring> // Encoding
```

**Note:** Simple options use flags (`--timeout`, `--default`, etc.). Complex options like validation closures and completion arrays use `with` syntax for readability.

---

## 18. Standard Library

### Platform Detection

```poly
// Platform detection functions
fn is_windows(): bool
    // Returns true if running on Windows
end fn

fn is_macos(): bool
    // Returns true if running on macOS
end fn

fn is_linux(): bool
    // Returns true if running on Linux
end fn

fn is_unix(): bool
    // Returns true if running on Unix-like system (Linux, macOS, etc.)
end fn

// Platform-specific path separators
fn path_separator(): ustring
    return if is_windows(),"\\" else "/" end if
end fn

// Platform-specific line endings
fn line_ending(): ustring
    return if is_windows(),"\r\n" else "\n" end if
end fn
```

### Debugging Utilities

```poly
// Debugging utility functions
fn stringify(expr: any): ustring
    // Converts any expression to string representation
end fn

fn type_of(value: any): ustring
    // Returns the type name of a value
end fn

fn get_stack_trace(): ustring
    // Returns the current stack trace as a string
end fn

fn get_memory_usage(): i64
    // Returns current memory usage in bytes
end fn

fn get_timestamp(): i64
    // Returns current timestamp in milliseconds
end fn

fn get_version(): ustring
    // Returns the application version
end fn

fn get_env(name: ustring): Option<ustring>
    // Returns environment variable value
end fn

fn set_env(name: ustring, value: ustring)
    // Sets an environment variable
end fn
```

### Internationalization

```poly
// Internationalization utility functions
fn get_locale(): ustring
    // Returns current locale (e.g., "en-US", "ja-JP")
end fn

fn translate(key: ustring, locale: ustring): ustring
    // Translates a key to the specified locale
end fn

fn pluralize(count: i32, singular: ustring, plural: ustring): ustring
    // Returns singular or plural form based on count
end fn

fn format_date(date: Date, locale: ustring): ustring
    // Formats date according to locale conventions
end fn

fn format_number(number: f64, locale: ustring): ustring
    // Formats number according to locale conventions
end fn

fn format_currency(amount: f64, currency: ustring, locale: ustring): ustring
    // Formats currency according to locale conventions
end fn

fn is_rtl(locale: ustring): bool
    // Returns true if locale is right-to-left
end fn
```

### Collections

```poly
// Vec<T> - Dynamic array
var vec: Vec<i32> = [1, 2, 3]
vec.push(4)
vec.pop()
vec.len()
vec.is_empty()
vec.contains(&3)
vec.iter()
vec.iter_mut()
vec.into_iter()

// Map<K, V> - Hash map
var map: Map<ustring, i32> = []
map.insert(u"key", 42)
map.get(u"key")
map.remove(u"key")
map.contains_key(u"key")
map.len()
map.iter()

// Set<T> - Hash set
var set: Set<i32> = []
set.insert(1)
set.remove(&1)
set.contains(&1)
set.len()

// Deque<T> - Double-ended queue
var deque: Deque<i32> = []
deque.push_front(1)
deque.push_back(2)
deque.pop_front()
deque.pop_back()

// Stack<T> - LIFO stack
var stack: Stack<i32> = []
stack.push(1)
stack.pop()
stack.peek()

// Queue<T> - FIFO queue
var queue: Queue<i32> = []
queue.enqueue(1)
queue.dequeue()
queue.peek()
```

### I/O

```poly
// Standard I/O (preferred: use 'put' for output, 'get' for input)
put u"Hello, World!"     // Output to stdout
put -n u"Enter value: "  // No newline
error u"Error message"   // Error message to stderr
warn u"Warning"          // Warning to stderr
info u"Debug info"       // Debug info to stderr

// Input with get
var name: ustring = get u"Enter name: "  // Input with prompt
var age: i32 = get                        // Typed input
var line: ustring = get                   // Simple line input

// Legacy syntax (still supported)
print(u"Hello, World!")
print_err(u"Error message")

// File I/O
var file = File::open(u"test.txt")?
var content = file.read_to_string()?
file.write(u"Hello")?
file.close()

// Buffered I/O
var reader = BufReader::new(file)
var line = reader.read_line()?

// Path operations
var path = Path::new(u"/home/user/file.txt")
var exists = path.exists()
var parent = path.parent()
var filename = path.filename()
```

### String Operations

```poly
// String manipulation
var s: ustring = u"Hello, World!"
s.len()              // Length
s.is_empty()         // Check empty
s.contains(u"World") // Substring check
s.starts_with(u"Hello")
s.ends_with(u"World!")
s.to_upper()         // Uppercase
s.to_lower()         // Lowercase
s.trim()             // Trim whitespace
s.split(u",")        // Split
s.replace(u"World", u"Poly")
s.find(u"World")     // Find index
s.chars()            // Character iterator
s.bytes()            // Byte iterator
s.parse::<i32>()     // Parse to integer
```

### Math Operations

```poly
// Basic math
abs(x)
sqrt(x)
pow(x, n)
log(x)
exp(x)

// Trigonometry
sin(x)
cos(x)
tan(x)
asin(x)
acos(x)
atan(x)

// Min/Max
min(a, b)
max(a, b)
clamp(value, min, max)

// Random
random::<i32>()     // Random integer
random_range(1, 10) // Random in range
```

### Iterators

```poly
// Iterator methods
var iter = vec.iter()
iter.next()          // Next element
iter.count()         // Count elements
iter.sum()           // Sum elements
iter.product()       // Product of elements
iter.min()           // Minimum
iter.max()           // Maximum
iter.any(|x| x > 5) // Any match
iter.all(|x| x > 0) // All match
iter.collect()       // Collect to collection

// Range iterators
(0..10).iter()       // 0 to 9
(0..=10).iter()      // 0 to 10
(0..10).rev().iter() // 10 down to 0
```

### Concurrency

```poly
// Threads
var handle = thread::spawn(|| 
    print(u"Hello from thread!")
end)
handle.join()

// Channels (message passing)
var (tx, rx) = channel()
thread::spawn(move ||
    tx.send(u"Hello!")
end)
var msg = rx.recv()

// Mutex (shared state)
var data: Mutex<Vec<i32>> = Mutex::new([])
{
    var guard = data.lock()
    guard.push(1)
} // Guard dropped here

// Arc (atomic reference counting)
var shared: Arc<Mutex<i32>> = Arc::new(Mutex::new(0))
var clone = Arc::clone(&shared)
thread::spawn(move ||
    var guard = clone.lock()
    *guard += 1
end)

// Async/Await
async fn fetch_data(url: ustring): Result<ustring, ustring>
    var response = await get --timeout 5000 < url
    return Ok(response)
end fn

var data = await fetch_data(u"https://api.example.com")

// Futures
var future = async long_running_task()
// Do other work...
var result = await future

// Channels (detailed)
struct Channel<T>
    sender: Sender<T>
    receiver: Receiver<T>
end struct

fn channel<T>(): (Sender<T>, Receiver<T>)
    // Creates a new channel
end fn

// Mutex (detailed)
struct Mutex<T>
    data: T
    lock: Lock
end struct

fn Mutex::new(data: T): Mutex<T>
    // Creates a new mutex
end fn

fn Mutex::lock(&self): MutexGuard<T>
    // Acquires the lock
end fn

// Arc (detailed)
struct Arc<T>
    data: T
    reference_count: AtomicI32
end struct

fn Arc::new(data: T): Arc<T>
    // Creates a new atomic reference
end fn

fn Arc::clone(arc: &Arc<T>): Arc<T>
    // Clones the reference
end fn

// ThreadPool
struct ThreadPool
    workers: Vec<Worker>
    sender: Sender<Job>
end struct

fn ThreadPool::new(size: i32): ThreadPool
    // Creates a thread pool with specified number of workers
end fn

fn ThreadPool::execute<F>(&self, job: F)
    where F: FnOnce() + Send + 'static
    // Executes a job in the thread pool
end fn
```

---

## 19. Transpilation & Target Project Structure

### Project Structure

```
my_poly_project/
├── Cargo.toml
├── poly.toml           # Poly project configuration
├── src/
│   ├── main.rs         # Entry point
│   ├── lib.rs          # Library root
│   ├── modules/
│   │   ├── mod.rs      # Module declarations
│   │   ├── math.rs     # Math module
│   │   └── io.rs       # I/O module
│   └── types/
│       ├── mod.rs      # Type declarations
│       └── structs.rs  # Struct definitions
├── tests/              # Integration tests
└── benches/            # Benchmarks
```

### Complete Transpilation Mapping

#### Output Commands

| Poly Syntax | Rust Output |
|-------------|-------------|
| `put expr` | `println!("{}", expr);` |
| `put -n expr` | `print!("{}", expr);` |
| `error expr` | `eprintln!("[ERROR] {}", expr);` |
| `warn expr` | `eprintln!("[WARN] {}", expr);` |
| `info expr` | `eprintln!("[INFO] {}", expr);` |
| `put expr > "file"` | `std::fs::write("file", expr.to_string()).unwrap();` |
| `put expr >> "file"` | Append to file with writeln |
| `put "{x}"` | `println!("{}", x);` |
| `put "{x:.2f}"` | `println!("{:.2f}", x);` |

**Note:** The `error`, `warn`, and `info` commands automatically prepend level indicators (`[ERROR]`, `[WARN]`, `[INFO]`) to the output for easier log filtering.

#### Input Commands

| Poly Syntax | Rust Output |
|-------------|-------------|
| `var x = get` | `let mut x = String::new(); std::io::stdin().read_line(&mut x).unwrap(); x = x.trim().to_string();` |
| `var x: i32 = get` | `let mut input = String::new(); std::io::stdin().read_line(&mut input).unwrap(); let x: i32 = input.trim().parse().unwrap();` |
| `var x = get u"prompt"` | `print!("prompt"); std::io::stdout().flush().unwrap(); let mut x = String::new(); std::io::stdin().read_line(&mut x).unwrap(); x = x.trim().to_string();` |
| `var x = get < "file"` | `let x = std::fs::read_to_string("file").unwrap();` |
| `var x: bytes = get < "file"` | `let x = std::fs::read("file").unwrap();` |
| `get --timeout 5000` | `// Uses std::thread::spawn with timer and channel to implement timeout. See standard library.` |
| `get --default u"val"` | `// If input empty, returns default value. See standard library.` |
| `get --mask u"*"` | `// Uses terminal raw mode to mask input characters. See standard library.` |
| `get --as i32` | `// Parse as specified type. See standard library.` |
| `get --until u","` | `// Reads until delimiter found in input stream. See standard library.` |
| `get --bytes 8` | `// Read specified number of bytes. See standard library.` |
| `get with validate \|x\| ...` | `// Loops until validation closure returns true. See standard library.` |
| `get with complete [...]` | `// Uses line editor library for completion. See standard library.` |

#### Other Constructs

| Poly Construct | Rust Output |
|----------------|-------------|
| **Variables & Constants** | |
| `var x: i32 = 10` | `let mut x: i32 = 10;` |
| `let x: i32 = 10` | `let x: i32 = 10;` |
| `const MAX = 100` | `const MAX: i32 = 100;` |
| **Arithmetic** | |
| `add x, 5` | `x += 5;` |
| `sub x, 3` | `x -= 3;` |
| `x += 5` | `x += 5;` |
| **Functions** | |
| `fn foo() { }` | `fn foo() { }` |
| `fn foo(): i32` | `fn foo() -> i32` |
| `fn foo(x: i32)` | `fn foo(x: i32)` |
| `fn foo(x: &i32)` | `fn foo(x: &i32)` |
| `fn foo(x: &mut i32)` | `fn foo(x: &mut i32)` |
| **Control Flow** | |
| `if x,` | `if x {` |
| `else if x,` | `} else if x {` |
| `else` | `} else {` |
| `end if` | `}` |
| `while x` | `while x {` |
| `end while` | `}` |
| `loop` | `loop {` |
| `end loop` | `}` |
| `loop: 0..10` | `for i in 0..10 {` |
| `loop: items` | `for item in items {` |
| `end loop` | `}` |
| `match x` | `match x {` |
| `pattern => expr` | `pattern => expr,` |
| `end match` | `}` |
| **Structs** | |
| `struct Foo { }` | `struct Foo { }` |
| `impl Foo { }` | `impl Foo { }` |
| **Enums** | |
| `enum Foo { A, B }` | `enum Foo { A, B }` |
| `Foo::A` | `Foo::A` |
| **Traits** | |
| `trait Foo { }` | `trait Foo { }` |
| `impl Foo for Bar { }` | `impl Foo for Bar { }` |
| `fn foo<T: Trait>()` | `fn foo<T: Trait>()` |
| **Modules** | |
| `module foo` | `mod foo {` |
| `use foo::bar` | `use foo::bar;` |
| `pub fn foo()` | `pub fn foo()` |
| **Error Handling** | |
| `try expr` | `expr?` |
| `Result<T, E>` | `Result<T, E>` |
| `Option<T>` | `Option<T>` |
| `Ok(val)` | `Ok(val)` |
| `Error(e)` | `Err(e)` |
| `Some(val)` | `Some(val)` |
| `None` | `None` |
| `panic(msg)` | `panic!("{}", msg)` |
| **Pointers** | |
| `ptr T` | `*const T` / `*mut T` |
| `deref ptr` | `unsafe { *ptr }` |
| `addr val` | `&val as *const _` |
| **Smart Pointers** | |
| `Box::new(val)` | `Box::new(val)` |
| `Rc::new(val)` | `Rc::new(val)` |
| `Arc::new(val)` | `Arc::new(val)` |
| **Closures** | |
| `\|x\| expr` | `\|x\| expr` |
| `\|x\| { body }` | `\|x\| { body }` |
| `move \|\| { }` | `move \|\| { }` |
| **Collections** | |
| `Vec<T>` | `Vec<T>` |
| `Map<K, V>` | `HashMap<K, V>` |
| `Set<T>` | `HashSet<T>` |
| `[1, 2, 3]` | `vec![1, 2, 3]` |
| **Output** | |
| `put expr` | `println!("{}", expr);` |
| `put -n expr` | `print!("{}", expr);` |
| `error expr` | `eprintln!("[ERROR] {}", expr);` |
| `warn expr` | `eprintln!("[WARN] {}", expr);` |
| `info expr` | `eprintln!("[INFO] {}", expr);` |
| `put expr > "f"` | `std::fs::write("f", expr)` |
| `put expr >> "f"` | `writeln!(f, "{}", expr)` |

---

## 20. Implementation Roadmap

### Phase 1: Core Language (Complete) ✓

- [x] Variables and constants
- [x] Basic arithmetic
- [x] Primitive types
- [x] Character/string types
- [x] Structs
- [x] Raw pointers
- [x] Basic functions
- [x] Basic control flow (if/else, while, loop)

### Phase 2: Essential Features (In Progress)

- [ ] For loops (range-based)
- [ ] Match expressions
- [ ] Enums with data
- [ ] Option and Result types
- [ ] Error propagation (try/?)
- [ ] Module system
- [ ] I/O commands (put/get)

### Phase 3: Type System Enhancement

- [ ] Traits and trait bounds
- [ ] Generics
- [ ] Closures
- [ ] Lifetime annotations
- [ ] Smart pointers (Box, Rc, Arc)

### Phase 4: Advanced Features

- [ ] Pattern matching (guards, destructuring)
- [ ] Iterator methods
- [ ] Async/await
- [ ] Macros
- [ ] Const generics

### Phase 5: Standard Library

- [ ] Collections (Vec, Map, Set)
- [ ] I/O operations
- [ ] String manipulation
- [ ] Math operations
- [ ] Concurrency primitives

### Phase 6: Tooling & Ecosystem

- [ ] Poly compiler/transpiler
- [ ] Language server (LSP)
- [ ] Package manager integration
- [ ] Documentation generator
- [ ] Testing framework

### Phase 7: Polish & Optimization

- [ ] Performance optimizations
- [ ] Better error messages
- [ ] Expanded standard library
- [ ] Real-world examples
- [ ] Community building

---

## Appendix A: Comparison with Similar Languages

| Feature | Poly | Rust | Go | Zig |
|---------|------|------|-----|-----|
| Memory Safety | Via Rust | Native | GC | Manual |
| Learning Curve | Low | High | Medium | Medium |
| Syntax Style | Assembly | Mixed | Simple | C-like |
| Transpilation | To Rust | N/A | N/A | N/A |
| Pattern Matching | Yes | Yes | Limited | Yes |
| Generics | Yes | Yes | Yes | Comptime |
| Traits | Yes | Yes (Traits) | Interfaces | N/A |

---

## Appendix B: Grammar Summary (BNF)

```bnf
<program> ::= <module>*

<module> ::= "module" <identifier> <declaration>* "end" "module"

<declaration> ::= <variable_decl>
               | <constant_decl>
               | <function_decl>
               | <struct_decl>
               | <enum_decl>
               | <trait_decl>
               | <impl_decl>
               | <use_decl>

<variable_decl> ::= "var" <identifier> ":" <type> "=" <expr>
                  | "let" <identifier> ":" <type> "=" <expr>

<constant_decl> ::= "const" <identifier> "=" <expr>

<function_decl> ::= "fn" <identifier> "(" <params>? ")" ("->" <type>)? <block>

<struct_decl> ::= "struct" <identifier> <struct_fields>? "end" "struct"

<enum_decl> ::= "enum" <identifier> <enum_variants>? "end" "enum"

<trait_decl> ::= "trait" <identifier> <function_decl>* "end" "trait"

<impl_decl> ::= "impl" <type> "for" <type> <function_decl>* "end" "impl"

<use_decl> ::= "use" <path> ("as" <identifier>)?

<block> ::= <statement>* "end"

<statement> ::= <variable_decl>
             | <assignment>
             | <expr>
             | <if_stmt>
             | <while_stmt>
             | <for_stmt>
             | <loop_stmt>
             | <match_stmt>
             | <return_stmt>

<if_stmt> ::= "if" <expr> "then" <block> ("else" "if" <expr> "then" <block>)* ("else" <block>)? "end" "if"

<while_stmt> ::= "while" <expr> <block> "end" "while"

<for_stmt> ::= "for" <identifier> "in" <expr> <block> "end" "for"

<loop_stmt> ::= "loop" <block> "end" "loop"

<match_stmt> ::= "match" <expr> <match_arm>* "end" "match"

<match_arm> ::= <pattern> "=>" <expr>

<pattern> ::= <literal>
            | <identifier>
            | "_"                    // Wildcard
            | <identifier> "(" <pattern> ("," <pattern>)* ")"  // Enum variant with data
            | <pattern> "@" <identifier>  // Binding
            | <pattern> "|" <pattern>     // Or pattern
            | "(" <pattern> ("," <pattern>)* ")"

<expr> ::= <literal>
         | <identifier>
         | <binary_expr>
         | <unary_expr>
         | <call_expr>
         | <index_expr>
         | <field_expr>
         | <closure_expr>
         | <match_expr>
         | <if_expr>
         | <array_expr>
         | "(" <expr> ")"

<array_expr> ::= "[" <expr> ("," <expr>)* "]"

<type> ::= <identifier>
         | <type> "<" <type_args> ">"
         | "[" <type> ";" <number> "]"
         | "(" <type> ("," <type>)* ")"
         | "ptr" <type>
         | "&" <type>
         | "&mut" <type>
```

---

**Document Version:** 1.4 Draft  
**Last Updated:** August 6, 2026  
**Status:** Work in Progress - Major Expansion Complete
