# Poly Language Compatibility Guide

## Overview

This guide covers platform compatibility considerations for the new Poly I/O and error handling syntax.

---

## 1. Platform-Specific Considerations

### Unix/Linux

```poly
// Good: Unix-compatible output\nput \"Hello, World!\"  // LF line ending\n\n// Unix file paths\nvar path: ustring = \"/home/user/file.txt\"\nvar content: ustring = get < path\n\n// Unix permissions\nset_file_permissions(\"file.txt\", 0o644)\n```

### Windows

```poly
// Good: Windows-compatible output\nput \"Hello, World!\"  // CRLF line ending handled by OS\n\n// Windows file paths\nvar path: ustring = \"C:\\\\Users\\\\user\\\\file.txt\"\nvar content: ustring = get < path\n\n// Windows permissions\nset_file_permissions(\"file.txt\", 0o644)  // Mapped to Windows ACLs\n```

### macOS

```poly
// Good: macOS-compatible output\nput \"Hello, World!\"  // LF line ending\n\n// macOS file paths\nvar path: ustring = \"/Users/user/file.txt\"\nvar content: ustring = get < path\n\n// macOS permissions\nset_file_permissions(\"file.txt\", 0o644)\n```

---

## 2. Character Encoding

### UTF-8

```poly
// Good: UTF-8 encoding\nvar text: ustring = u\"Hello, World!\"\nput text\n\n// UTF-8 file reading\nvar content: ustring = get < \"utf8.txt\" with encoding u\"utf-8\"\n```

### ASCII

```poly
// Good: ASCII encoding\nvar text: ustring = \"Hello, World!\"  // ASCII subset\nput text\n\n// ASCII file reading\nvar content: ustring = get < \"ascii.txt\" with encoding u\"ascii\"\n```

### Latin-1

```poly
// Good: Latin-1 encoding\nvar text: ustring = u\"café\"  // Latin-1 characters\nput text\n\n// Latin-1 file reading\nvar content: ustring = get < \"latin1.txt\" with encoding u\"latin-1\"\n```

---

## 3. Line Endings

### Unix (LF)

```poly
// Good: Unix line endings\nput \"Line 1\\nLine 2\"  // LF\n\n// Read Unix file\nvar content: ustring = get < \"unix.txt\"\nvar lines: Vec<ustring> = content.split(\"\\n\")\n```

### Windows (CRLF)

```poly
// Good: Windows line endings\nput \"Line 1\\r\\nLine 2\"  // CRLF\n\n// Read Windows file\nvar content: ustring = get < \"windows.txt\"\nvar lines: Vec<ustring> = content.split(\"\\r\\n\")\n```

### Cross-Platform

```poly
// Good: Cross-platform line endings\nvar newline: ustring = if is_windows(),\"\\r\\n\" else \"\\n\" end if\nput \"Line 1\" + newline + \"Line 2\"\n\n// Read any file\nvar content: ustring = get < \"any.txt\"\nvar lines: Vec<ustring> = content.split(\"\\r?\\n\")  // Match either\n```

---

## 4. File Paths

### Absolute Paths

```poly
// Good: Absolute paths\nvar path: ustring = if is_windows(),\n    \"C:\\\\Users\\\\user\\\\file.txt\"\nelse\n    \"/home/user/file.txt\"\nend if\nvar content: ustring = get < path\n```

### Relative Paths

```poly
// Good: Relative paths\nvar path: ustring = \"./data/file.txt\"\nvar content: ustring = get < path\n```

### Path Separators

```poly
// Good: Cross-platform path separators\nvar path: ustring = join_path([\"data\", \"file.txt\"])\n// Returns \"/data/file.txt\" on Unix\n// Returns \"\\\\data\\\\file.txt\" on Windows\nvar content: ustring = get < path\n```

---

## 5. Error Handling

### Platform-Specific Errors

```poly
// Good: Handle platform-specific errors\nenum FileError\n    NotFound\n    PermissionDenied\n    AccessDenied  // Windows-specific\n    TooManyOpenFiles  // Unix-specific\nend enum\n\nfn read_file(path: ustring): Result<ustring, FileError>\n    match platform_read(path)\n        Ok(content) => return Ok(content)\n        Error(e) =>\n            if is_windows() && e.code == 5,\n                return Error(FileError::AccessDenied)\n            else if is_unix() && e.code == 24,\n                return Error(FileError::TooManyOpenFiles)\n            else\n                return Error(e.to_file_error())\n            end if\n    end match\nend fn\n```

### Cross-Platform Error Messages

```poly
// Good: Cross-platform error messages\nfn get_error_message(error: FileError): ustring\n    match error\n        NotFound => return u\"File not found\"\n        PermissionDenied => return u\"Permission denied\"\n        AccessDenied => return u\"Access denied\"  // Windows\n        TooManyOpenFiles => return u\"Too many open files\"  // Unix\n    end match\nend fn\n```

---

## 6. Input/Output

### Terminal Output

```poly
// Good: Cross-platform terminal output\nput \"Hello, World!\"  // Works on all platforms\nput -n \"Progress: \"  // Works on all platforms\n\n// Platform-specific formatting\nif is_windows(),\n    put \"Windows-style output\"\nelse\n    put \"Unix-style output\"\nend if\n```

### Terminal Input

```poly
// Good: Cross-platform input\nput -n \"Enter your name: \"\nvar name: ustring = get  // Works on all platforms\n\n// Platform-specific input handling\nif is_windows(),\n    // Windows-specific input handling\nelse\n    // Unix-specific input handling\nend if\n```

### File I/O

```poly
// Good: Cross-platform file I/O\nvar content: ustring = get < \"file.txt\"  // Works on all platforms\nput \"data\" > \"output.txt\"  // Works on all platforms\n\n// Platform-specific file operations\nif is_windows(),\n    // Windows-specific file operations\nelse\n    // Unix-specific file operations\nend if\n```

---

## 7. Networking

### Cross-Platform Networking

```poly
// Good: Cross-platform networking\nvar response = get --timeout 5000 < \"https://api.example.com\"\n\n// Platform-specific networking\nif is_windows(),\n    // Windows-specific networking\nelse\n    // Unix-specific networking\nend if\n```

### SSL/TLS

```poly
// Good: Cross-platform SSL/TLS\nvar response = get < \"https://api.example.com\" with verify_certificate(true)\n\n// Platform-specific SSL/TLS\nif is_windows(),\n    // Windows-specific SSL/TLS\nelse\n    // Unix-specific SSL/TLS\nend if\n```

---

## 8. Performance

### Cross-Platform Performance

```poly
// Good: Cross-platform performance\nvar start = time_now()\n// Performance-critical code\nvar duration = time_now() - start\nput \"Duration: \" + duration.to_string() + \"ms\"\n\n// Platform-specific optimizations\nif is_windows(),\n    // Windows-specific optimizations\nelse\n    // Unix-specific optimizations\nend if\n```

### Memory Management

```poly
// Good: Cross-platform memory management\nvar data: Vec<ustring> = []\ndata.reserve(1000)  // Pre-allocate\n\n// Platform-specific memory management\nif is_windows(),\n    // Windows-specific memory management\nelse\n    // Unix-specific memory management\nend if\n```

---

## 9. Testing

### Cross-Platform Testing

```poly
// Good: Cross-platform tests\nfn test_file_operations()\n    var test_file: ustring = if is_windows(),\n        \"test_windows.txt\"\n    else\n        \"test_unix.txt\"\n    end if\n    \n    put \"Test content\" > test_file\n    var content: ustring = get < test_file\n    assert(content == u\"Test content\")\n    delete_file(test_file)\nend fn\n\n// Platform-specific tests\nfn test_platform_specific()\n    if is_windows(),\n        test_windows_specific()\n    else\n        test_unix_specific()\n    end if\nend fn\n```

---

## 10. Best Practices

### Use Platform Detection

```poly
// Good: Platform detection\nif is_windows(),\n    // Windows-specific code\nelse if is_macos(),\n    // macOS-specific code\nelse if is_linux(),\n    // Linux-specific code\nelse\n    // Fallback code\nend if\n```

### Use Abstractions

```poly
// Good: Use abstractions\nfn read_file(path: ustring): Result<ustring, FileError>\n    return platform_read(path)  // Platform-specific implementation\nend fn\n\n// Bad: Platform-specific code everywhere\nfn read_file(path: ustring): Result<ustring, FileError>\n    if is_windows(),\n        // Windows-specific code\n    else\n        // Unix-specific code\n    end if\nend fn\n```

### Test on Multiple Platforms

```poly
// Good: Cross-platform testing\nfn test_cross_platform()\n    // Test on current platform\n    test_file_operations()\n    test_network_operations()\n    test_error_handling()\n    \n    // Note: Full cross-platform testing requires CI/CD\nend fn\n```

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
