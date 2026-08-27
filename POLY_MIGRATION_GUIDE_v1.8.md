# Poly v1.8.0 Migration Guide

This guide helps you update code written in Poly v1.7.x (and earlier) to work with v1.8.0. The changes simplify the syntax by removing SuperBASIC and assembly-style commands that were hard to read.

## Quick Reference

| Old Syntax | New Syntax | Purpose |
|---|---|---|
| `loop: i 0..10` | `loop i 0..10` | Range loop (remove colon) |
| `loop: item in xs` | `loop item in xs` | Collection loop (remove colon) |
| `add x, 5` | `x := x + 5` | Addition assignment |
| `sub x, 3` | `x := x - 3` | Subtraction assignment |
| `inc x` | `x := x + 1` | Increment |
| `dec x` | `x := x - 1` | Decrement |
| `set x to y` | `x := y` | Assignment |
| `x += 5` | `x := x + 5` | Compound add |
| `x -= 3` | `x := x - 3` | Compound subtract |
| `put "hello" > "file.txt"` | `put "hello" to "file.txt"` | Write to file |
| `put "hello" >> "file.txt"` | `put "hello" to "file.txt" -append` | Append to file |
| `get < "file.txt"` | `get from "file.txt"` | Read from file |

## Detailed Changes

### 1. Loop Syntax (remove colon)

**Before:**
~~~poly
loop: i 0..10
    put i
end loop

loop: item in fruits
    put item
end loop
~~~

**After:**
~~~poly
loop i 0..10
    put i
end loop

loop item in fruits
    put item
end loop
~~~

The infinite loop (`loop ... end loop`) is unchanged.

### 2. Remove `add`, `sub`, `inc`, `dec`

These assembly-style mutation commands have been removed. Use the `:=` assignment operator with arithmetic expressions instead.

**Before:**
~~~poly
var count := 0
add count, 1        # count = 1
inc count            # count = 2
sub count, 1        # count = 1
dec count            # count = 0
~~~

**After:**
~~~poly
var count := 0
count := count + 1   # count = 1
count := count + 1   # count = 2
count := count - 1   # count = 1
count := count - 1   # count = 0
~~~

### 3. Remove `set ... to`

The `set x to y` form has been removed. Use `x := y` instead.

**Before:**
~~~poly
set name to "Alice"
set age to 30
set result to a + b
~~~

**After:**
~~~poly
var name := "Alice"
var age := 30
var result := a + b
~~~

Or if the variable already exists:
~~~poly
name := "Alice"
age := 30
result := a + b
~~~

### 4. Remove Compound Mutation Operators

The `+=`, `-=`, `*=`, `/=` operators have been removed. Use `:=` with the full expression.

**Before:**
~~~poly
x += 5
x -= 3
x *= 2
x /= 4
~~~

**After:**
~~~poly
x := x + 5
x := x - 3
x := x * 2
x := x / 4
~~~

### 5. File I/O Operators

Replace `>` / `>>` / `<` with `to` / `to ... -append` / `from`.

**Before:**
~~~poly
put "Hello" > "output.txt"      # Write
put "World" >> "output.txt"     # Append
var content := get < "input.txt"  # Read
~~~

**After:**
~~~poly
put "Hello" to "output.txt"        # Write
put "World" to "output.txt" -append # Append
var content := get from "input.txt" # Read
~~~

### 6. Assignment Operator

The `=` operator is now **only** for equality checking. Assignment **must** use `:=`.

**Before (ambiguous):**
~~~poly
x = 5          # Was this assignment or comparison?
~~~

**After (unambiguous):**
~~~poly
x := 5         # Assignment
if x = 5,      # Equality check
    put "equal"
end if
~~~

## Migration Tips

1. **Use the compiler's hints.** When you run old syntax, the compiler will show a 💡 suggestion with the correct new syntax.

2. **Find and replace patterns:**
   - Search for `loop:` and replace with `loop `
   - Search for `set ` followed by ` to ` and replace with `:= `
   - Search for `add `, `sub `, `inc `, `dec ` and rewrite as `:=` expressions
   - Search for `> "` after `put` and replace with `to "`
   - Search for `>> "` after `put` and replace with `to "..." -append`
   - Search for `< "` after `get` and replace with `from "`

3. **Test your code.** After updating, run `poly --check your_file.poly` to verify syntax and generated-target compilation.

## What Stayed the Same

These features are unchanged and still work as before:

- `put` / `get` for I/O (stdin/stdout)
- `var x := value` for variable declaration
- `if` / `else if` / `else` / `end if`
- `fn name(params): Type` for functions
- `match` / `end match` for pattern matching
- `struct` / `enum` / `impl` for types
- `for x in collection` for iteration
- `loop` / `end loop` for infinite loops
- `while` / `end while` for conditional loops
- `try` for error propagation
- `:=` for assignment (was already the primary syntax)
- `=` for equality checking (now the only use of `=`)
