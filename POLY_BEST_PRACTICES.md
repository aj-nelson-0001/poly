# Poly Language Best Practices Guide

> **Historical guide:** These practices were written for the v1 language surface. Consult
[POLY_DOCUMENTATION_INDEX.md](POLY_DOCUMENTATION_INDEX.md) for current v2 references.

## Overview

This guide covers best practices for using the new Poly I/O and error handling syntax effectively.

---

## 1. Output Best Practices

### Use Appropriate Output for Progress Indicators

> **Note:** `sleep()` is not a v2 builtin. Use a `#rust` block with Tokio's `tokio::time::sleep` for delays on the Rust target.

~~~poly fragment
// Good: Progress indicator pattern (Rust target with #rust helper)
#rust
use tokio::time::{sleep, Duration};

async fn wait_ms(ms: i32) {
    sleep(Duration::from_millis(ms as u64)).await;
}
#endrust

fn main()
    put "Loading"
    loop i 0..10
        put "."
        wait_ms(100)
    end loop
    put " Done!"
end fn

~~~poly fragment

// Bad: Newlines in progress - creates multiple lines
loop i 0..10
    put "Loading..."  // Creates multiple lines
end loop

~~~















The "Bad" variant above is intentionally a fragment: it is the anti-pattern







being called out, not a runnable program.















### Use Appropriate Error Levels















~~~poly fragment

// Good: Use correct levels
error "File not found: " + path           // Actual errors
warn "Deprecated function used"           // Potential issues
info "Processing item " + item.to_string() // Debug info

// Bad: Wrong levels
info "File not found"     // Should be error
error "Loading..."        // Should be info

~~~




### Format Output Consistently

~~~poly fragment

// Good: Consistent formatting
put "Name: " + name
put "Age: " + age.to_string()
put "Email: " + email

// Bad: Inconsistent formatting
put "Name:" + name
put "Age is " + age
put "Email:  " + email

~~~















---















## 2. Input Best Practices















### Always Provide Prompts















~~~poly

fn main()
    // Good: Clear prompts
    put "Enter your name: "
    var name ustring := get
end fn

~~~poly fragment







// Bad: No prompt







var name ustring := get  // User doesn't know what to enter







~~~

### Use Default Values for Optional Fields

~~~poly
fn main()
    // Good: Sensible defaults
    put "Enter color (default: blue): "
    var color ustring := get --default unicode "blue"
end fn

~~~poly fragment
// Bad: No default
put "Enter color: "
var color ustring := get  // Forces user to enter something
~~~















### Set Timeouts for Interactive Input















~~~poly fragment
// Good: Prevent hanging
match get --timeout 5000
    Ok(input), process(input)
    Timeout, warn "Input timeout, using default"
    Error(e), error "Input error: " + e
end match

// Bad: No timeout
var input ustring := get  // Can hang forever

~~~




### Validate Input Immediately

> **Note:** The `with validate` clause is a parser-only compatibility form and is not part of the maintained runnable v2 API.
Validate input with a loop instead.

~~~poly

fn main()
    // Good: Validate early with a loop
    var age i32 := 0
    var done bool := false
    while not done
        put "Enter age (1-150): "
        var input i32 := get --as i32
        if input >= 1 and input <= 150
            age := input
            done := true
        else
            error "Age must be between 1 and 150"
        end if
    end while
    put age
end fn

~~~poly fragment
// Bad: Validate late
var age i32 := get --as i32
if age < 0 or age > 150,
    error "Invalid age"
end if
~~~

### Mask Sensitive Input

~~~poly







fn main()







    // Good: Mask passwords







    put "Enter password: "







    var password ustring := get --mask unicode "*"







end fn















~~~poly fragment
// Bad: Expose passwords
put "Enter password: "
var password ustring := get  // Visible on screen
~~~




---

## 3. Error Handling Best Practices

### Define Specific Error Types

~~~poly
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
~~~















### Use `try` for Error Propagation















~~~poly fragment
// Good: Propagate errors
fn process_config(): Result<ustring, FileError>
    var content := try read_file(unicode "config.txt")
    var validated := try validate_config(content)
    return Ok(validated)
end fn

// Bad: Handle every error manually
fn process_config(): Result<ustring, FileError>
    match read_file(unicode "config.txt")
        Ok(content),
            match validate_config(content)
                Ok(validated), return Ok(validated)
                Error(e), return Error(e)
            end match
        Error(e), return Error(e)
    end match
end fn

~~~




### Use Pattern Matching for Error Handling

> **Note:** Poly's `Result` type and pattern matching work on the Rust target. Other backends may require foreign helpers.

~~~poly fragment

// Good: Pattern matching
match read_file(unicode "config.txt")
    Ok(content), process(content)
    Error(FileError::NotFound), create_default_config()
    Error(FileError::PermissionDenied), request_permissions()
    Error(e), error "Unexpected error: " + e
end match

// Bad: If-else chains with non-existent methods
var result := read_file(unicode "config.txt")
if result.is_ok(),  // is_ok() is a Rust-target Result method
    process(result.unwrap())
else if result.error() = FileError::NotFound,  // error() is not a v2 method
    create_default_config()
else if result.error() = FileError::PermissionDenied,
    request_permissions()
else
    error "Unexpected error"
end if

~~~















### Use Wildcard for Catch-All















~~~poly fragment

// Good: Catch-all for unknown errors
match validate_name(input)
    Ok(name), put "Valid: " + name
    Error(EmptyInput), error "Name cannot be empty"
    Error(TooShort(min)), error "Name too short"
    Error(_), error "Validation failed"  // Catches any other error
end match

// Bad: Exhaustive matching without catch-all
match validate_name(input)
    Ok(name), put "Valid: " + name
    Error(EmptyInput), error "Name cannot be empty"
    Error(TooShort(min)), error "Name too short"
    // Missing Error(TooLong) and Error(InvalidFormat)
end match

~~~




---

## 4. Code Organization Best Practices

### Group Related Output

~~~poly fragment

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

~~~















### Use Functions for Complex Logic















~~~poly fragment

// Good: Function for complex validation
fn validate_user(name: ustring, email: ustring): Result<(ustring, ustring), ValidationError>
    var valid_name := try validate_name(name)
    var valid_email := try validate_email(email)
    return Ok((valid_name, valid_email))
end fn

// Bad: Inline complex logic
var valid_name := if name.len() == 0,
    error "Empty name"
else if name.len() < 2,
    error "Name too short"
else
    name
end if
// ... repeat for email ...

~~~




### Handle Errors at the Right Level

~~~poly fragment

// Good: Handle errors at appropriate level (propagate to caller)
fn read_config(): Result<ustring, FileError>
    return try read_file(unicode "config.txt")  // `read_file` returns Result
end fn

fn main()
    match read_config()
        Ok(config), process(config)
        Error(e), error "Failed to load config: " + e
    end match
end fn

// Bad: Handle errors too early (log and re-wrap instead)
fn read_config(): Result<ustring, FileError>
    match read_file(unicode "config.txt")
        Ok(content), return Ok(content)
        Error(e),
            error "Failed to read config"  // Too early
            return Error(e)
    end match
end fn

~~~















---















## 5. Performance Best Practices















### Buffer Output When Possible















~~~poly fragment

// Good: Build output and print once
var output ustring := ""
loop item in items
    output := output + item.to_string() + "\n"
end loop
put output

// Bad: Frequent output - multiple system calls

loop item in items

    put item.to_string()  // Multiple system calls

end loop

~~~




### Use Appropriate Data Types

~~~poly

fn main()
    // Good: Use appropriate types
    var count i32 := get --as i32
    var price f64 := get --as f64
    put count
    put price
end fn

~~~poly fragment
// Bad: Wrong types
var count ustring := get  // Then parse later
var price ustring := get  // Then convert later
~~~

---

## 6. Security Best Practices

### Always Mask Sensitive Input

~~~poly







fn main()







    // Good: Mask passwords







    put "Enter password: "







    var password ustring := get --mask unicode "*"







end fn















~~~poly fragment

// Bad: Expose passwords

put "Enter password: "

var password ustring := get  // Visible on screen

~~~




### Validate All Input

> **Note:** The `with validate` clause is a parser-only compatibility form and is not part of the maintained runnable v2 API.
Validate input with loops instead.

~~~poly

fn main()
    // Good: Validate everything with loops
    var age i32 := 0
    var done bool := false
    while not done
        put "Enter age (1-150): "
        var input i32 := get --as i32
        if input >= 1 and input <= 150
            age := input
            done := true
        else
            error "Age must be between 1 and 150"
        end if
    end while

    var email ustring := unicode ""
    done := false
    while not done
        put "Enter email: "
        var input ustring := get
        if input.contains(unicode "@")
            email := input
            done := true
        else
            error "Email must contain @"
        end if
    end while

    put age
    put email
end fn

~~~poly fragment
// Bad: Trust input
var age i32 := get --as i32  // Could be negative
var email ustring := get  // Could be invalid
~~~

### Use Timeouts for Interactive Input

~~~poly fragment







// Good: Timeout for interactive input (Rust target)







match get --timeout 5000







    Ok(input), process(input)







    Timeout, warn "Input timeout, using default"







    Error(e), error "Input error: " + e







end match















// Bad: No timeout - can hang forever on interactive input







var input ustring := get  // Can hang forever







~~~

> **Note:** `--timeout` is a Rust-target feature. Other backends require foreign helpers for timeout behavior.

---

## 7. Testing Best Practices

### Test Error Cases

~~~poly fragment
// Good: Test all error paths (Rust target)
fn test_validation()
    // Test empty input
    match validate_name(unicode "")
        Ok(_), fail("Should not succeed")
        Error(EmptyInput), pass("Correctly caught empty input")
        Error(_), fail("Wrong error type")
    end match

    // Test short input
    match validate_name(unicode "a")
        Ok(_), fail("Should not succeed")
        Error(TooShort(_)), pass("Correctly caught short input")
        Error(_), fail("Wrong error type")
    end match
end fn

> **Note:** `fail()` and `pass()` are test helpers available on the Rust target. `validate_name`, `validate_age` would be
user-defined functions.
~~~

### Test Edge Cases

~~~poly fragment







// Good: Test edge cases (Rust target)







fn test_boundaries()







    // Test minimum valid age







    var age := validate_age(1)







    if age.is_ok()







        pass("Minimum age accepted")







    else







        fail("Minimum age rejected")







    end if















    // Test maximum valid age







    age := validate_age(150)







    if age.is_ok()







        pass("Maximum age accepted")







    else







        fail("Maximum age rejected")







    end if















    // Test out of bounds







    age := validate_age(0)







    if age.is_error()







        pass("Zero age rejected")







    else







        fail("Zero age accepted")







    end if















    age := validate_age(151)







    if age.is_error()







        pass("Over-limit age rejected")







    else







        fail("Over-limit age accepted")







    end if







end fn















> **Note:** `is_ok()` and `is_error()` are Rust-target Result methods. Other backends require foreign helpers.







~~~

---

## Summary

1. **Output**: Use appropriate error levels
2. **Input**: Always prompt, use defaults, set timeouts, validate with loops
3. **Errors**: Define specific types, use `try`, pattern matching
4. **Organization**: Group output, use functions, handle errors at right level
5. **Performance**: Buffer output, use appropriate types
6. **Security**: Mask sensitive input, validate everything, use timeouts
7. **Testing**: Test error cases and edge cases
