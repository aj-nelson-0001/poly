# The Poly Programming Language

> **Historical reference:** This document describes the v1 language. It is retained for context and migration only; it is not normative for Poly 2.0.0-preview.1. See [POLY_DOCUMENTATION_INDEX.md](POLY_DOCUMENTATION_INDEX.md).

*A Tutorial Introduction*

---

## Chapter 1. Introduction

Poly is a systems programming language that transpiles to Rust. It draws its
syntax from assembly language and Sinclair QL SuperBASIC: explicit variable
declarations, comma-terminated conditions, and end-delimited blocks. The
resulting Rust is idiomatic and safe; Poly itself adds no runtime.

This book is a concise introduction to the language. We begin with the
smallest useful programs and build toward structures, pattern matching,
closures, and async programming. Each chapter introduces one new idea and
demonstrates it with short, complete examples.

### Conventions

- Poly code appears in blocks marked poly.
- Where it illuminates the semantics, generated Rust is shown alongside.
- All programs in this book can be transpiled with poly --check program.poly
  and compiled with rustc.

---

## Chapter 2. A First Program

The traditional starting point:

~~~poly
fn main()
    put "Hello, world!"
end fn
~~~

`put` writes to standard output with a trailing newline. The program is a
single function called main — the entry point. Transpiled to Rust:

~~~rust
fn main() {
    println!("{}", String::from("Hello, world!"));
}
~~~

Poly has no semicolons, no curly braces, and no trailing-expression syntax.
Blocks begin after the opening keyword and end with end <keyword>.

A comment starts with # and runs to the end of the line:

~~~poly
# This is a comment
~~~

---

## Chapter 3. Variables and Types

Poly variables are declared with var, a name, an optional type, and an
initializer using :=:

~~~poly
var x := 42            # inferred as i32
var y f64 := 3.14      # explicit type
var name := "Alice"    # inferred as String
~~~

The compiler infers types from the initializer when the type is omitted.
Once declared, a variable is mutable. Reassign with set or with the
= operator:

~~~poly
var count i32 := 0
count := 5
count = 10
~~~

### Primitive Types

| Type | Description | Size |
|------|-------------|------|
| i8, i16, i32, i64, i128 | Signed integers | 1–16 bytes |
| u8, u16, u32, u64, u128 | Unsigned integers | 1–16 bytes |
| f32, f64 | Floating-point | 4–8 bytes |
| bool | true or false | 1 byte |
| char | ASCII character | 1 byte |
| uchar | Unicode scalar (UTF-32) | 4 bytes |
| string | ASCII string (Vec<u8>) | variable |
| ustring | UTF-8 string (String) | variable |
| byte | Raw byte (u8) | 1 byte |
| bytes | Byte buffer (Vec<u8>) | variable |

Integer literals default to i32. Float literals default to f64. There is
no automatic narrowing — assignment to a smaller type requires an explicit
cast.

### Constants

Constants are declared with const and are immutable:

~~~poly
const MAX_SIZE := 1024
const PI := 3.141592653589793
~~~

---

## Chapter 4. Expressions

Poly expressions are familiar arithmetic with a few additions.

### Arithmetic

~~~poly
var a i32 := 10
var b i32 := 3

put a + b     # 13
put a - b     # 7
put a * b     # 30
put a / b     # 3 (integer division)
put a mod b   # 1 (modulo)
~~~

### Comparison and Logic

~~~poly fragment
put a > b       # true
put a = 10      # true (single = is equality)
put a != 5      # true
put a >= 10     # true

put true and false   # false
put true or false    # true
put not false        # true
~~~

Note: equality is =, not ==. Assignment uses := (declaration) or
= (reassignment).

### Bitwise Operations

~~~poly
var x i32 := 0b1010
var y i32 := 0b1100

put x bitand y    # 0b1000  (AND)
put x bitor y     # 0b1110  (OR)
put x xor y       # 0b0110  (XOR)
put bitnot x      # bitwise NOT
put x << 2        # left shift
put x >> 1        # right shift
~~~

### String Concatenation

The + operator concatenates strings. Numeric values are converted to
strings automatically:

~~~poly
var greeting := "Hello, " + "world!"
var n i32 := 42
put "The answer is " + n.to_string()
~~~

### String Interpolation

Embed expressions in strings with {}:

~~~poly
var name := "Ada"
var age i32 := 36
put "Hello, {name}! You are {age} years old."
~~~

---

## Chapter 5. Control Flow

### If Statements

Poly uses comma-terminated conditions:

~~~poly
var score i32 := 85

if score >= 90,
    put "A"
else if score >= 80,
    put "B"
else if score >= 70,
    put "C"
else,
    put "F"
end if
~~~

The comma after the condition is mandatory. The else if and else
branches are optional. There is no parenthesization around conditions.

### While Loops

~~~poly fragment
var i i32 := 0
while i < 5
    put i
    i += 1
end while
~~~

break exits the loop; continue skips to the next iteration. Both work
inside while loops.

### Infinite Loops

~~~poly
loop
    var input := get
    if input = "quit",
        break
    end if
    put "You said: " + input
end loop
~~~

### Range Loops (the loop command)

The loop command iterates over ranges and collections. The loop variable
follows the colon. Loop ranges are inclusive at both endpoints: 1..3
visits 1, 2, 3.

~~~poly
loop i 0..5
    put i
end loop

loop i 1..3, 7, 19..20
    put i      # 1, 2, 3, 7, 19, 20
end loop
~~~

Step and negative step:

~~~poly
loop i 0..10 step 2
    put i      # 0, 2, 4, 6, 8, 10
end loop

loop i 10..1 step -1
    put i      # 10, 9, 8, ..., 1
end loop
~~~

### For Loops

for iterates over collections:

~~~poly
var fruits := ["apple", "banana", "cherry"]

for fruit in fruits
    put fruit
end for
~~~

Destructuring with tuples:

~~~poly
var pairs := [(1, "one"), (2, "two"), (3, "three")]

for (num, label) in pairs
    put "{num}: {label}"
end for
~~~

### Break with Value

break inside a loop can carry a value that becomes the loop's result:

~~~poly fragment
var result := loop
    var n := get
    var parsed n i32 := n.parse::<i32>()
    if parsed > 0,
        break parsed
    end if
    put "Try again."
end loop
~~~

---

## Chapter 6. Functions

Functions are declared with fn, a name, parameters with types, and an
optional return type:

~~~poly
fn add(a: i32, b: i32): i32
    return a + b
end fn
~~~

Parameters are typed. The return type follows a colon. There is no
return keyword for single-expression bodies — but return is always
available and explicit:

~~~poly
fn abs(x: i32): i32
    if x < 0,
        return -x
    end if
    return x
end fn
~~~

A function without a return type returns nothing:

~~~poly
fn greet(name: ustring)
    put "Hello, " + name + "!"
end fn
~~~

### Default Parameters

~~~poly
fn power(base: f64, exp: i32): f64
    var result := 1.0
    loop i 0..exp - 1
        result = result * base
    end loop
    return result
end fn
~~~

### Nested Functions

Functions may be declared inside other functions:

~~~poly
fn outer(x: i32): i32
    fn double(n: i32): i32
        return n * 2
    end fn
    return double(x) + 1
end fn
~~~

---

## Chapter 7. Arrays and Vectors

### Array Literals

Array literals use square brackets:

~~~poly
var numbers := [1, 2, 3, 4, 5]
var names := ["Alice", "Bob", "Charlie"]
~~~

The type is inferred. To annotate:

~~~poly
var scores Vec<i32> := [90, 85, 78, 92, 88]
~~~

An empty array literal is polymorphic — it can initialize any container
type when the declaration specifies one:

~~~poly
var m Map<ustring, i32> := []
var s Set<i32> := []
var empty Vec<f64> := []
~~~

### Indexing

Arrays and vectors are zero-indexed:

~~~poly
var xs := [10, 20, 30]
put xs[0]    # 10
put xs[2]    # 30
~~~

### Checked Indexing

xs.get(i) returns Option<T> instead of panicking on an out-of-bounds
index:

~~~poly
var xs := [10, 20, 30]
match xs.get(1)
    Some(v), put v     # 20
    None, put "empty"
end match
~~~

### Vector Methods

~~~poly fragment
var xs Vec<i32> := [3, 1, 4, 1, 5]

xs.push(9)            # append an element
xs.clear()            # remove all elements
xs.reserve(100)       # pre-allocate capacity
put xs.len()           # number of elements
put xs.is_empty()      # true if len = 0
~~~

### Maps and Sets

~~~poly
var m Map<ustring, i32> := []
m.insert("one", 1)
m.insert("two", 2)
put m.get("one")       # Some(1)
put m.contains_key("one")  # true
m.remove("two")
put m.len()            # 1
put m["one"]           # 1 (reads via Option)

var s Set<i32> := []
s.insert(42)
put s.contains(42)    # true
s.remove(42)
~~~

---

## Chapter 8. Strings

Poly distinguishes ASCII strings (string) from Unicode strings (ustring).
In practice, ustring (which maps to Rust's String) is the default for
text:

~~~poly
var s := "hello"           # String (ASCII)
var t := unicode "hello"   # String (Unicode/UTF-8)
~~~

### String Operations

~~~poly
var s := "Hello, world!"

put s.len()                  # 13
put s.contains("world")     # true
put s.starts_with("Hello")  # true
put s.ends_with("!")        # true

put s.replace("world", "Poly")   # "Hello, Poly!"
put s.split(", ")                # ["Hello", "world!"]
put s.trim()                     # "Hello, world!"
put s.to_uppercase()             # "HELLO, WORLD!"
put s.to_lowercase()             # "hello, world!"
~~~

### Mutation

Strings are mutable with add and +=:

~~~poly fragment
var buf := "Hello"
buf := buf + " world"
buf += "!"
put buf    # "Hello world!"
~~~

### Repeat and Join

~~~poly
put "#".repeat(5)              # "#####"
put ["a", "b", "c"].join(", ")  # "a, b, c"
~~~

### Characters and Iteration

~~~poly fragment
var s := "hello"
loop c in s
    put c
end for
~~~

String methods that return iterators:

~~~poly fragment
put "hello".map(|c| c)           # Vec of chars
put "hello".filter(|c| c != 'l')  # ['h', 'e', 'o']
~~~

---

## Chapter 9. Structures

Structures group related data:

~~~poly
struct Point
    var x: f64
    var y: f64
end struct
~~~

The var prefix is optional on fields. Construct a struct with named
fields:

~~~poly fragment
var p := Point { x: 3.0, y: 4.0 }
put p.x    # 3.0
~~~

### Methods and Associated Functions

Methods are defined inside impl blocks. The first parameter self
receives the struct value:

~~~poly
struct Point
    var x: f64
    var y: f64
end struct

impl Point
    fn new(x: f64, y: f64): Point
        return Point { x: x, y: y }
    end fn

    fn distance_to(self, other: Point): f64
        var dx := self.x - other.x
        var dy := self.y - other.y
        return sqrt(dx * dx + dy * dy)
    end fn
end impl

fn main()
    var p1 := Point::new(0.0, 0.0)
    var p2 := Point::new(3.0, 4.0)
    put p1.distance_to(p2)    # 5.0
end fn
~~~

Point::new(...) is an associated function (no self). p1.distance_to(p2)
is a method call.

### Tuples

Tuples are lightweight compound values:

~~~poly
var pair := (10, true)
put pair.0    # 10
put pair.1    # true

var nested := (1, (2, 3))
put nested.1.0    # 2
~~~

Tuple assignment:

~~~poly fragment
pair.0 := 99
pair.1 = false
~~~

Destructuring:

~~~poly fragment
var (x, y) := (1, 2)
~~~

---

## Chapter 10. Enumerations

Enumerations define types with a fixed set of variants:

~~~poly
enum Direction
    North
    South
    East
    West
end enum
~~~

### Variants with Data

Variants can carry data. Tuple variants use parentheses; struct variants
use braces:

~~~poly fragment
enum Shape
    Circle(f64)
    Rectangle { width: f64, height: f64 }
end enum

var c := Shape::Circle(5.0)
var r := Shape::Rectangle { width: 10.0, height: 20.0 }
~~~

### Enums with Methods

~~~poly
enum TrafficLight
    Red
    Yellow
    Green

fn duration(self): i32
    match self
        Red, 60
        Yellow, 5
        Green, 45
    end match
end fn
end enum
~~~

### Recursive Enums

Recursive enum types are automatically boxed:

~~~poly
enum Expr
    Num(f32)
    Add(Expr, Expr)
    Mul(Expr, Expr)
    Neg(Expr)
end enum
~~~

---

## Chapter 11. Pattern Matching

match dispatches on a value's shape. Arms use comma syntax: pattern, expression.

~~~poly
var x i32 := 5
match x
    0, put "zero"
    1, put "one"
    _, put "other"
end match
~~~

### Matching Enums

~~~poly fragment
var color := Color::Red
match color
    Red, put "warm"
    Green, put "calm"
    Blue, put "cool"
    _, put "unknown"
end match
~~~

### Destructuring

Match arms can extract data from variants:

~~~poly
enum Shape
    Circle(f64)
    Rectangle(f64, f64)
end enum

fn area(s: Shape): f64
    match s
        Circle(r), 3.14159 * r * r
        Rectangle(w, h), w * h
    end match
end fn
~~~

### Match Guards

Add conditions with if:

~~~poly
var n i32 := 42
match n
    x if x < 0, put "negative"
    x if x = 0, put "zero"
    x if x > 0 and x < 100, put "small positive"
    _, put "large positive"
end match
~~~

### Range Patterns

~~~poly fragment
match score
    0..=59, put "F"
    60..=69, put "D"
    70..=79, put "C"
    80..=89, put "B"
    90..=100, put "A"
    _, put "out of range"
end match
~~~

### Binding Patterns

Bind a value while matching a pattern with @:

~~~poly fragment
match age
    n @ 0..12, put "child: " + n
    n @ 13..17, put "teenager: " + n
    _, put "adult"
end match
~~~

---

## Chapter 12. Closures

Closures are anonymous functions. The simplest form takes a single
expression:

~~~poly
var square := |x: i32| x * x
put square(4)    # 16
~~~

A multi-line closure:

~~~poly fragment
var classify := |x: i32|
    if x < 0,
        return "negative"
    else if x = 0,
        return "zero"
    else,
        return "positive"
    end if
end fn
~~~

### Closure Type Annotations

Annotate closure types in function parameters with |params| return_type:

~~~poly
fn apply(f: |x: i32| i32, v: i32): i32
    return f(v)
end fn

fn main()
    put apply(|x| x * 2, 21)    # 42
end fn
~~~

### Returning Closures

Functions can return closures. The return type uses |params| return_type
and lowers to impl Fn(...) in Rust:

~~~poly
fn make_adder(n: i32): |x: i32| i32
    return |x| x + n
end fn

fn main()
    var add5 := make_adder(5)
    put add5(10)    # 15
    put make_adder(100)(1)    # 101
end fn
~~~

---

## Chapter 13. Higher-Order Functions

Functions that take or return functions. The standard vector methods
map, filter, reduce, and sort_by take closures:

~~~poly
var xs := [1, 2, 3, 4, 5]

# map: transform each element
put xs.map(|x| x * 2)          # [2, 4, 6, 8, 10]

# filter: keep elements matching a predicate
put xs.filter(|x| x mod 2 = 0)   # [2, 4]

# reduce: combine all elements
put xs.reduce(0, |acc, x| acc + x)   # 15

# sort_by: sort with a comparator
put xs.sort_by(|a, b| a > b)    # [5, 4, 3, 2, 1]
~~~

### Iterator Chains

Use .iter() to get an iterator, then chain operations:

~~~poly
var xs := [1, 2, 3, 4, 5]
var sum i32 := xs.iter().sum()
var evens := xs.iter().filter(|x| x mod 2 = 0).collect()
~~~

### String Higher-Order Methods

Strings iterate over characters:

~~~poly fragment
put "hello".map(|c| c)                    # Vec of chars
put "hello".filter(|c| c != 'l')          # ['h', 'e', 'o']
put "hello".reduce(0, |acc, c| acc + (c as i32))   # sum of char codes
~~~

---

## Chapter 14. Error Handling

Poly uses Result<T, E> for operations that can fail. Ok(value) signals
success; Error(e) signals failure.

### Defining Error Types

~~~poly
enum FileError
    NotFound
    PermissionDenied
    InvalidData
end enum
~~~

### Returning Results

~~~poly fragment
fn read_config(path: ustring): Result<ustring, FileError>
    if path.len() = 0,
        return Error(FileError::NotFound)
    end if
    return Ok(unicode "content from " + path)
end fn
~~~

### Matching on Results

~~~poly fragment
match read_config(unicode "config.txt")
    Ok(content), put "loaded: " + content
    Error(FileError::NotFound), error "file not found"
    Error(FileError::PermissionDenied), error "permission denied"
    _, error "unknown error"
end match
~~~

### Error Propagation

Use try to propagate errors up the call chain (like Rust's ?):

~~~poly fragment
fn load_app(): Result<ustring, FileError>
    var config := try read_config(unicode "config.txt")
    # If read_config returned Error, we return it here.
    # Otherwise, config holds the Ok value.
    return Ok(config)
end fn
~~~

### Checked Accessors

~~~poly fragment
var r := Ok(42)
put r.is_ok()       # true
put r.is_error()    # false
put r.unwrap()      # 42
put r.unwrap_or(0)  # 42
~~~

---

## Chapter 15. Input and Output

### Output with put

~~~poly
put "Hello"              # println!
put "data" to "out.txt"   # write to file
put "line" to "log.txt"  # append to file
~~~

### Error and Warning Output

~~~poly
error "something went wrong"    # [ERROR] to stderr
warn "deprecated feature"       # [WARN] to stderr
info "debug info"               # [INFO] to stderr
~~~

### Input with get

~~~poly
var name := get                    # read a line from stdin
var name := get unicode "Name: "   # with prompt
var age i32 := get --as i32        # parse as integer
~~~

### File Reading

~~~poly fragment
var content := get from "data.txt"              # read file
var bytes := get from "image.png"              # read as bytes
~~~

### Flags

~~~poly
var input := get --timeout 5000              # Result with timeout
var input := get --default "fallback"        # default on EOF
var input := get --mask "*"                  # suppress echo (passwords)
var input := get --until ","                 # read until delimiter
~~~

### Test Helpers

~~~poly fragment
assert(age > 0)                     # panics if false
pass("all tests passed")            # prints [PASS]
fail("unexpected value")            # prints [FAIL], exits 1
~~~

---

## Chapter 16. Generics

Functions and structs can be parameterized by type:

~~~poly
fn identity<T>(x: T): T
    return x
end fn

fn main()
    put identity(42)       # i32
    put identity("hi")     # String
end fn
~~~

### Generic Containers

~~~poly fragment
fn first<T>(xs: Vec<T>): T
    return xs[0]
end fn

var nums := [1, 2, 3]
put first(nums)    # 1
~~~

### Generic Structs

~~~poly
struct Pair<A, B>
    var first: A
    var second: B
end struct

var p := Pair { first: 1, second: "hello" }
~~~

The compiler substitutes concrete types at call sites and rejects mismatches.

---

## Chapter 17. Async Programming

Poly supports async/await syntax. Async functions are declared with async fn
and awaited with .await:

~~~poly fragment
async fn fetch(url: ustring): Result<ustring, ustring>
    var response := http_get(url).await
    return response
end fn

fn main()
    var data := fetch(unicode "https://example.com").await
    match data
        Ok(body), put body
        Error(e), error e
    end match
end fn
~~~

The compiler detects async usage and adds #[tokio::main] automatically.

### Spawning Tasks

~~~poly fragment
spawn delay(1000).await
~~~

### Database Access

~~~poly fragment
var rows := db_execute(unicode "CREATE TABLE t (id INTEGER)").await
var rows := db_execute(unicode "SELECT * FROM t").await
~~~

The database is a shared in-memory SQLite instance. Tables and data persist
across calls within one program.

---

## Chapter 18. Putting It All Together

A program that reads names, validates them, and writes results:

~~~poly
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
        return Error(ValidationError::TooShort(2))
    end if
    if name.len() > 50,
        return Error(ValidationError::TooLong(50))
    end if
    return Ok(name)
end fn

fn main()
    put "Enter names (one per line, empty to quit):"
    loop
        var line := get
        if line.len() = 0,
            break
        end if
        match validate_name(line)
            Ok(valid), put "OK: " + valid
            Error(EmptyInput), error "name is empty"
            Error(TooShort(min)), error "too short (min " + min + ")"
            Error(TooLong(max)), error "too long (max " + max + ")"
        end match
    end loop
    put "done."
end fn
~~~

---

## Appendix A. Operator Precedence

From highest to lowest:

| Operator | Meaning |
|----------|---------|
| . | Field access, method call |
| () [] | Call, index |
| - ~ ! | Negation, bitwise NOT, logical NOT |
| as | Type cast |
| * / % | Multiply, divide, modulo |
| + - | Add, subtract |
| << >> | Shift |
| & | Bitwise AND |
| ^ | Bitwise XOR |
| \| | Bitwise OR |
| =, !=, <, >, <=, >= | Comparison |
| and | Logical AND |
| or | Logical OR |
| := = += -= ... | Assignment |

## Appendix B. Command-Line Usage

~~~
poly file.poly          # compile and run
poly --check file.poly  # type-check only
poly --emit-rust file.poly   # print generated Rust
poly --ir file.poly         # print optimized Rust
poly --tokens file.poly     # print tokens
poly --ast file.poly        # print AST
poly --repl                 # interactive REPL
poly --project dir file.poly  # generate Cargo project
~~~

## Appendix C. Transpilation Reference

| Poly | Rust |
|------|------|
| put x | println!("{}", x); |
| put x | print!("{}", x); |
| error x | eprintln!("[ERROR] {}", x); |
| var x i32 := 42 | let mut x: i32 = 42; |
| let x i32 = 42 | let x: i32 = 42; |
| const X := 42 | const X: i32 = 42; |
| x := y | x = y; |
| x += 1 | x += 1; |
| x := x + 5 | x += 5; |
| x := x + 1 | x += 1; |
| if x, | if x { |
| else if x, | } else if x { |
| else, | } else { |
| end if | } |
| while cond | while cond { |
| loop | loop { |
| loop i 0..10 | for i in 0..=10 { |
| loop x in xs | for x in xs { |
| end loop | } |
| match x | match x { |
| pattern, expr | pattern => expr, |
| end match | } |
| fn name(p: T): T | fn name(p: T) -> T { |
| return x | return x; |
| struct S | struct S { |
| enum E | enum E { |
| impl S | impl S { |
| \|x\| x * 2 | \|x\| x * 2 |
| try expr | expr? |
| assert(cond) | assert!(cond, "assertion failed") |
| put x to "f" | std::fs::write("f", ...).unwrap(); |
| put x to "f" | writeln!(f, ...).unwrap(); |
| get | reads stdin with read_line |
| http_get(url).await | real HTTP/1.1 GET over tokio TCP |
| db_execute(q).await | SQLite query via rusqlite |
