# Poly Language Performance Optimization Guide

> **Historical guide:** Performance examples target the v1 Rust-only surface and may use retired syntax. They are not normative for the v2 preview.

## Overview

This guide covers performance optimization techniques for the new Poly I/O and error handling syntax.

---

## 1. Output Optimization

### Buffer Output When Possible

~~~poly fragment
// Good: Buffer output
var output ustring := ""
loop item in items
    output := output + item.to_string() + "\n"
end loop
put output

// Bad: Frequent output
loop item in items
    put item.to_string()  // Multiple system calls
end loop
~~~

### Use String Concatenation Efficiently

~~~poly
// Good: Build string efficiently
var parts Vec<ustring> := []
loop i 0..1000
    parts.push(i.to_string())
end loop
var result ustring := parts.join(", ")
put result

// Bad: String concatenation in loop
var result ustring := ""
loop i 0..1000
    result := result + i.to_string() + ", "// O(n²) complexity
end loop
~~~

### Avoid Unnecessary Formatting

~~~poly fragment
// Good: Simple output
put age.to_string()

// Bad: Unnecessary formatting
put "{age}"  // Slower than direct to_string()
~~~

---

## 2. Input Optimization

### Use Appropriate Data Types

~~~poly
// Good: Use appropriate types
var count i32 := get --as i32
var price f64 := get --as f64

// Bad: Wrong types
var count ustring := get  // Then parse later
var price ustring := get  // Then convert later
~~~

### Validate Early

~~~poly
// Good: Validate immediately
var age i32 := get with validate |x| x > 0 && x < 150

// Bad: Validate late
var age i32 := get
if age < 0 || age > 150,
    error "Invalid age"
    // ... more code before handling
end if
~~~

### Use Timeouts

~~~poly fragment
// Good: Prevent hanging
match get --timeout 5000
    Ok(input), process(input)
    Timeout, warn "Timeout, using default"
    Error(e), error e
end match

// Bad: No timeout
var input ustring := get  // Can hang forever
~~~

### Batch Input Operations

~~~poly fragment
// Good: Batch reads
var lines Vec<ustring> := []
var file := open("data.txt")
while !file.eof()
    lines.push(file.get_line())
end while

// Bad: Frequent reads
var file := open("data.txt")
while !file.eof()
    var line ustring := file.get_line()
    process(line)  // Process each line immediately
end while
~~~

---

## 3. File I/O Optimization

### Use Buffering

~~~poly
// Good: Buffered writes
var buffer Vec<ustring> := []
loop i 0..10000
    buffer.push("Line " + i.to_string())
end loop
put buffer.join("\n") to "output.txt"

// Bad: Unbuffered writes
loop i 0..10000
    put "Line " + i.to_string() >to "output.txt"  // 10000 file operations
end loop
~~~

### Read Files Efficiently

~~~poly
// Good: Read entire file
var content ustring := get from "large_file.txt"
var lines Vec<ustring> := content.split("\n")

// Bad: Read line by line
var file := open("large_file.txt")
while !file.eof()
    var line ustring := file.get_line()
    // Process each line
end while
~~~

### Use Binary Mode When Appropriate

~~~poly
// Good: Binary read for binary files
var data bytes := get from "image.png"

// Bad: Text read for binary files
var data ustring := get from "image.png"  // May corrupt data
~~~

---

## 4. Error Handling Optimization

### Use `try` for Error Propagation

~~~poly fragment
// Good: Propagate errors
fn process(): Result<ustring, Error>
    var data := try read_file("config.txt")
    var validated := try validate(data)
    return Ok(validated)
end fn

// Bad: Handle every error
fn process(): Result<ustring, Error>
    match read_file("config.txt")
        Ok(data),
            match validate(data)
                Ok(validated), return Ok(validated)
                Error(e), return Error(e)
            end match
        Error(e), return Error(e)
    end match
end fn
~~~

### Use Pattern Matching

~~~poly fragment
// Good: Pattern matching
match result
    Ok(value), process(value)
    Error(e), handle_error(e)
end match

// Bad: If-else chains
if result.is_ok(),
    process(result.unwrap())
else
    handle_error(result.error())
end if
~~~

### Avoid Unnecessary Error Creation

~~~poly fragment
// Good: Create errors only when needed
fn validate(input: ustring): Result<ustring, ustring>
    if input.len() = 0,
        return Error(unicode "Empty input")
    end if
    return Ok(input)
end fn

// Bad: Create errors unnecessarily
fn validate(input: ustring): Result<ustring, ustring>
    if input.len() = 0,
        var error ustring := unicode "Empty input"
        return Error(error)
    end if
    return Ok(input)
end fn
~~~

---

## 5. Memory Optimization

### Use References When Possible

~~~poly fragment
// Good: Pass by reference
fn process(data: &ustring)
    // Use data without copying
end fn

// Bad: Pass by value
fn process(data: ustring)
    // Copies data
end fn
~~~

### Reuse Buffers

~~~poly fragment
// Good: Reuse buffer
var buffer Vec<ustring> := []
loop i 0..1000
    buffer.clear()  // Reuse buffer
    buffer.push(i.to_string())
    process(buffer)
end loop

// Bad: Create new buffer each time
loop i 0..1000
    var buffer Vec<ustring> := [i.to_string()]  // New allocation
    process(buffer)
end loop
~~~

### Use Primitive Types

~~~poly fragment
// Good: Use primitive types
var count i32 := 42
var flag bool := true

// Bad: Use wrapper types
var count Box<i32> := Box::new(42)  // Unnecessary boxing
var flag Box<bool> := Box::new(true)
~~~

---

## 6. String Optimization

### Use String Interpolation

~~~poly
// Good: String interpolation
var name ustring := "Alice"
var age i32 := 30
put "Name: {name}, Age: {age}"

// Bad: String concatenation
put "Name: " + name + ", Age: " + age.to_string()
~~~

### Pre-allocate Strings

~~~poly
// Good: Pre-allocate
var result ustring := "".repeat(1000)
// Fill result...

// Bad: Dynamic growth
var result ustring := ""
loop i 0..1000
    result := result + "a"// Multiple reallocations
end loop
~~~

### Use String Views

~~~poly fragment
// Good: Use string views
fn process(data: &ustring)
    // Use data without copying
end fn

// Bad: Copy strings
fn process(data: ustring)
    // Copies data
end fn
~~~

---

## 7. Collection Optimization

### Use Appropriate Collection Types

~~~poly fragment
// Good: Use appropriate types
var list Vec<i32> := [1, 2, 3]  // Dynamic array
var map Map<ustring, i32> := []  // Hash map
var set Set<ustring> := []  // Hash set

// Bad: Wrong types
var list Vec<ustring> := ["1", "2", "3"]  // Strings for numbers
var map Vec<(ustring, i32)> := []  // Vector for map
~~~

### Pre-allocate Collections

~~~poly
// Good: Pre-allocate
var list Vec<i32> := []
list.reserve(1000)  // Pre-allocate space
loop i 0..1000
    list.push(i)
end loop

// Bad: Dynamic growth
var list Vec<i32> := []
loop i 0..1000
    list.push(i)  // Multiple reallocations
end loop
~~~

### Use Iterators

~~~poly
fn main()
    var list Vec<i32> := [1, 2, 3, 4]

    // Good: Use iterator chains
    var sum := list.iter().sum()
    put sum
    var doubled := list.iter().map(|x| x * 2).collect()
    put doubled
    var evens := list.iter().filter(|x| x % 2 == 0).collect()
    put evens

    // Bad: Manual iteration
    var sum2 i32 := 0
    loop item in list
        sum2 := sum2 + item
    end loop
    put sum2
end fn
~~~

---

## 8. Parallel Processing

### Use Parallel Iterators

~~~poly fragment
// Good: Parallel processing
var results Vec<i32> := list.par_iter().map(|x| x * 2).collect()

// Bad: Sequential processing
var results Vec<i32> := []
loop item in list
    results.push(item * 2)
end loop
~~~

### Use Async I/O

~~~poly fragment
// Good: Async file operations
var content := async read_file("large_file.txt")
// Do other work while reading
process_other_data()
// Wait for read to complete
var data := await content

// Bad: Synchronous file operations
var data := read_file("large_file.txt")  // Blocks execution
~~~

---

## 9. Profiling and Benchmarking

### Profile Your Code

~~~poly fragment
// Good: Profile critical sections
var start := time_now()
// Critical code here
var duration := time_now() - start
info "Duration: " + duration.to_string() + "ms"

// Bad: No profiling
// Critical code here
// No idea how long it took
~~~

### Benchmark Different Approaches

~~~poly fragment
// Good: Benchmark approaches
var start1 := time_now()
approach1()
var duration1 := time_now() - start1

var start2 := time_now()
approach2()
var duration2 := time_now() - start2

put "Approach 1: " + duration1.to_string() + "ms"
put "Approach 2: " + duration2.to_string() + "ms"

// Bad: No benchmarking
approach1()
approach2()
// No idea which is faster
~~~

---

## Summary

1. **Output**: Use `-n`, buffer output, avoid unnecessary formatting
2. **Input**: Use appropriate types, validate early, use timeouts
3. **File I/O**: Use buffering, read efficiently, use binary mode
4. **Error Handling**: Use `try`, pattern matching, avoid unnecessary errors
5. **Memory**: Use references, reuse buffers, use primitive types
6. **Strings**: Use interpolation, pre-allocate, use views
7. **Collections**: Use appropriate types, pre-allocate, use iterators
8. **Parallelism**: Use parallel iterators, async I/O
9. **Profiling**: Profile critical sections, benchmark approaches