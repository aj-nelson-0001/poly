# Poly Language Comprehensive Guide

> **Historical guide:** Examples and feature claims reflect the v1 documentation set. For the current v2 preview, use [POLY_SPEC_v2.md](POLY_SPEC_v2.md).

## Table of Contents

1. [Introduction](#introduction)
2. [Getting Started](#getting-started)
3. [Language Basics](#language-basics)
4. [Type System](#type-system)
5. [Control Flow](#control-flow)
6. [Functions](#functions)
7. [Data Structures](#data-structures)
8. [Pattern Matching](#pattern-matching)
9. [Error Handling](#error-handling)
10. [File I/O](#file-io)
11. [Advanced Features](#advanced-features)
12. [Real-World Examples](#real-world-examples)
13. [Best Practices](#best-practices)
14. [Common Pitfalls](#common-pitfalls)
15. [Performance Tips](#performance-tips)

---

## Introduction

Poly is a modern, expressive programming language that transpiles to Rust. It combines the simplicity of Python-like syntax with the power of Rust's type system and performance.

### Key Features

- **Clean Syntax**: Readable, Python-inspired syntax
- **Strong Typing**: Static type inference with optional type annotations
- **Pattern Matching**: Powerful exhaustive pattern matching
- **Algebraic Data Types**: Enums with unit, tuple, and struct variants
- **Error Handling**: Built-in error handling with `error`, `warn`, and `info` statements
- **File I/O**: Simple file reading and writing
- **Transpiles to Rust**: Generates efficient, idiomatic Rust code

### Example

~~~poly
// Hello World in Poly
put "Hello, World!"
~~~

Transpiles to:

~~~rust
fn main() {
    println!("{}", "Hello, World!");
}
~~~

---

## Getting Started

### Installation

~~~bash
# Clone the repository
git clone https://github.com/poly-lang/poly.git
cd poly

# Build the compiler
cargo build --release

# Run a Poly program
./target/release/poly examples/hello.poly
~~~

### Your First Program

Create a file called `hello.poly`:

~~~poly
// Variables
var name := "World"
var age := 25

// String interpolation
put "Hello, " + name + "!"
put "You are " + age + " years old."

// Functions
fn greet(person: String): String
    return "Hello, " + person + "!"
end fn

put greet("Alice")
~~~

Run it:

~~~bash
./target/release/poly hello.poly
~~~

---

## Language Basics

### Variables

Poly supports mutable and immutable variables:

~~~poly
// Mutable variables (can be changed)
var x := 10
x := 20// OK

// Immutable variables (cannot be changed)
let y := 10
// y = 20  // Error!

// Constants (compile-time constants)
const PI := 3.14159
const MAX_SIZE := 1024
~~~

### Type Annotations

Type annotations are optional but recommended:

~~~poly
// With type annotations
var name String := "Alice"
var age i32 := 30
var height f64 := 5.9
var active bool := true

// Without type annotations (type inferred)
var name := "Alice"  // Inferred as String
var age := 30        // Inferred as i32
~~~

### Basic Types

| Type | Description | Example |
|------|-------------|---------|
| `i8`, `i16`, `i32`, `i64`, `i128` | Signed integers | `42` |
| `u8`, `u16`, `u32`, `u64`, `u128` | Unsigned integers | `42u32` |
| `f32`, `f64` | Floating point | `3.14` |
| `bool` | Boolean | `true`, `false` |
| `String`, `ustring` | Strings | `"hello"` |
| `char` | Character | `'a'` |
| `byte` | Single byte | `0xFF` |
| `bytes` | Byte array | `[0x48, 0x65]` |

### Operators

~~~poly fragment
// Arithmetic
var sum := 10 + 5      // 15
var diff := 10 - 5     // 5
var product := 10 * 5   // 50
var quotient := 10 / 5  // 2
var remainder := 10 % 3  // 1

// Comparison
var equal := 10 == 10   // true
var not_equal := 10 != 5  // true
var less := 10 < 20     // true
var greater := 10 > 5   // true

// Logical
var and := true && false  // false
var or := true || false   // true
var not := !true          // false

// Bitwise
var band := 0xFF & 0x0F  // 0x0F
var bor := 0xF0 | 0x0F   // 0xFF
var bxor := 0xFF ^ 0x0F  // 0xF0
var shift := 1 << 4       // 16
~~~

---

## Type System

### Built-in Types

~~~poly
// Numeric types
var i i32 := 42
var u u64 := 100
var f f64 := 3.14

// Boolean
var b bool := true

// String
var s String := "hello"
var us ustring := "world"

// Collections
var arr Vec<i32> := [1, 2, 3, 4, 5]   // Vector literal
var vec Vec<i32> := [1, 2, 3]         // Equivalent
var tuple (i32, String, bool) := (42, "hello", true)

// Option and Result
var opt Option<i32> := Some(42)
var res Result<i32, String> := Ok(42)
~~~

### Custom Types

~~~poly
// Type aliases
type UserId := i32
type Email := String

// Generic structs
struct Wrapper<T>
    var value: T
end struct

// Enums (Option and Result are built-in generics)
enum Shape
    Circle(f32)
    Rectangle(f32, f32)
end enum
~~~

### Generic Functions

~~~poly fragment
// Generic identity function
fn identity<T>(value: T): T
    return value
end fn

// Generic with trait bound
fn print_value<T: Debug>(value: T)
    put value
end fn

// Generic containers
var boxed Box<i32> := Box::new(42)
var map Map<ustring, i32> := []
var set Set<i32> := []
~~~

---

## Control Flow

### If/Else

~~~poly
var age := 25

if age >= 18
    put "You are an adult."
else if age >= 13
    put "You are a teenager."
else
    put "You are a child."
end if
~~~

### While Loops

~~~poly
var i := 0
while i < 10
    put i
    i := i + 1
end while
~~~

### For Loops with Ranges

~~~poly
// Simple range (inclusive endpoints)
loop i 0..10
    put i
end loop

// With step
loop i 0..100 step 2
    put i
end loop

// Descending (negative step)
loop i 10..0 step -1
    put i
end loop
~~~

### Match Expressions

~~~poly fragment
var x := 42

match x
    0, put "Zero"
    1, put "One"
    n if n > 0, put "Positive"
    _, put "Other"
end match
~~~

---

## Functions

### Basic Functions

~~~poly fragment
fn add(a: i32, b: i32): i32
    return a + b
end fn

// Single expression body
fn double(x: i32): i32
    return x * 2
end fn

// No return value
fn greet(name: String)
    put "Hello, " + name + "!"
end fn
~~~

### Multiple Parameters

~~~poly
fn greet(name: String, greeting: String): String
    return greeting + ", " + name + "!"
end fn

put greet("Alice", "Hello")           // "Hello, Alice!"
put greet("Bob", "Hi")               // "Hi, Bob!"
~~~

### Higher-Order Functions

~~~poly fragment
fn apply(f: fn(i32) -> i32, x: i32): i32
    return f(x)
end fn

fn double(x: i32): i32
    return x * 2
end fn

fn square(x: i32): i32
    return x * x
end fn

put apply(double, 5)   // 10
put apply(square, 5)   // 25
~~~

### Closures

~~~poly
fn make_adder(x: i32): fn(i32) -> i32
    return |y| x + y
end fn

var add5 := make_adder(5)
put add5(10)   // 15
~~~

---

## Data Structures

### Arrays

~~~poly
// Vectors (dynamic arrays)
var arr Vec<i32> := [1, 2, 3, 4, 5]
put arr[0]      // 1
put arr.len()   // 5

// Multi-dimensional (element type inferred)
var matrix := [
    [1, 2, 3],
    [4, 5, 6],
    [7, 8, 9]
]

// Dynamic operations
var vec Vec<i32> := [1, 2, 3]
vec.push(4)
put vec.len()  // 4

// Iteration
for item in vec
    put item
end for
~~~

### Tuples

~~~poly fragment
var point (f64, f64) := (3.0, 4.0)
var (x, y) := point
put x  // 3.0
put y  // 4.0

// Access elements by position with `.N` (zero-based)
put point.0  // 3.0
put point.1  // 4.0

// Chained access works on nested tuples
var nested := (1, (2, 3))
put nested.1.0  // 2

// Elements are assignable
var pair := (10, true)
pair.0 := 99
pair.1 = false
add pair.0  // 99 -> 100

// `for` loops destructure tuples, e.g. from enumerate()
var items := [5, 6]
for (idx, val) in items.enumerate()
    put idx   // 0, 1
    put val   // 5, 6
end for
put items.len()  // collection still usable (loop borrows)

// Named tuples (planned)
// var point := (x: 3.0, y: 4.0)
~~~

### Structs

~~~poly
struct Point
    var x: f64
    var y: f64
end struct

// Methods
impl Point
    fn distance(self, other: Point): f64
        var dx := self.x - other.x
        var dy := self.y - other.y
        return (dx * dx + dy * dy)
    end fn
end impl

var p1 := Point { x: 0.0, y: 0.0 }
var p2 := Point { x: 3.0, y: 4.0 }
put p1.distance(p2)  // 5.0
~~~

### Enums

~~~poly
// Unit variants
enum Color
    Red
    Green
    Blue
end enum

// Tuple variants
enum Shape
    Circle(f32)
    Rectangle(f32, f32)
    Triangle(f32, f32, f32)
end enum

// Struct variants
enum Message
    Quit
    Move { x: i32, y: i32 }
    Write(String)
    ChangeColor(i32, i32, i32)
end enum
~~~

---

## Pattern Matching

### Basic Patterns

~~~poly fragment
var x := 42

match x
    0, put "Zero"
    1, put "One"
    2..=9, put "Small"
    10..=99, put "Medium"
    _, put "Large"
end match
~~~

### Destructuring

~~~poly fragment
var point := (3.0, 4.0)

match point
    (0.0, 0.0), put "Origin"
    (x, 0.0), put "On x-axis at " + x
    (0.0, y), put "On y-axis at " + y
    (x, y), put "Point at (" + x + ", " + y + ")"
end match
~~~

### Guard Conditions

~~~poly fragment
var age := 25

match age
    n if n < 0, put "Invalid"
    0, put "Just born"
    n if n < 13, put "Child"
    n if n < 18, put "Teenager"
    n if n < 65, put "Adult"
    _, put "Senior"
end match
~~~

### Nested Patterns

~~~poly
enum Expr
    Num(f32)
    Add(Expr, Expr)
    Mul(Expr, Expr)
end enum

fn evaluate(expr: Expr): f32
    match expr
        Num(n), n
        Add(l, r), evaluate(l) + evaluate(r)
        Mul(l, r), evaluate(l) * evaluate(r)
    end match
end fn

var expr := Add(Num(2.0), Mul(Num(3.0), Num(4.0)))
put evaluate(expr)  // 14.0
~~~

---

## Error Handling

### Error Statements

~~~poly
fn divide(a: f64, b: f64): f64
    if b = 0.0
        error "Division by zero"
    end if
    return a / b
end fn

fn validate_age(age: i32): bool
    if age < 0
        error "Age cannot be negative"
    end if
    if age > 150
        warn "Age seems unrealistic"
    end if
    info "Age validated: " + age
    return true
end fn
~~~

### Try/Catch (Planned)

~~~poly fragment
// Future syntax
try
    var content := get from "file.txt"
catch FileNotFoundError
    error "File not found"
catch PermissionError
    error "Permission denied"
end try
~~~

---

## File I/O

### Reading Files

~~~poly fragment
// Read entire file
var content := get from "file.txt"

// Read with error handling
try
    var content := get from "file.txt"
    put content
catch
    error "Could not read file"
end try
~~~

### Writing Files

~~~poly
// Write to file (overwrite)
put "Hello, World!" to "output.txt"

// Append to file
put "New line" to "output.txt" -append
~~~

### Command Line Input

~~~poly
// Read from stdin
var name := get
put "Hello, " + name + "!"
~~~

---

## Advanced Features

### Closures

~~~poly fragment
// Simple closure
var add := |x, y| x + y
put add(2, 3)  // 5

// Closure with capture
fn make_counter()
    var count := 0
    return || 
        count := count + 1
        return count
    end fn
end fn

var counter := make_counter()
put counter()  // 1
put counter()  // 2
~~~

### Higher-Order Functions

~~~poly fragment
// Map
fn map(arr: Vec<i32>, f: fn(i32) -> i32): Vec<i32>
    var result := vec![]
    for item in arr
        result.push(f(item))
    end for
    return result
end fn

// Filter
fn filter(arr: Vec<i32>, pred: fn(i32) -> bool): Vec<i32>
    var result := vec![]
    for item in arr
        if pred(item)
            result.push(item)
        end if
    end for
    return result
end fn

// Reduce
fn reduce(arr: Vec<i32>, f: fn(i32, i32) -> i32, init: i32): i32
    var acc := init
    for item in arr
        acc := f(acc, item)
    end for
    return acc
end fn

// Usage
var numbers := [1, 2, 3, 4, 5]
var doubled := map(numbers, |x| x * 2)
var evens := filter(numbers, |x| x % 2 == 0)
var sum := reduce(numbers, |a, b| a + b, 0)
~~~

### Modules

~~~poly fragment
module Math
    fn add(a: i32, b: i32): i32
        return a + b
    end fn
    
    fn multiply(a: i32, b: i32): i32
        return a * b
    end fn
end module

use Math

put Math.add(2, 3)      // 5
put Math.multiply(2, 3)  // 6
~~~

### Traits (Planned)

~~~poly fragment
trait Printable
    fn to_string(self): String
end trait

impl Printable for Point
    fn to_string(self): String
        return "(" + self.x + ", " + self.y + ")"
    end fn
end impl
~~~

---

## Real-World Examples

### Calculator

~~~poly
fn calculator()
    put "Simple Calculator"
    put "------------------"
    
    var running := true
    while running
        put "Enter first number (or 'quit' to exit): "
        var input := get
        
        if input = "quit"
            running := false
            continue
        end if
        
        var a f64 := get --as f64
        put "Enter operator (+, -, *, /): "
        var op := get
        put "Enter second number: "
        var b f64 := get --as f64
        
        var result := 0.0
        if op = "+"
            result := a + b
        else if op = "-"
            result := a - b
        else if op = "*"
            result := a * b
        else if op = "/"
            if b = 0.0
                error "Division by zero!"
            end if
            result := a / b
        else
            error "Invalid operator!"
        end if
        
        put "Result: " + result
    end while
end fn

calculator()
~~~

### File Processor

~~~poly fragment
fn process_file(input_path: String, output_path: String)
    put "Processing " + input_path + "..."
    
    try
        var content := get from  input_path
        var lines := content.split("\n")
        var processed := vec![]
        
        for line in lines
            // Simple processing: convert to uppercase
            var upper := line.to_uppercase()
            processed.push(upper)
        end for
        
        var output := processed.join("\n")
        put output > output_path
        
        put "Done! Output written to " + output_path
    catch
        error "Failed to process file"
    end try
end fn

process_file("input.txt", "output.txt")
~~~

### Simple Game

~~~poly fragment
fn hangman()
    var words := ["apple", "banana", "cherry", "date", "elderberry"]
    var word := words.random()
    var guessed := vec![]
    var attempts := 6
    
    put "Welcome to Hangman!"
    put "Word has " + word.length + " letters."
    
    while attempts > 0
        put "
Attempts left: " + attempts
        put "Guessed letters: " + guessed
        
        // Show current state
        var display := ""
        for char in word
            if guessed.contains(char)
                display := display + char
            else
                display := display + "_"
            end if
        end for
        put display
        
        if !display.contains("_")
            put "Congratulations! You won!"
            return
        end if
        
        put "Guess a letter: "
        var guess := get
        
        if guessed.contains(guess)
            put "Already guessed!"
            continue
        end if
        
        guessed.push(guess)
        
        if !word.contains(guess)
            put "Wrong!"
            attempts := attempts - 1
        end if
    end while
    
    put "Game over! The word was: " + word
end fn

hangman()
~~~

### HTTP Client (Planned)

~~~poly fragment
// Future syntax with standard library
use http

fn fetch_url(url: String): String
    var response := http.get(url)
    if response.status = 200
        return response.body
    else
        error "HTTP " + response.status
    end if
end fn

var html := fetch_url("https://example.com")
put html
~~~

---

## Best Practices

### 1. Use Meaningful Variable Names

~~~poly
// Good
var user_name := "Alice"
var total_price := 99.99
var is_authenticated := true

// Bad
var x := "Alice"
var tp := 99.99
var b := true
~~~

### 2. Prefer Immutable Variables

~~~poly fragment
// Good - immutable when possible
let config := load_config()
let user := get_current_user()

// Mutable only when needed
var counter := 0
counter := counter + 1
~~~

### 3. Use Pattern Matching

~~~poly fragment
// Good - exhaustive matching
fn process(value: Shape)
    match value
        Circle(r), process_circle(r)
        Rectangle(w, h), process_rect(w, h)
        Triangle(a, b, c), process_tri(a, b, c)
    end match
end fn

// Avoid nested if/else
// Bad
fn process(value: Shape)
    if value.type = "circle"
        process_circle(value.r)
    else if value.type = "rectangle"
        process_rect(value.w, value.h)
    end if
end fn
~~~

### 4. Handle Errors Explicitly

~~~poly fragment
// Good
fn read_config(path: String): String
    try
        return get from  path
    catch
        error "Failed to read config: " + path
    end try
end fn

// Bad - ignoring errors
fn read_config(path: String): String
    return get from  path  // Might panic!
end fn
~~~

### 5. Keep Functions Small

~~~poly fragment
// Good - single responsibility
fn validate_email(email: String): bool
    return email.contains("@") && email.contains(".")
end fn

fn send_welcome_email(user: User)
    var email := user.email
    if !validate_email(email)
        error "Invalid email address"
    end if
    // Send email logic
end fn

// Bad - doing too much
fn process_user(user: User)
    // Validation, database, email, logging...
    // Hard to test and maintain
end fn
~~~

---

## Common Pitfalls

### 1. Forgetting `end` Keywords

~~~poly fragment
// Wrong
fn add(a, b)
    return a + b

// Correct
fn add(a, b)
    return a + b
end fn
~~~

### 2. Mutable vs Immutable

~~~poly fragment
// Wrong - can't reassign an immutable `let` binding
let x := 10
x := 20  // Compile error!

// Correct - `var` is mutable
var x := 10
x := 20  // OK
~~~

### 3. Array Bounds

~~~poly
var arr := [1, 2, 3]
put arr[5]  // Panic!

// Safe access
if arr.len() > 5
    put arr[5]
end if
~~~

### 4. String Comparison

~~~poly
var a := "hello"
var b := "hello"
if a = b
    put "Equal"
end if  // This works!
~~~

---

## Performance Tips

### 1. Use Appropriate Types

~~~poly
// Good - use smallest type that fits
var small_num u8 := 255 as u8
var big_num u64 := 18446744073709551615 as u64

// Bad - using i64 for small values
var small_num i64 := 255  // Wastes memory
~~~

### 2. Pre-allocate Vectors

~~~poly fragment
// Good - pre-allocate
var vec := vec![0; 1000]

// Bad - dynamic resizing
var vec := vec![]
for i in 0..1000
    vec.push(i)  // May reallocate multiple times
end for
~~~

### 3. Avoid Unnecessary Clones

~~~poly fragment
// Good - borrow when possible
fn print_length(s: &ustring)
    put s.len()
end fn

// Bad - cloning
fn print_length(s: ustring)
    put s.len()  // s is dropped after
end fn
~~~

### 4. Use Iterators

~~~poly
// Good - iterator chain
var numbers := [1, -2, 3, -4, 5]
var sum := numbers.filter(|x| x > 0).reduce(0, |acc, x| acc + x)

// Bad - manual loop
var sum2 := 0
loop x in numbers
    if x > 0,
        sum2 := sum2 + x
    end if
end loop
~~~

---

## Conclusion

Poly provides a clean, expressive syntax while leveraging Rust's powerful type system and performance. By following these guidelines and examples, you can write efficient, maintainable Poly code that transpiles to optimal Rust.

### Resources

- [Poly Language Specification](POLY_LANGUAGE_SPECIFICATION_EXPANDED.md)
- [API Reference](POLY_API_REFERENCE.md)
- [Tutorial](POLY_TUTORIAL.md)
- [Examples](examples/)

### Getting Help

- Open an issue on GitHub
- Join our Discord community
- Check the FAQ section

Happy coding with Poly! 🚀
