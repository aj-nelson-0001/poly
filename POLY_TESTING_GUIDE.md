# Poly Language Testing Guide

## Overview

This guide covers testing strategies and best practices for the new Poly I/O and error handling syntax.

**Note:** Some functions used in this guide (like `mock_input()`, `mock_timeout()`, `time_now()`, `delete_file()`) are test utilities or standard library functions. See the Test Utilities section for details.

---

## 1. Unit Testing Output Commands

### Testing `put` Output

```poly
// Test basic output
fn test_basic_output()
    var output = capture put "Hello, World!"
    assert(output == "Hello, World!")
end fn

// Test output without newline
fn test_put_n()
    var output = capture put -n "No newline"
    assert(output == "No newline")
    assert(not output.ends_with("\n"))
end fn

// Test error output
fn test_error_output()
    var output = capture error "Error message"
    assert(output == "[ERROR] Error message")
end fn

// Test warning output
fn test_warn_output()
    var output = capture warn "Warning message"
    assert(output == "[WARN] Warning message")
end fn

// Test debug output
fn test_info_output()
    var output = capture info "Debug info"
    assert(output == "[INFO] Debug info")
end fn
```

### Testing File Output

```poly
// Test file write
fn test_file_write()
    put "Test content" > "test_output.txt"
    var content: ustring = get < "test_output.txt"
    assert(content == "Test content")
    delete_file("test_output.txt")
end fn

// Test file append
fn test_file_append()
    put "Line 1" > "test_append.txt"
    put "Line 2" >> "test_append.txt"
    var content: ustring = get < "test_append.txt"
    assert(content == "Line 1\nLine 2")
    delete_file("test_append.txt")
end fn
```

---

## 2. Unit Testing Input Commands

### Testing Basic Input

```poly
// Test typed input
fn test_typed_input()
    var input = mock_input("42")
    var age: i32 = get
    assert(age == 42)
end fn

// Test string input
fn test_string_input()
    var input = mock_input("Hello")
    var name: ustring = get
    assert(name == "Hello")
end fn

// Test boolean input
fn test_boolean_input()
    var input = mock_input("true")
    var flag: bool = get
    assert(flag == true)
end fn
```

### Testing Input Flags

```poly
// Test default value
fn test_default_value()
    var input = mock_input("")  // Empty input
    var color: ustring = get --default u"blue"
    assert(color == "blue")
end fn

// Test timeout
fn test_timeout()
    var input = mock_timeout(100)  // Timeout after 100ms
    match get --timeout 50
        Ok(_) => fail("Should have timed out")
        Timeout => pass("Correctly timed out")
        Error(e) => fail("Unexpected error: " + e)
    end match
end fn

// Test validation
fn test_validation()
    var input = mock_input("150")
    var age: i32 = get with validate |x| x >= 1 && x <= 150
    assert(age == 150)
end fn

// Test validation failure
fn test_validation_failure()
    var input = mock_input("200")
    match get with validate |x| x >= 1 && x <= 150
        Ok(_) => fail("Should have failed validation")
        Error(_) => pass("Correctly failed validation")
    end match
end fn
```

### Testing File Input

```poly
// Test file read
fn test_file_read()
    put "Test content" > "test_input.txt"
    var content: ustring = get < "test_input.txt"
    assert(content == "Test content")
    delete_file("test_input.txt")
end fn

// Test binary read
fn test_binary_read()
    var data: bytes = [0x48, 0x65, 0x6C, 0x6C, 0x6F]
    data > "test_binary.bin"
    var binary: bytes = get < "test_binary.bin"
    assert(binary.len() == 5)
    delete_file("test_binary.bin")
end fn

// Test bytes read
fn test_bytes_read()
    put "Hello, World!" > "test_bytes.txt"
    var first_five: bytes = get < "test_bytes.txt" --bytes 5
    assert(first_five.len() == 5)
    delete_file("test_bytes.txt")
end fn
```

---

## 3. Unit Testing Error Handling

### Testing Result Type

```poly
// Test Ok value
fn test_ok_value()
    var result: Result<i32, ustring> = Ok(42)
    match result
        Ok(value) => assert(value == 42)
        Error(_) => fail("Should be Ok")
    end match
end fn

// Test Error value
fn test_error_value()
    var result: Result<i32, ustring> = Error(u"Something went wrong")
    match result
        Ok(_) => fail("Should be Error")
        Error(e) => assert(e == u"Something went wrong")
    end match
end fn
```

### Testing Error Propagation

```poly
// Test try propagation
fn risky_operation(): Result<ustring, ustring>
    return Error(u"Risky error")
end fn

fn test_error_propagation()
    match risky_operation()
        Ok(_) => fail("Should have failed")
        Error(e) => assert(e == u"Risky error")
    end match
end fn

// Test try unwrapping
fn safe_operation(): Result<ustring, ustring>
    return Ok(u"Safe result")
end fn

fn test_try_unwrap()
    var result = try safe_operation()
    assert(result == u"Safe result")
end fn
```

### Testing Pattern Matching

```poly
// Test specific error patterns
enum TestError
    NotFound
    PermissionDenied
    InvalidData(message: ustring)
end enum

fn test_specific_patterns()
    var result: Result<ustring, TestError> = Error(TestError::NotFound)
    match result
        Ok(_) => fail("Should be Error")
        Error(NotFound) => pass("Correctly matched NotFound")
        Error(_) => fail("Wrong error type")
    end match
end fn

// Test wildcard pattern
fn test_wildcard_pattern()
    var result: Result<ustring, TestError> = Error(TestError::InvalidData(u"bad"))
    match result
        Ok(_) => fail("Should be Error")
        Error(_) => pass("Correctly matched any error")
    end match
end fn

// Test pattern with data extraction
fn test_pattern_with_data()
    var result: Result<ustring, TestError> = Error(TestError::InvalidData(u"bad data"))
    match result
        Ok(_) => fail("Should be Error")
        Error(InvalidData(message)) => assert(message == u"bad data")
        Error(_) => fail("Wrong error type")
    end match
end fn
```

---

## 4. Integration Testing

### Testing Complete Workflows

```poly
// Test user registration workflow
fn test_user_registration()
    var inputs = [
        u"John",           // name
        u"john@email.com", // email
        u"password123"     // password
    ]
    mock_input_sequence(inputs)
    
    put -n "Enter name: "
    var name: ustring = get with validate |n| n.len() >= 2
    
    put -n "Enter email: "
    var email: ustring = get with validate |e| e.contains(u"@")
    
    put -n "Enter password: "
    var password: ustring = get --mask u"*" with validate |p| p.len() >= 8
    
    assert(name == u"John")
    assert(email == u"john@email.com")
    assert(password == u"password123")
end fn

// Test file processing workflow
fn test_file_workflow()
    // Create test file
    put "Line 1\nLine 2\nLine 3" > "workflow_test.txt"
    
    // Read file
    var content: ustring = get < "workflow_test.txt"
    var lines: Vec<ustring> = content.split(u"\n")
    
    // Process lines
    var processed: Vec<ustring> = []
    loop: lines
        processed.push(line.to_uppercase())
    end loop
    
    // Verify
    assert(processed.len() == 3)
    assert(processed[0] == u"LINE 1")
    assert(processed[1] == u"LINE 2")
    assert(processed[2] == u"LINE 3")
    
    // Cleanup
    delete_file("workflow_test.txt")
end fn
```

---

## 5. Edge Case Testing

### Testing Boundary Conditions

```poly
// Test minimum values
fn test_minimum_values()
    var age: i32 = get with validate |x| x >= 1
    // Input: 1
    assert(age == 1)
end fn

// Test maximum values
fn test_maximum_values()
    var age: i32 = get with validate |x| x <= 150
    // Input: 150
    assert(age == 150)
end fn

// Test empty input
fn test_empty_input()
    var input = mock_input("")
    var name: ustring = get --default u"Anonymous"
    assert(name == u"Anonymous")
end fn

// Test very long input
fn test_long_input()
    var long_string: ustring = "a".repeat(10000)
    var input = mock_input(long_string)
    var result: ustring = get
    assert(result.len() == 10000)
end fn
```

### Testing Special Characters

```poly
// Test Unicode characters
fn test_unicode()
    var input = mock_input("日本語")
    var text: ustring = get
    assert(text == u"日本語")
end fn

// Test special characters
fn test_special_chars()
    var input = mock_input("Hello!@#$%^&*()")
    var text: ustring = get
    assert(text == u"Hello!@#$%^&*()")
end fn

// Test newlines in input
fn test_newlines()
    var input = mock_input("Line1\nLine2")
    var text: ustring = get
    assert(text == u"Line1\nLine2")
end fn
```

---

## 6. Performance Testing

### Testing Output Performance

```poly
// Test frequent output
fn test_frequent_output()
    var start = time_now()
    loop: 0..1000
        put -n "."
    end loop
    put ""
    var duration = time_now() - start
    assert(duration < 1000)  // Should complete in under 1 second
end fn

// Test file write performance
fn test_file_write_performance()
    var start = time_now()
    loop: 0..1000
        put "Line " + i.to_string() >> "perf_test.txt"
    end loop
    var duration = time_now() - start
    delete_file("perf_test.txt")
    assert(duration < 5000)  // Should complete in under 5 seconds
end fn
```

### Testing Input Performance

```poly
// Test input parsing performance
fn test_input_parsing()
    var start = time_now()
    loop: 0..1000
        var input = mock_input(i.to_string())
        var num: i32 = get
        assert(num == i)
    end loop
    var duration = time_now() - start
    assert(duration < 2000)  // Should complete in under 2 seconds
end fn
```

---

## 7. Test Utilities

### Mock Input Functions

```poly
// Mock single input
fn mock_input(value: ustring): ustring
    // Implementation depends on test framework
    return value
end fn

// Mock input sequence
fn mock_input_sequence(values: Vec<ustring>)
    // Implementation depends on test framework
end fn

// Mock timeout
fn mock_timeout(ms: i32)
    // Implementation depends on test framework
end fn
```

### Assertion Functions

```poly
// Basic assertion
fn assert(condition: bool)
    if not condition,
        error "Assertion failed"
        exit(1)
    end if
end fn

// Assertion with message
fn assert(condition: bool, message: ustring)
    if not condition,
        error "Assertion failed: " + message
        exit(1)
    end if
end fn

// Test pass/fail
fn pass(message: ustring)
    info "PASS: " + message
end fn

fn fail(message: ustring)
    error "FAIL: " + message
    exit(1)
end fn
```

---

## 8. Test Organization

### Directory Structure

```
tests/
├── unit/
│   ├── output_tests.poly
│   ├── input_tests.poly
│   └── error_handling_tests.poly
├── integration/
│   ├── workflow_tests.poly
│   └── file_operation_tests.poly
├── edge_cases/
│   ├── boundary_tests.poly
│   └── special_chars_tests.poly
├── performance/
│   ├── output_performance_tests.poly
│   └── input_performance_tests.poly
└── utilities/
    ├── mock_functions.poly
    └── assertion_functions.poly
```

### Running Tests

```poly
// Run all tests
fn run_all_tests()
    run_unit_tests()
    run_integration_tests()
    run_edge_case_tests()
    run_performance_tests()
end fn

// Run specific test suite
fn run_unit_tests()
    test_basic_output()
    test_put_n()
    test_error_output()
    test_typed_input()
    test_default_value()
    // ... more tests
end fn
```

---

## Summary

1. **Unit Tests**: Test individual commands and functions
2. **Integration Tests**: Test complete workflows
3. **Edge Cases**: Test boundary conditions and special characters
4. **Performance Tests**: Test speed and efficiency
5. **Test Utilities**: Create reusable mock and assertion functions
6. **Organization**: Use clear directory structure and naming
