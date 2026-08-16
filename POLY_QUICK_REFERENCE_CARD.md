# Poly Language Quick Reference Card

## Variables & Constants

~~~poly
var x := 10              // Mutable variable
let y := 20              // Immutable variable
const PI := 3.14159      // Constant
var name String := "hi" // With type annotation
~~~

## Basic Types

| Type | Example |
|------|---------|
| `i8`-`i128` | `42i32` |
| `u8`-`u128` | `42u64` |
| `f32`, `f64` | `3.14` |
| `bool` | `true`, `false` |
| `String` | `"hello"` |
| `char` | `'a'` |
| `[T; N]` | `[1, 2, 3]` |
| `Vec<T>` | `vec![1, 2, 3]` |
| `(T, U)` | `(1, "hi")` |

## Operators

~~~poly
// Arithmetic: + - * / %
// Comparison: == != < > <= >=
// Logical: && || !
// Bitwise: & | ^ << >>
// Equality: =    Initialization: :=
~~~

## Control Flow

~~~poly fragment
// If/Else
if condition
    // ...
else if other
    // ...
else
    // ...
end if

// While
while condition
    // ...
end while

// For
for i in 0..10
    // ...
end for

for i in 0..=10      // Inclusive
for i in (0..100).step_by(2)
for item in collection
~~~

## Functions

~~~poly
fn name(param: Type): ReturnType
    // ...
    return value
end fn

fn greet(name: String = "World"): String
    return "Hello, " + name + "!"
end fn
~~~

## Data Structures

~~~poly
// Struct
struct Point
    var x: f64
    var y: f64
end struct

// Enum
enum Color
    Red
    Green
    Blue
end enum

// Enum with data
enum Shape
    Circle(f32)
    Rectangle(f32, f32)
end enum
~~~

## Pattern Matching

~~~poly fragment
match value
    pattern1, expression1
    pattern2 if guard, expression2
    _, default
end match
~~~

## Error Handling

~~~poly
error "Error message"     // Print error
warn "Warning message"    // Print warning
info "Info message"       // Print info
~~~

## File I/O

~~~poly
var content := get < "file.txt"     // Read file
put "text" > "file.txt"            // Write file
put "text" >> "file.txt"           // Append file
var input := get                     // Read stdin
~~~

## Closures

~~~poly fragment
var add := |x, y| x + y
var square := |x| x * x
~~~

## Modules

~~~poly fragment
module Math
    fn add(a, b): i32
        return a + b
    end fn
end module

use Math
Math.add(2, 3)
~~~

## Common Patterns

~~~poly
// List processing
var doubled := map(list, |x| x * 2)
var evens := filter(list, |x| x % 2 == 0)
var sum := reduce(list, |a, b| a + b, 0)

// String operations
var upper := str.to_uppercase()
var parts := str.split(",")
var joined := parts.join("-")

// Collections
vec.push(item)
vec.pop()
vec.length
vec.contains(item)
~~~

## Built-in Functions

~~~poly
put "text"              // Print with newline
put -n "text"           // Print without newline
error "msg"             // Print error
warn "msg"              // Print warning
info "msg"              // Print info
get                     // Read from stdin
get < "file"            // Read from file
~~~

## Comments

~~~poly
// Single line comment
/* Multi
   line
   comment */
~~~

---

## Complete Example

~~~poly
// Fibonacci with pattern matching
fn fibonacci(n: i32): i32
    match n
        0, 0
        1, 1
        _, fibonacci(n - 1) + fibonacci(n - 2)
    end match
end fn

fn main()
    for i in 0..10
        put "fib(" + i + ") = " + fibonacci(i)
    end for
end fn
~~~

---

**More info:** See `POLY_COMPREHENSIVE_GUIDE.md` for detailed documentation.
