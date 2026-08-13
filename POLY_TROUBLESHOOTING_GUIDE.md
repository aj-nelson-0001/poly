# Poly Language Troubleshooting Guide

## Overview

This guide covers common issues and solutions for the new Poly I/O and error handling syntax.

**Note:** Some functions used in this guide (like `is_input_available()`, `get_disk_free_space()`, `get_file_permissions()`, `set_file_permissions()`, `file_exists()`) are standard library functions. See the Standard Library section for details.

---

## 1. Output Issues

### Issue: Output Not Showing

**Symptoms:**
~~~poly fragment
put "Hello, World!"  // Nothing appears
~~~

**Solutions:**
~~~poly fragment
// Solution 1: Check for buffering
put "Hello, World!" + "\n"  // Force flush

// Solution 2: Use error output (always shows)
error "Hello, World!"

// Solution 3: Check if output is being captured
var output := capture put "Hello, World!"
info "Output: " + output  // Check captured output
~~~

### Issue: Extra Newlines

**Symptoms:**
~~~poly
put "Line 1"
put "Line 2"
// Output:
// Line 1
//
// Line 2
~~~

**Solutions:**
~~~poly
// Solution 1: Use -n flag
put -n "Line 1"
put "Line 2"
// Output:
// Line 1Line 2

// Solution 2: Manual newline control
put "Line 1\nLine 2"
// Output:
// Line 1
// Line 2
~~~

### Issue: File Output Not Working

**Symptoms:**
~~~poly
put "data" > "output.txt"  // File not created
~~~

**Solutions:**
~~~poly
// Solution 1: Check file permissions
put "data" > "output.txt"
var permissions := get_file_permissions("output.txt")
info "Permissions: " + permissions.to_string()

// Solution 2: Check disk space
var free_space := get_disk_free_space(".")
info "Free space: " + free_space.to_string() + " bytes"

// Solution 3: Use absolute path
put "data" > "/absolute/path/to/output.txt"
~~~

---

## 2. Input Issues

### Issue: Input Not Reading

**Symptoms:**
~~~poly
var input ustring := get  // Program hangs
~~~

**Solutions:**
~~~poly
// Solution 1: Use timeout
match get --timeout 5000
    Ok(input) => process(input)
    Timeout => warn "Input timeout"
    Error(e) => error "Input error: " + e
end match

// Solution 2: Check if input is available
if is_input_available(),
    var input ustring := get
else
    warn "No input available"
end if

// Solution 3: Use default value
var input ustring := get --default unicode "default"
~~~

### Issue: Type Conversion Fails

**Symptoms:**
~~~poly
var age i32 := get  // Input: "abc"
// Error: type conversion failed
~~~

**Solutions:**
~~~poly fragment
// Solution 1: Validate before conversion
var input ustring := get
if input.parse::<i32>().is_ok(),
    var age i32 := input.parse::<i32>().unwrap()
else
    error "Please enter a valid number"
end if

// Solution 2: Use validation closure
var age i32 := get with validate |x| x.parse::<i32>().is_ok()

// Solution 3: Use default value
var age i32 := get --as i32 --default 0
~~~

### Issue: Default Value Not Working

**Symptoms:**
~~~poly
var input ustring := get --default unicode "default"  // Still prompts for input
~~~

**Solutions:**
~~~poly fragment
// Solution 1: Check if input is empty
var input ustring := get
if input.len() == 0,
    input = unicode "default"
end if

// Solution 2: Use validation with default
var input ustring := get with validate |i| i.len() > 0 --default unicode "default"

// Solution 3: Use environment variable
var input ustring := get_env("INPUT") or unicode "default"
~~~

---

## 3. Error Handling Issues

### Issue: Error Not Caught

**Symptoms:**
~~~poly
match result
    Ok(value) => process(value)
    Error(e) => handle_error(e)  // Error not caught
end match
~~~

**Solutions:**
~~~poly
// Solution 1: Check error type
match result
    Ok(value) => process(value)
    Error(FileError::NotFound) => error "File not found"
    Error(FileError::PermissionDenied) => error "Permission denied"
    Error(e) => error "Unknown error: " + e.to_string()
end match

// Solution 2: Use wildcard pattern
match result
    Ok(value) => process(value)
    Error(_) => error "An error occurred"
end match

// Solution 3: Log error details
match result
    Ok(value) => process(value)
    Error(e) =>
        error "Error: " + e.to_string()
        info "Error type: " + type_of(e)
        info "Stack trace: " + get_stack_trace()
end match
~~~

### Issue: Error Propagation Not Working

**Symptoms:**
~~~poly
fn risky_operation(): Result<ustring, ustring>
    var result := try other_operation()  // Error not propagated
    return Ok(result)
end fn
~~~

**Solutions:**
~~~poly fragment
// Solution 1: Check return type
fn risky_operation(): Result<ustring, ustring>  // Must return Result
    var result := try other_operation()
    return Ok(result)
end fn

// Solution 2: Use match instead of try
fn risky_operation(): Result<ustring, ustring>
    match other_operation()
        Ok(result) => return Ok(result)
        Error(e) => return Error(e)
    end match
end fn

// Solution 3: Explicit error handling
fn risky_operation(): Result<ustring, ustring>
    var result := other_operation()
    if result.is_err(),
        return Error(result.error())
    end if
    return Ok(result.unwrap())
end fn
~~~

### Issue: Panic on Unwrap

**Symptoms:**
~~~poly
var value := result.unwrap()  // Panics if error
~~~

**Solutions:**
~~~poly fragment
// Solution 1: Use match
match result
    Ok(value) => process(value)
    Error(e) => handle_error(e)
end match

// Solution 2: Use unwrap_or
var value := result.unwrap_or(default_value)

// Solution 3: Use unwrap_or_else
var value := result.unwrap_or_else(|| compute_default())
~~~

---

## 4. File Operation Issues

### Issue: File Not Found

**Symptoms:**
~~~poly
var content ustring := get < "file.txt"  // Error: file not found
~~~

**Solutions:**
~~~poly
// Solution 1: Check if file exists
if file_exists("file.txt"),
    var content ustring := get < "file.txt"
else
    error "File not found"
end if

// Solution 2: Use error handling
match get < "file.txt"
    Ok(content) => process(content)
    Error(e) => error "File error: " + e
end match

// Solution 3: Use default value
var content ustring := get < "file.txt" --default unicode ""
~~~

### Issue: Permission Denied

**Symptoms:**
~~~poly
put "data" > "output.txt"  // Error: permission denied
~~~

**Solutions:**
~~~poly
// Solution 1: Check permissions
var permissions := get_file_permissions("output.txt")
info "Permissions: " + permissions.to_string()

// Solution 2: Change permissions
set_file_permissions("output.txt", 0o644)

// Solution 3: Use different file location
put "data" > "/tmp/output.txt"  // Use temp directory
~~~

### Issue: File Too Large

**Symptoms:**
~~~poly
var content ustring := get < "large_file.txt"  // Error: out of memory
~~~

**Solutions:**
~~~poly
// Solution 1: Read in chunks
var file := open("large_file.txt")
while !file.eof()
    var chunk ustring := file.read_chunk(1024)
    process(chunk)
end while

// Solution 2: Use streaming
var stream := open_stream("large_file.txt")
loop: stream
    process(line)
end loop

// Solution 3: Use binary mode for large files
var data bytes := get < "large_file.bin" --bytes 1024
~~~

---

## 5. Performance Issues

### Issue: Slow Output

**Symptoms:**
~~~poly
loop: 0..10000
    put "Line " + i.to_string()  // Very slow
end loop
~~~

**Solutions:**
~~~poly
// Solution 1: Buffer output
var buffer Vec<ustring> := []
loop: 0..10000
    buffer.push("Line " + i.to_string())
end loop
put buffer.join("\n")

// Solution 2: Use -n for progress
loop: 0..10000
    put -n "\rProgress: " + i.to_string()
end loop
put ""

// Solution 3: Write to file
loop: 0..10000
    put "Line " + i.to_string() >> "output.txt"
end loop
~~~

### Issue: Slow Input

**Symptoms:**
~~~poly
var input ustring := get  // Very slow
~~~

**Solutions:**
~~~poly
// Solution 1: Use timeout
match get --timeout 5000
    Ok(input) => process(input)
    Timeout => warn "Timeout"
    Error(e) => error e
end match

// Solution 2: Use default value
var input ustring := get --default unicode ""

// Solution 3: Use validation
var input ustring := get with validate |i| i.len() > 0
~~~

### Issue: Memory Usage

**Symptoms:**
~~~poly
var large_string ustring := "a".repeat(1000000)  // High memory usage
~~~

**Solutions:**
~~~poly
// Solution 1: Use streaming
var stream := open_stream("large_file.txt")
loop: stream
    process(line)  // Process line by line
end loop

// Solution 2: Use chunks
var chunks := large_string.chunks(1024)
loop: chunks
    process(chunk)
end loop

// Solution 3: Use generators
fn generate_data(): Iterator<ustring>
    loop: 0..1000000
        yield "Line " + i.to_string()
    end loop
end fn

loop: generate_data()
    process(line)
end loop
~~~

---

## 6. Network Issues

### Issue: Connection Timeout

**Symptoms:**
~~~poly
var response := get < "https://api.example.com"  // Timeout
~~~

**Solutions:**
~~~poly fragment
// Solution 1: Use timeout
match get --timeout 5000 < "https://api.example.com"
    Ok(response) => process(response)
    Timeout => warn "Connection timeout"
    Error(e) => error "Connection error: " + e
end match

// Solution 2: Use retry logic
var response := retry(3, || get < "https://api.example.com")

// Solution 3: Use async
var response := async get < "https://api.example.com"
// Do other work
var result := await response
~~~

### Issue: SSL Certificate Error

**Symptoms:**
~~~poly
var response := get < "https://api.example.com"  // SSL error
~~~

**Solutions:**
~~~poly
// Solution 1: Verify certificate
var response := get < "https://api.example.com" with verify_certificate(true)

// Solution 2: Use trusted CA
var response := get < "https://api.example.com" with ca_certificate("ca.pem")

// Solution 3: Skip verification (insecure)
var response := get < "https://api.example.com" with verify_certificate(false)  // Not recommended
~~~

---

## 7. Debugging Tips

### Enable Debug Mode

~~~poly
// Set debug environment variable
set_env("DEBUG", "true")

// Check debug mode
var debug_mode bool := get_env("DEBUG") or unicode "false" == unicode "true"
if debug_mode,
    info "Debug mode enabled"
    // Debug output
end if
~~~

### Use Logging

~~~poly
// Log function entry/exit
fn process_data()
    info "Entering process_data"
    // ... function body
    info "Exiting process_data"
end fn

// Log variable values
var x i32 := 42
info "x = " + x.to_string()

// Log function results
var result := risky_operation()
match result
    Ok(value) => info "Success: " + value.to_string()
    Error(e) => error "Error: " + e.to_string()
end match
~~~

### Use Breakpoints

~~~poly
// Pseudo-code for debugging
fn complex_function()
    // ... code before breakpoint
    debug_break()  // Pause execution
    // ... code after breakpoint
end fn
~~~

---

## Summary

1. **Output Issues**: Check buffering, newlines, file permissions
2. **Input Issues**: Use timeouts, validation, default values
3. **Error Handling**: Use match, check error types, log details
4. **File Operations**: Check existence, permissions, file size
5. **Performance**: Buffer output, use streaming, optimize memory
6. **Network Issues**: Use timeouts, retry logic, async
7. **Debugging**: Enable debug mode, use logging, breakpoints
