# Poly Language Best Practices Guide

## Overview

This guide covers best practices for using the new Poly I/O and error handling syntax effectively.

---

## 1. Output Best Practices

### Use `-n` for Progress Indicators

```poly
// Good: Progress indicator
put -n "Loading"
loop: 0..10
    put -n "."
    sleep(100)
end loop
put " Done!"

// Bad: Newlines in progress
loop: 0..10
    put "Loading..."  // Creates multiple lines
end loop
```

### Use Appropriate Error Levels

```poly
// Good: Use correct levels
error "File not found: " + path           // Actual errors
warn "Deprecated function used"           // Potential issues
info "Processing item " + item.to_string() // Debug info

// Bad: Wrong levels
info "File not found"     // Should be error
error "Loading..."        // Should be info
```

### Format Output Consistently

```poly
// Good: Consistent formatting
put "Name: " + name
put "Age: " + age.to_string()
put "Email: " + email

// Bad: Inconsistent formatting
put "Name:" + name
put "Age is " + age
put "Email:  " + email
```

---

## 2. Input Best Practices

### Always Provide Prompts

```poly
// Good: Clear prompts
put -n "Enter your name: "
var name: ustring = get

// Bad: No prompt
var name: ustring = get  // User doesn't know what to enter
```

### Use Default Values for Optional Fields

```poly
// Good: Sensible defaults
put -n "Enter color (default: blue): "
var color: ustring = get --default u"blue"

// Bad: No default
put -n "Enter color: "
var color: ustring = get  // Forces user to enter something
```

### Set Timeouts for Interactive Input

```poly
// Good: Prevent hanging
match get --timeout 5000
    Ok(input) => process(input)
    Timeout => warn "Input timeout, using default"
    Error(e) => error "Input error: " + e
end match

// Bad: No timeout
var input: ustring = get  // Can hang forever
```

### Validate Input Immediately

```poly
// Good: Validate early
var age: i32 = get with validate |x| x > 0 && x < 150

// Bad: Validate late
var age: i32 = get
if age < 0 || age > 150 then
    error "Invalid age"
end if
```

### Mask Sensitive Input

```poly
// Good: Mask passwords
put -n "Enter password: "
var password: ustring = get --mask u"*"

// Bad: Expose passwords
put -n "Enter password: "
var password: ustring = get  // Visible on screen
```

---

## 3. Error Handling Best Practices

### Define Specific Error Types

```poly
// Good: Specific error types
enum FileError
    NotFound
    PermissionDenied
    InvalidData
    Unknown(message: ustring)
end enum

// Bad: Generic errors
enum Error
    Error1
    Error2
    Error3
end enum
```

### Use `try` for Error Propagation

```poly
// Good: Propagate errors
fn process_config(): Result<ustring, FileError>
    var content = try read_file(u"config.txt")
    var validated = try validate_config(content)
    return Ok(validated)
end fn

// Bad: Handle every error manually
fn process_config(): Result<ustring, FileError>
    match read_file(u"config.txt")
        Ok(content) =>
            match validate_config(content)
                Ok(validated) => return Ok(validated)
                Error(e) => return Error(e)
            end match
        Error(e) => return Error(e)
    end match
end fn
```

### Use Pattern Matching for Error Handling

```poly
// Good: Pattern matching
match read_file(u"config.txt")
    Ok(content) => process(content)
    Error(FileError::NotFound) => create_default_config()
    Error(FileError::PermissionDenied) => request_permissions()
    Error(e) => error "Unexpected error: " + e
end match

// Bad: If-else chains
var result = read_file(u"config.txt")
if result.is_ok() then
    process(result.unwrap())
else if result.error() == FileError::NotFound then
    create_default_config()
else if result.error() == FileError::PermissionDenied then
    request_permissions()
else
    error "Unexpected error"
end if
```

### Use Wildcard for Catch-All

```poly
// Good: Catch-all for unknown errors
match validate_name(input)
    Ok(name) => put "Valid: " + name
    Error(EmptyInput) => error "Name cannot be empty"
    Error(TooShort(min)) => error "Name too short"
    Error(_) => error "Validation failed"  // Catches any other error
end match

// Bad: Exhaustive matching without catch-all
match validate_name(input)
    Ok(name) => put "Valid: " + name
    Error(EmptyInput) => error "Name cannot be empty"
    Error(TooShort(min)) => error "Name too short"
    // Missing Error(TooLong) and Error(InvalidFormat)
end match
```

---

## 4. Code Organization Best Practices

### Group Related Output

```poly
// Good: Grouped output
put "=== User Registration ==="
put ""
put "Name: " + name
put "Email: " + email
put ""

// Bad: Scattered output
put "Name: " + name
// ... 100 lines of code ...
put "Email: " + email
```

### Use Functions for Complex Logic

```poly
// Good: Function for complex validation
fn validate_user(name: ustring, email: ustring): Result<(ustring, ustring), ValidationError>
    var valid_name = try validate_name(name)
    var valid_email = try validate_email(email)
    return Ok((valid_name, valid_email))
end fn

// Bad: Inline complex logic
var valid_name = if name.len() == 0 then
    error "Empty name"
else if name.len() < 2 then
    error "Name too short"
else
    name
end if
// ... repeat for email ...
```

### Handle Errors at the Right Level

```poly
// Good: Handle errors at appropriate level
fn read_config(): Result<ustring, FileError>
    return try read_file(u"config.txt")  // Propagate to caller
end fn

fn main()
    match read_config()
        Ok(config) => process(config)
        Error(e) => error "Failed to load config: " + e
    end match
end fn

// Bad: Handle errors too early
fn read_config(): Result<ustring, FileError>
    match read_file(u"config.txt")
        Ok(content) => return Ok(content)
        Error(e) =>
            error "Failed to read config"  // Too early
            return Error(e)
    end match
end fn
```

---

## 5. Performance Best Practices

### Use `-n` for Frequent Output

```poly
// Good: Efficient progress updates
loop: 0..1000
    put -n "\rProcessing: " + i.to_string()
end loop
put ""

// Bad: Inefficient output
loop: 0..1000
    put "Processing: " + i.to_string()  // Creates 1000 lines
end loop
```

### Buffer Output When Possible

```poly
// Good: Buffer output
var output: ustring = ""
loop: items
    output = output + item.to_string() + "\n"
end loop
put output

// Bad: Frequent output
loop: items
    put item.to_string()  // Multiple system calls
end loop
```

### Use Appropriate Data Types

```poly
// Good: Use appropriate types
var count: i32 = get --as i32
var price: f64 = get --as f64

// Bad: Wrong types
var count: ustring = get  // Then parse later
var price: ustring = get  // Then convert later
```

---

## 6. Security Best Practices

### Always Mask Sensitive Input

```poly
// Good: Mask passwords
put -n "Enter password: "
var password: ustring = get --mask u"*"

// Bad: Expose passwords
put -n "Enter password: "
var password: ustring = get
```

### Validate All Input

```poly
// Good: Validate everything
var age: i32 = get with validate |x| x > 0 && x < 150
var email: ustring = get with validate |e| e.contains(u"@")

// Bad: Trust input
var age: i32 = get  // Could be negative
var email: ustring = get  // Could be invalid
```

### Use Timeouts for Network Operations

```poly
// Good: Timeout for network
match get --timeout 5000
    Ok(data) => process(data)
    Timeout => warn "Network timeout"
    Error(e) => error "Network error: " + e
end match

// Bad: No timeout
var data: ustring = get  // Can hang forever
```

---

## 7. Testing Best Practices

### Test Error Cases

```poly
// Good: Test all error paths
fn test_validation()
    // Test empty input
    match validate_name(u"")
        Ok(_) => fail("Should not succeed")
        Error(EmptyInput) => pass("Correctly caught empty input")
        Error(_) => fail("Wrong error type")
    end match
    
    // Test short input
    match validate_name(u"a")
        Ok(_) => fail("Should not succeed")
        Error(TooShort(_)) => pass("Correctly caught short input")
        Error(_) => fail("Wrong error type")
    end match
end fn
```

### Test Edge Cases

```poly
// Good: Test edge cases
fn test_boundaries()
    // Test minimum valid age
    var age = validate_age(1)
    assert(age.is_ok())
    
    // Test maximum valid age
    age = validate_age(150)
    assert(age.is_ok())
    
    // Test out of bounds
    age = validate_age(0)
    assert(age.is_error())
    
    age = validate_age(151)
    assert(age.is_error())
end fn
```

---

## Summary

1. **Output**: Use `-n` for progress, appropriate error levels
2. **Input**: Always prompt, use defaults, set timeouts, validate early
3. **Errors**: Define specific types, use `try`, pattern matching
4. **Organization**: Group output, use functions, handle errors at right level
5. **Performance**: Buffer output, use appropriate types
6. **Security**: Mask sensitive input, validate everything, use timeouts
7. **Testing**: Test error cases and edge cases
