# Poly Language Migration Guide: Old to New Syntax

## Overview

This guide helps you migrate from the old Poly syntax to the new syntax with flags and improved readability.

---

## Output Commands

### Old Syntax
```poly
put u\"Hello, World!\"           # Output with newline
putn u\"Enter value: \"          # Output without newline
pute u\"Error message\"          # Output to stderr
```

### New Syntax
```poly
put \"Hello, World!\"            # Output with newline
put -n \"Enter value: \"         # Output without newline
error \"Error message\"          # Error message to stderr
warn \"Warning message\"         # Warning message to stderr
info \"Debug information\"       # Debug info to stderr
```

### Changes Summary
| Old | New | Description |
|-----|-----|-------------|
| `putn` | `put -n` | Use `-n` flag for no newline |
| `pute` | `error`/`warn`/`info` | Use separate commands for different output levels |
| `u\"...\"` | `\"...\"` | Unicode strings are now auto-detected |

---

## Input Commands

### Old Syntax
```poly
var x: ustring = get with timeout 5000
var x: i32 = get with validate |x| x > 0
var x: ustring = get with default u\"value\"
var x: ustring = get with mask u\"*\"
var x: ustring = get with complete [u\"a\", u\"b\"]
var x: Person = get as Person
var x: ustring = get until u\",\" 
```

### New Syntax
```poly
var x: ustring = get --timeout 5000
var x: i32 = get with validate |x| x > 0
var x: ustring = get --default u"value"
var x: ustring = get --mask u"*"
var x: ustring = get with complete [u"a", u"b"]
var x: Person = get --as Person
var x: ustring = get --until u","
var x: bytes = get < "file" --bytes 8
```

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
```poly
enum Result<T, E>
    Ok(T)
    Err(E)
end enum

match result
    Ok(value) => process(value)
    Err(e) => handle_error(e)
end match
```

### New Syntax
```poly
enum Result<T, E>
    Ok(T)
    Error(E)
end enum

match result
    Ok(value) => process(value)
    Error(e) => handle_error(e)
end match
```

### Changes Summary
| Old | New | Description |
|-----|-----|-------------|
| `Err(e)` | `Error(e)` | Use full word for better readability |

---

## String Literals

### Old Syntax
```poly
var name: ustring = u\"Alice\"
put u\"Hello, \" + name
var greeting: ustring = u\"你好\"
```

### New Syntax
```poly
var name: ustring = u\"Alice\"   # Still works
put \"Hello, \" + name           # Auto-detected as Unicode
var greeting: ustring = u\"你好\"  # Explicit Unicode still works
```

### Changes Summary
| Old | New | Description |
|-----|-----|-------------|
| `u\"...\"` required for Unicode | Auto-detected | Unicode is now inferred from content |
| `u\"...\"` still works | Optional | Explicit Unicode annotation still supported |

---

## Complete Migration Examples

### Example 1: Interactive Menu

**Old Syntax:**
```poly
loop
    put u\"Menu:\"
    put u\"1. Start\"
    put u\"2. Stop\"
    put u\"3. Exit\"
    putn u\"Choose: \"
    var choice: i32 = get
    match choice
        1 => start_process()
        2 => stop_process()
        3 => break
        _ => put u\"Invalid choice\"
    end match
end loop
```

**New Syntax:**
```poly
loop
    put \"Menu:\"
    put \"1. Start\"
    put \"2. Stop\"
    put \"3. Exit\"
    put -n \"Choose: \"
    var choice: i32 = get
    match choice
        1 => start_process()
        2 => stop_process()
        3 => break
        _ => put \"Invalid choice\"
    end match
end loop
```

### Example 2: Input Validation

**Old Syntax:**
```poly
put u\"Enter age: \"
var age: i32 = get with validate |x| x > 0 && x < 150
put u\"Age: \" + age

put u\"Enter color: \"
var color: ustring = get with default u\"blue\"
put u\"Color: \" + color
```

**New Syntax:**
```poly
put -n \"Enter age: \"
var age: i32 = get with validate |x| x > 0 && x < 150
put \"Age: \" + age

put -n \"Enter color: \"
var color: ustring = get --default u\"blue\"
put \"Color: \" + color
```

### Example 3: Error Handling

**Old Syntax:**
```poly
fn read_file(path: ustring): Result<ustring, FileError>
    var file = try open_file(path)
    return Ok(file.read_all())
end fn

match read_file(u\"config.txt\")
    Ok(content) => process(content)
    Err(e) => print_err(u\"Error: \" + e)
end match
```

**New Syntax:**
```poly
fn read_file(path: ustring): Result<ustring, FileError>
    var file = try open_file(path)
    return Ok(file.read_all())
end fn

match read_file(u\"config.txt\")
    Ok(content) => process(content)
    Error(e) => error \"Error: \" + e
end match
```

### Example 4: File Operations

**Old Syntax:**
```poly
put u\"Line 1\\nLine 2\" > \"output.txt\"
put u\"Appended line\" >> \"output.txt\"

var content: ustring = get < \"input.txt\"
var data: bytes = get < \"binary.bin\"
```

**New Syntax:**
```poly
put \"Line 1\\nLine 2\" > \"output.txt\"
put \"Appended line\" >> \"output.txt\"

var content: ustring = get < \"input.txt\"
var data: bytes = get < \"binary.bin\"
```

---

## Loop Syntax (New)

### Old Syntax (if applicable)
```poly
for i in 0..10
    put i
end for

for item in items
    put item
end for
```

### New Syntax
```poly
# Simple range
loop: 0..10
    put i
end loop

# Multiple ranges and values (SuperBASIC-inspired)
loop: 1..3, 7, 19..21
    put i  # Iterates: 1, 2, 3, 7, 19, 20, 21
end loop

# With step
loop: 0..10 step 2
    put i  # Iterates: 0, 2, 4, 6, 8
end loop

# Negative step (counting down)
loop: 10..1 step -1
    put i  # Iterates: 10, 9, 8, ..., 1
end loop

# Iterate over collection
loop: items
    put item
end loop

# Iterate with index
loop: (index, item) in items.enumerate()
    put index.to_string() + ": " + item
end loop
```

### Changes Summary
| Old | New | Description |
|-----|-----|-------------|
| `for i in 0..10` | `loop: 0..10` | Use `loop:` with colon for ranges |
| `for item in items` | `loop: items` | Iterate over collections |
| `end for` | `end loop` | Closing keyword changed |
| N/A | `loop: 1..3, 7, 19..21` | New: Multiple ranges and values |
| N/A | `step` | New: Step support for increments |

**Note:** The infinite `loop` (without colon) remains unchanged.

---

## Quick Reference

### Output
| Old | New |
|-----|-----|
| `put u\"text\"` | `put \"text\"` |
| `putn u\"text\"` | `put -n \"text\"` |
| `pute u\"text\"` | `error \"text\"` |

### Input
| Old | New |
|-----|-----|
| `get with timeout 5000` | `get --timeout 5000` |
| `get with default u\"val\"` | `get --default u\"val\"` |
| `get with mask u\"*\"` | `get --mask u\"*\"` |
| `get as Type` | `get --as Type` |
| `get until u\",\"` | `get --until u\",\"` |

### Error Handling
| Old | New |
|-----|-----|
| `Err(e)` | `Error(e)` |
| `print_err(u\"msg\")` | `error \"msg\"` |

---

## Benefits of New Syntax

1. **More Readable**: `Error(e)` is clearer than `Err(e)`
2. **Consistent Flags**: `--timeout`, `--default`, `--mask` follow CLI conventions
3. **Unicode Inference**: No need for `u\"...\"` prefix in most cases
4. **Better Error Levels**: `error`, `warn`, `info` for different severity levels
5. **Simpler Flags**: `-n` for no newline is intuitive

---

## Migration Checklist

- [ ] Replace `putn` with `put -n`
- [ ] Replace `pute` with `error`, `warn`, or `info`
- [ ] Replace `with timeout` with `--timeout`
- [ ] Replace `with default` with `--default`
- [ ] Replace `with mask` with `--mask`
- [ ] Replace `as Type` with `--as Type`
- [ ] Replace `until` with `--until`
- [ ] Replace `Err(e)` with `Error(e)`
- [ ] Remove unnecessary `u\"...\"` prefixes (optional)
- [ ] Update transpilation tables if needed
