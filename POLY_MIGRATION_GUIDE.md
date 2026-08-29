# Poly Language Migration Guide: Old to New Syntax

> **Historical migration reference:** Use [POLY_MIGRATION_GUIDE_v2.md](POLY_MIGRATION_GUIDE_v2.md) for the current preview. The examples below describe earlier syntax transitions.

## Overview

This guide helps you migrate from the old Poly syntax to the new syntax with flags and improved readability.

---

## Output Commands

### Old Syntax
~~~poly fragment
put unicode "Hello, World!"           # Output with newline
putn unicode "Enter value: "          # Output without newline
pute unicode "Error message"          # Output to stderr
~~~

### New Syntax
~~~poly fragment
put "Hello, World!"            # Output with newline
put "Enter value: "         # Output without newline
error "Error message"          # Error message to stderr
warn "Warning message"         # Warning message to stderr
info "Debug information"       # Debug info to stderr
~~~

### Changes Summary
| Old | New | Description |
|-----|-----|-------------|
| `putn` | `put` | Standard output with newline |
| `pute` | `error`/`warn`/`info` | Use separate commands for different output levels |
| `unicode "..."` | `"..."` | Unicode strings are now auto-detected |

---

## Input Commands

### Old Syntax
~~~poly fragment
var x ustring := get with timeout 5000
var x i32 := get with validate |x| x > 0
var x ustring := get with default unicode "value"
var x ustring := get with mask unicode "*"
var x ustring := get with complete [unicode "a", unicode "b"]
var x i32 := get as i32
var x ustring := get until unicode ","
~~~

### New Syntax
~~~poly
var x := get --timeout 5000             # -> Result
var x i32 := get with validate |x| x > 0
var x ustring := get --default unicode "value"
var x ustring := get --mask unicode "*"
var x ustring := get with complete [unicode "a", unicode "b"]
var x i32 := get --as i32
var x ustring := get --until unicode ","
var x bytes := get from "file" --bytes 8
~~~

### Changes Summary
| Old | New | Description |
|-----|-----|-------------|
| `with timeout` | `--timeout` | Use flag for timeout |
| `with default` | `--default` | Use flag for default value |
| `with mask` | `--mask` | Use flag for input mask |
| `as Type` | `--as Type` | Use flag for type conversion |
| `until` | `--until` | Use flag for delimiter |
| `with bytes` | `--bytes` | Use flag for byte count |
| `with validate` | `with validate` | Keep `with` for complex options |
| `with complete` | `with complete` | Keep `with` for complex options |

---

## Error Handling

### Old Syntax
~~~poly fragment
enum Result<T, E>
    Ok(T)
    Err(E)
end enum

match result
    Ok(value), process(value)
    Err(e), handle_error(e)
end match
~~~

### New Syntax
~~~poly fragment
enum Result<T, E>
    Ok(T)
    Error(E)
end enum

match result
    Ok(value), process(value)
    Error(e), handle_error(e)
end match
~~~

### Changes Summary
| Old | New | Description |
|-----|-----|-------------|
| `Err(e)` | `Error(e)` | Use full word for better readability |

---

## String Literals

### Old Syntax
~~~poly fragment
var name ustring := unicode "Alice"
put unicode "Hello, " + name
var greeting ustring := unicode "你好"
~~~

### New Syntax
~~~poly fragment
var name ustring := unicode "Alice"
put unicode "Hello, " + name
var greeting ustring := unicode "你好"
~~~

### Changes Summary
| Old | New | Description |
|-----|-----|-------------|
| `unicode "..."` | `unicode "..."` | The old prefix is replaced by an explicit keyword |
| Ordinary `"..."` | Ordinary `"..."` | Plain strings remain available for normal text |

---

## Complete Migration Examples

### Example 1: Interactive Menu

**Old Syntax:**
~~~poly fragment
loop
    put unicode "Menu:"
    put unicode "1. Start"
    put unicode "2. Stop"
    put unicode "3. Exit"
    putn unicode "Choose: "
    var choice i32 := get
    match choice
        1, start_process()
        2, stop_process()
        3, break
        _, put unicode "Invalid choice"
    end match
end loop
~~~

**New Syntax:**
~~~poly fragment
loop
    put "Menu:"
    put "1. Start"
    put "2. Stop"
    put "3. Exit"
    put "Choose: "
    var choice i32 := get
    match choice
        1, start_process()
        2, stop_process()
        3, break
        _, put "Invalid choice"
    end match
end loop
~~~

### Example 2: Input Validation

**Old Syntax:**
~~~poly fragment
put unicode "Enter age: "
var age i32 := get with validate |x| x > 0 && x < 150
put unicode "Age: " + age

put unicode "Enter color: "
var color ustring := get with default unicode "blue"
put unicode "Color: " + color
~~~

**New Syntax:**
~~~poly fragment
put "Enter age: "
var age i32 := get with validate |x| x > 0 && x < 150
put "Age: " + age

put "Enter color: "
var color ustring := get --default unicode "blue"
put "Color: " + color
~~~

### Example 3: Error Handling

**Old Syntax:**
~~~poly fragment
fn read_file(path: ustring): Result<ustring, FileError>
    var file := try open_file(path)
    return Ok(file.read_all())
end fn

match read_file(unicode "config.txt")
    Ok(content), process(content)
    Err(e), print_err(unicode "Error: " + e)
end match
~~~

**New Syntax:**
~~~poly fragment
fn read_file(path: ustring): Result<ustring, FileError>
    var file := try open_file(path)
    return Ok(file.read_all())
end fn

match read_file(unicode "config.txt")
    Ok(content), process(content)
    Error(e), error "Error: " + e
end match
~~~

### Example 4: File Operations

**Old Syntax:**
~~~poly fragment
put unicode "Line 1\nLine 2" to "output.txt"
put unicode "Appended line" >to "output.txt"

var content ustring := get from "input.txt"
var data bytes := get from "binary.bin"
~~~

**New Syntax:**
~~~poly fragment
put "Line 1\nLine 2" to "output.txt"
put "Appended line" to "output.txt" -append

var content ustring := get from "input.txt"
var data bytes := get from "binary.bin"
~~~

---

## Loop Syntax (New)

### Old Syntax (if applicable)
~~~poly fragment
for i in 0..10
    put i
end for

for item in items
    put item
end for
~~~

### New Syntax
~~~poly fragment
# Simple range
loop i 0..10
    put i
end loop

# Multiple ranges and values (SuperBASIC-inspired)
loop i 1..3, 7, 19..21
    put i  # Iterates: 1, 2, 3, 7, 19, 20, 21
end loop

# With step
loop i 0..10 step 2
    put i  # Iterates: 0, 2, 4, 6, 8, 10
end loop

# Negative step (counting down)
loop i 10..1 step -1
    put i  # Iterates: 10, 9, 8, ..., 1
end loop

# Iterate over collection
loop item in items
    put item
end loop

# Iterate with index
loop (index, item) in items.enumerate()
    put index.to_string() + ": " + item
end loop
~~~

### Changes Summary
| Old | New | Description |
|-----|-----|-------------|
| `for i in 0..10` | `loop i 0..10` | Name the loop variable after `loop`; Poly loop ranges include the end |
| `for item in items` | `loop item in items` | Iterate over collections |
| `end for` | `end loop` | Closing keyword changed |
| N/A | `loop i 1..3, 7, 19..21` | New: Multiple ranges and values |
| N/A | `step` | New: Step support for increments |

**Note:** The infinite `loop` (without colon) remains unchanged.

---

## Quick Reference

### Output
| Old | New |
|-----|-----|
| `put unicode "text"` | `put "text"` |
| `putn unicode "text"` | `put "text"` |
| `pute unicode "text"` | `error "text"` |

### Input
| Old | New |
|-----|-----|
| `get with timeout 5000` | `get --timeout 5000` |
| `get with default unicode "val"` | `get --default unicode "val"` |
| `get with mask unicode "*"` | `get --mask unicode "*"` |
| `get as Type` | `get --as Type` |
| `get until unicode ","` | `get --until unicode ","` |

### Error Handling
| Old | New |
|-----|-----|
| `Err(e)` | `Error(e)` |
| `print_err(unicode "msg")` | `error "msg"` |

---

## Benefits of New Syntax

1. **More Readable**: `Error(e)` is clearer than `Err(e)`
2. **Consistent Flags**: `--timeout`, `--default`, `--mask` follow CLI conventions
3. **Unicode Inference**: No need for `unicode "..."` prefix in most cases
4. **Better Error Levels**: `error`, `warn`, `info` for different severity levels
5. **Simpler Flags**: `-n` for no newline is intuitive

---

## Migration Checklist

- [ ] Replace `putn` with `put`
- [ ] Replace `pute` with `error`, `warn`, or `info`
- [ ] Replace `with timeout` with `--timeout`
- [ ] Replace `with default` with `--default`
- [ ] Replace `with mask` with `--mask`
- [ ] Replace `as Type` with `--as Type`
- [ ] Replace `until` with `--until`
- [ ] Replace `Err(e)` with `Error(e)`
- [ ] Remove unnecessary `unicode "..."` prefixes (optional)
- [ ] Update transpilation tables if needed
