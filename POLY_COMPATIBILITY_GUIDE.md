# Poly Language Compatibility Guide

## Overview

This guide covers platform compatibility considerations for the new Poly I/O and error handling syntax.

---

## 1. Platform-Specific Considerations

### Unix/Linux

~~~poly
// Good: Unix-compatible output
put "Hello, World!"  // LF line ending

// Unix file paths
var path ustring := "/home/user/file.txt"
var content ustring := get < path

// Unix permissions
set_file_permissions("file.txt", 0o644)
~~~

### Windows

~~~poly
// Good: Windows-compatible output
put "Hello, World!"  // CRLF line ending handled by OS

// Windows file paths
var path ustring := "C:\\Users\\user\\file.txt"
var content ustring := get < path

// Windows permissions
set_file_permissions("file.txt", 0o644)  // Mapped to Windows ACLs
~~~

### macOS

~~~poly
// Good: macOS-compatible output
put "Hello, World!"  // LF line ending

// macOS file paths
var path ustring := "/Users/user/file.txt"
var content ustring := get < path

// macOS permissions
set_file_permissions("file.txt", 0o644)
~~~

---

## 2. Character Encoding

### UTF-8

~~~poly
// Good: UTF-8 encoding
var text ustring := unicode "Hello, World!"
put text

// UTF-8 file reading
var content ustring := get < "utf8.txt" with encoding unicode "utf-8"
~~~

### ASCII

~~~poly
// Good: ASCII encoding
var text ustring := "Hello, World!"  // ASCII subset
put text

// ASCII file reading
var content ustring := get < "ascii.txt" with encoding unicode "ascii"
~~~

### Latin-1

~~~poly
// Good: Latin-1 encoding
var text ustring := unicode "café"  // Latin-1 characters
put text

// Latin-1 file reading
var content ustring := get < "latin1.txt" with encoding unicode "latin-1"
~~~

---

## 3. Line Endings

### Unix (LF)

~~~poly
// Good: Unix line endings
put "Line 1\nLine 2"  // LF

// Read Unix file
var content ustring := get < "unix.txt"
var lines Vec<ustring> := content.split("\n")
~~~

### Windows (CRLF)

~~~poly
// Good: Windows line endings
put "Line 1\r\nLine 2"  // CRLF

// Read Windows file
var content ustring := get < "windows.txt"
var lines Vec<ustring> := content.split("\r\n")
~~~

### Cross-Platform

~~~poly
// Good: Cross-platform line endings
var newline ustring := if is_windows(),"\r\n" else "\n" end if
put "Line 1" + newline + "Line 2"

// Read any file
var content ustring := get < "any.txt"
var lines Vec<ustring> := content.split("\r?\n")  // Match either
~~~

---

## 4. File Paths

### Absolute Paths

~~~poly
// Good: Absolute paths
var path ustring := if is_windows(),
    "C:\\Users\\user\\file.txt"
else
    "/home/user/file.txt"
end if
var content ustring := get < path
~~~

### Relative Paths

~~~poly
// Good: Relative paths
var path ustring := "./data/file.txt"
var content ustring := get < path
~~~

### Path Separators

~~~poly
// Good: Cross-platform path separators
var path ustring := join_path(["data", "file.txt"])
// Returns "/data/file.txt" on Unix
// Returns "\\data\\file.txt" on Windows
var content ustring := get < path
~~~

---

## 5. Error Handling

### Platform-Specific Errors

~~~poly
// Good: Handle platform-specific errors
enum FileError
    NotFound
    PermissionDenied
    AccessDenied  // Windows-specific
    TooManyOpenFiles  // Unix-specific
end enum

fn read_file(path: ustring): Result<ustring, FileError>
    match platform_read(path)
        Ok(content) => return Ok(content)
        Error(e) =>
            if is_windows() && e.code == 5,
                return Error(FileError::AccessDenied)
            else if is_unix() && e.code == 24,
                return Error(FileError::TooManyOpenFiles)
            else
                return Error(e.to_file_error())
            end if
    end match
end fn
~~~

### Cross-Platform Error Messages

~~~poly fragment
// Good: Cross-platform error messages
fn get_error_message(error: FileError): ustring
    match error
        NotFound => return unicode "File not found"
        PermissionDenied => return unicode "Permission denied"
        AccessDenied => return unicode "Access denied"  // Windows
        TooManyOpenFiles => return unicode "Too many open files"  // Unix
    end match
end fn
~~~

---

## 6. Input/Output

### Terminal Output

~~~poly
// Good: Cross-platform terminal output
put "Hello, World!"  // Works on all platforms
put -n "Progress: "  // Works on all platforms

// Platform-specific formatting
if is_windows(),
    put "Windows-style output"
else
    put "Unix-style output"
end if
~~~

### Terminal Input

~~~poly
// Good: Cross-platform input
put -n "Enter your name: "
var name ustring := get  // Works on all platforms

// Platform-specific input handling
if is_windows(),
    // Windows-specific input handling
else
    // Unix-specific input handling
end if
~~~

### File I/O

~~~poly
// Good: Cross-platform file I/O
var content ustring := get < "file.txt"  // Works on all platforms
put "data" > "output.txt"  // Works on all platforms

// Platform-specific file operations
if is_windows(),
    // Windows-specific file operations
else
    // Unix-specific file operations
end if
~~~

---

## 7. Networking

### Cross-Platform Networking

~~~poly
// Good: Cross-platform networking
var response := get --timeout 5000 < "https://api.example.com"

// Platform-specific networking
if is_windows(),
    // Windows-specific networking
else
    // Unix-specific networking
end if
~~~

### SSL/TLS

~~~poly
// Good: Cross-platform SSL/TLS
var response := get < "https://api.example.com" with verify_certificate(true)

// Platform-specific SSL/TLS
if is_windows(),
    // Windows-specific SSL/TLS
else
    // Unix-specific SSL/TLS
end if
~~~

---

## 8. Performance

### Cross-Platform Performance

~~~poly
// Good: Cross-platform performance
var start := time_now()
// Performance-critical code
var duration := time_now() - start
put "Duration: " + duration.to_string() + "ms"

// Platform-specific optimizations
if is_windows(),
    // Windows-specific optimizations
else
    // Unix-specific optimizations
end if
~~~

### Memory Management

~~~poly
// Good: Cross-platform memory management
var data Vec<ustring> := []
data.reserve(1000)  // Pre-allocate

// Platform-specific memory management
if is_windows(),
    // Windows-specific memory management
else
    // Unix-specific memory management
end if
~~~

---

## 9. Testing

### Cross-Platform Testing

~~~poly
// Good: Cross-platform tests
fn test_file_operations()
    var test_file ustring := if is_windows(),
        "test_windows.txt"
    else
        "test_unix.txt"
    end if

    put "Test content" > test_file
    var content ustring := get < test_file
    assert(content == unicode "Test content")
    delete_file(test_file)
end fn

// Platform-specific tests
fn test_platform_specific()
    if is_windows(),
        test_windows_specific()
    else
        test_unix_specific()
    end if
end fn
~~~

---

## 10. Best Practices

### Use Platform Detection

~~~poly
// Good: Platform detection
if is_windows(),
    // Windows-specific code
else if is_macos(),
    // macOS-specific code
else if is_linux(),
    // Linux-specific code
else
    // Fallback code
end if
~~~

### Use Abstractions

~~~poly
// Good: Use abstractions
fn read_file(path: ustring): Result<ustring, FileError>
    return platform_read(path)  // Platform-specific implementation
end fn

// Bad: Platform-specific code everywhere
fn read_file(path: ustring): Result<ustring, FileError>
    if is_windows(),
        // Windows-specific code
    else
        // Unix-specific code
    end if
end fn
~~~

### Test on Multiple Platforms

~~~poly
// Good: Cross-platform testing
fn test_cross_platform()
    // Test on current platform
    test_file_operations()
    test_network_operations()
    test_error_handling()

    // Note: Full cross-platform testing requires CI/CD
end fn
~~~

---

## Summary

1. **Platform Detection**: Use `is_windows()`, `is_macos()`, `is_linux()` for platform-specific code
2. **Character Encoding**: Use UTF-8 by default, support other encodings when needed
3. **Line Endings**: Handle both LF and CRLF
4. **File Paths**: Use path utilities for cross-platform compatibility
5. **Error Handling**: Handle platform-specific errors gracefully
6. **I/O**: Use standard I/O functions that work across platforms
7. **Networking**: Use cross-platform networking libraries
8. **Performance**: Optimize for each platform when necessary
9. **Testing**: Test on multiple platforms
10. **Abstractions**: Use abstractions to hide platform differences
