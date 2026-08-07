# Poly Language Troubleshooting Guide

## Overview

This guide covers common issues and solutions for the new Poly I/O and error handling syntax.

**Note:** Some functions used in this guide (like `is_input_available()`, `get_disk_free_space()`, `get_file_permissions()`, `set_file_permissions()`, `file_exists()`) are standard library functions. See the Standard Library section for details.

---

## 1. Output Issues

### Issue: Output Not Showing

**Symptoms:**
```poly
put \"Hello, World!\"  // Nothing appears
```

**Solutions:**
```poly
// Solution 1: Check for buffering\nput \"Hello, World!\" + \"\\n\"  // Force flush\n\n// Solution 2: Use error output (always shows)\nerror \"Hello, World!\"\n\n// Solution 3: Check if output is being captured\nvar output = capture put \"Hello, World!\"\ninfo \"Output: \" + output  // Check captured output\n```

### Issue: Extra Newlines

**Symptoms:**
```poly
put \"Line 1\"\nput \"Line 2\"\n// Output:\n// Line 1\n//\n// Line 2\n```

**Solutions:**
```poly\n// Solution 1: Use -n flag\nput -n \"Line 1\"\nput \"Line 2\"\n// Output:\n// Line 1Line 2\n\n// Solution 2: Manual newline control\nput \"Line 1\\nLine 2\"\n// Output:\n// Line 1\n// Line 2\n```

### Issue: File Output Not Working

**Symptoms:**
```poly\nput \"data\" > \"output.txt\"  // File not created\n```

**Solutions:**
```poly\n// Solution 1: Check file permissions\nput \"data\" > \"output.txt\"\nvar permissions = get_file_permissions(\"output.txt\")\ninfo \"Permissions: \" + permissions.to_string()\n\n// Solution 2: Check disk space\nvar free_space = get_disk_free_space(\".\")\ninfo \"Free space: \" + free_space.to_string() + \" bytes\"\n\n// Solution 3: Use absolute path\nput \"data\" > \"/absolute/path/to/output.txt\"\n```

---

## 2. Input Issues

### Issue: Input Not Reading

**Symptoms:**
```poly\nvar input: ustring = get  // Program hangs\n```

**Solutions:**
```poly\n// Solution 1: Use timeout\nmatch get --timeout 5000\n    Ok(input) => process(input)\n    Timeout => warn \"Input timeout\"\n    Error(e) => error \"Input error: \" + e\nend match\n\n// Solution 2: Check if input is available\nif is_input_available() then\n    var input: ustring = get\nelse\n    warn \"No input available\"\nend if\n\n// Solution 3: Use default value\nvar input: ustring = get --default u\"default\"\n```

### Issue: Type Conversion Fails

**Symptoms:**
```poly\nvar age: i32 = get  // Input: \"abc\"\n// Error: type conversion failed\n```

**Solutions:**
```poly\n// Solution 1: Validate before conversion\nvar input: ustring = get\nif input.parse::<i32>().is_ok() then\n    var age: i32 = input.parse::<i32>().unwrap()\nelse\n    error \"Please enter a valid number\"\nend if\n\n// Solution 2: Use validation closure\nvar age: i32 = get with validate |x| x.parse::<i32>().is_ok()\n\n// Solution 3: Use default value\nvar age: i32 = get --as i32 --default 0\n```

### Issue: Default Value Not Working

**Symptoms:**
```poly\nvar input: ustring = get --default u\"default\"  // Still prompts for input\n```

**Solutions:**
```poly\n// Solution 1: Check if input is empty\nvar input: ustring = get\nif input.len() == 0 then\n    input = u\"default\"\nend if\n\n// Solution 2: Use validation with default\nvar input: ustring = get with validate |i| i.len() > 0 --default u\"default\"\n\n// Solution 3: Use environment variable\nvar input: ustring = get_env(\"INPUT\") or u\"default\"\n```

---

## 3. Error Handling Issues

### Issue: Error Not Caught

**Symptoms:**
```poly\nmatch result\n    Ok(value) => process(value)\n    Error(e) => handle_error(e)  // Error not caught\nend match\n```

**Solutions:**
```poly\n// Solution 1: Check error type\nmatch result\n    Ok(value) => process(value)\n    Error(FileError::NotFound) => error \"File not found\"\n    Error(FileError::PermissionDenied) => error \"Permission denied\"\n    Error(e) => error \"Unknown error: \" + e.to_string()\nend match\n\n// Solution 2: Use wildcard pattern\nmatch result\n    Ok(value) => process(value)\n    Error(_) => error \"An error occurred\"\nend match\n\n// Solution 3: Log error details\nmatch result\n    Ok(value) => process(value)\n    Error(e) => \n        error \"Error: \" + e.to_string()\n        info \"Error type: \" + type_of(e)\n        info \"Stack trace: \" + get_stack_trace()\nend match\n```

### Issue: Error Propagation Not Working

**Symptoms:**
```poly\nfn risky_operation(): Result<ustring, ustring>\n    var result = try other_operation()  // Error not propagated\n    return Ok(result)\nend fn\n```

**Solutions:**
```poly\n// Solution 1: Check return type\nfn risky_operation(): Result<ustring, ustring>  // Must return Result\n    var result = try other_operation()\n    return Ok(result)\nend fn\n\n// Solution 2: Use match instead of try\nfn risky_operation(): Result<ustring, ustring>\n    match other_operation()\n        Ok(result) => return Ok(result)\n        Error(e) => return Error(e)\n    end match\nend fn\n\n// Solution 3: Explicit error handling\nfn risky_operation(): Result<ustring, ustring>\n    var result = other_operation()\n    if result.is_err() then\n        return Error(result.error())\n    end if\n    return Ok(result.unwrap())\nend fn\n```

### Issue: Panic on Unwrap

**Symptoms:**
```poly\nvar value = result.unwrap()  // Panics if error\n```

**Solutions:**
```poly\n// Solution 1: Use match\nmatch result\n    Ok(value) => process(value)\n    Error(e) => handle_error(e)\nend match\n\n// Solution 2: Use unwrap_or\nvar value = result.unwrap_or(default_value)\n\n// Solution 3: Use unwrap_or_else\nvar value = result.unwrap_or_else(|| compute_default())\n```

---

## 4. File Operation Issues

### Issue: File Not Found

**Symptoms:**
```poly\nvar content: ustring = get < \"file.txt\"  // Error: file not found\n```

**Solutions:**
```poly\n// Solution 1: Check if file exists\nif file_exists(\"file.txt\") then\n    var content: ustring = get < \"file.txt\"\nelse\n    error \"File not found\"\nend if\n\n// Solution 2: Use error handling\nmatch get < \"file.txt\"\n    Ok(content) => process(content)\n    Error(e) => error \"File error: \" + e\nend match\n\n// Solution 3: Use default value\nvar content: ustring = get < \"file.txt\" --default u\"\"\n```

### Issue: Permission Denied

**Symptoms:**
```poly\nput \"data\" > \"output.txt\"  // Error: permission denied\n```

**Solutions:**
```poly\n// Solution 1: Check permissions\nvar permissions = get_file_permissions(\"output.txt\")\ninfo \"Permissions: \" + permissions.to_string()\n\n// Solution 2: Change permissions\nset_file_permissions(\"output.txt\", 0o644)\n\n// Solution 3: Use different file location\nput \"data\" > \"/tmp/output.txt\"  // Use temp directory\n```

### Issue: File Too Large

**Symptoms:**
```poly\nvar content: ustring = get < \"large_file.txt\"  // Error: out of memory\n```

**Solutions:**
```poly\n// Solution 1: Read in chunks\nvar file = open(\"large_file.txt\")\nwhile !file.eof()\n    var chunk: ustring = file.read_chunk(1024)\n    process(chunk)\nend while\n\n// Solution 2: Use streaming\nvar stream = open_stream(\"large_file.txt\")\nloop: stream\n    process(line)\nend loop\n\n// Solution 3: Use binary mode for large files\nvar data: bytes = get < \"large_file.bin\" --bytes 1024\n```

---

## 5. Performance Issues

### Issue: Slow Output

**Symptoms:**
```poly\nloop: 0..10000\n    put \"Line \" + i.to_string()  // Very slow\nend loop\n```

**Solutions:**
```poly\n// Solution 1: Buffer output\nvar buffer: Vec<ustring> = []\nloop: 0..10000\n    buffer.push(\"Line \" + i.to_string())\nend loop\nput buffer.join(\"\\n\")\n\n// Solution 2: Use -n for progress\nloop: 0..10000\n    put -n \"\\rProgress: \" + i.to_string()\nend loop\nput \"\"\n\n// Solution 3: Write to file\nloop: 0..10000\n    put \"Line \" + i.to_string() >> \"output.txt\"\nend loop\n```

### Issue: Slow Input

**Symptoms:**
```poly\nvar input: ustring = get  // Very slow\n```

**Solutions:**
```poly\n// Solution 1: Use timeout\nmatch get --timeout 5000\n    Ok(input) => process(input)\n    Timeout => warn \"Timeout\"\n    Error(e) => error e\nend match\n\n// Solution 2: Use default value\nvar input: ustring = get --default u\"\"\n\n// Solution 3: Use validation\nvar input: ustring = get with validate |i| i.len() > 0\n```

### Issue: Memory Usage

**Symptoms:**
```poly\nvar large_string: ustring = \"a\".repeat(1000000)  // High memory usage\n```

**Solutions:**
```poly\n// Solution 1: Use streaming\nvar stream = open_stream(\"large_file.txt\")\nloop: stream\n    process(line)  // Process line by line\nend loop\n\n// Solution 2: Use chunks\nvar chunks = large_string.chunks(1024)\nloop: chunks\n    process(chunk)\nend loop\n\n// Solution 3: Use generators\nfn generate_data(): Iterator<ustring>\n    loop: 0..1000000\n        yield \"Line \" + i.to_string()\n    end loop\nend fn\n\nloop: generate_data()\n    process(line)\nend loop\n```

---

## 6. Network Issues

### Issue: Connection Timeout

**Symptoms:**
```poly\nvar response = get < \"https://api.example.com\"  // Timeout\n```

**Solutions:**
```poly\n// Solution 1: Use timeout\nmatch get --timeout 5000 < \"https://api.example.com\"\n    Ok(response) => process(response)\n    Timeout => warn \"Connection timeout\"\n    Error(e) => error \"Connection error: \" + e\nend match\n\n// Solution 2: Use retry logic\nvar response = retry(3, || get < \"https://api.example.com\")\n\n// Solution 3: Use async\nvar response = async get < \"https://api.example.com\"\n// Do other work\nvar result = await response\n```

### Issue: SSL Certificate Error

**Symptoms:**
```poly\nvar response = get < \"https://api.example.com\"  // SSL error\n```

**Solutions:**
```poly\n// Solution 1: Verify certificate\nvar response = get < \"https://api.example.com\" with verify_certificate(true)\n\n// Solution 2: Use trusted CA\nvar response = get < \"https://api.example.com\" with ca_certificate(\"ca.pem\")\n\n// Solution 3: Skip verification (insecure)\nvar response = get < \"https://api.example.com\" with verify_certificate(false)  // Not recommended\n```

---

## 7. Debugging Tips

### Enable Debug Mode

```poly\n// Set debug environment variable\nset_env(\"DEBUG\", \"true\")\n\n// Check debug mode\nvar debug_mode: bool = get_env(\"DEBUG\") or u\"false\" == u\"true\"\nif debug_mode then\n    info \"Debug mode enabled\"\n    // Debug output\nend if\n```

### Use Logging

```poly\n// Log function entry/exit\nfn process_data()\n    info \"Entering process_data\"\n    // ... function body\n    info \"Exiting process_data\"\nend fn\n\n// Log variable values\nvar x: i32 = 42\ninfo \"x = \" + x.to_string()\n\n// Log function results\nvar result = risky_operation()\nmatch result\n    Ok(value) => info \"Success: \" + value.to_string()\n    Error(e) => error \"Error: \" + e.to_string()\nend match\n```

### Use Breakpoints

```poly\n// Pseudo-code for debugging\nfn complex_function()\n    // ... code before breakpoint\n    debug_break()  // Pause execution\n    // ... code after breakpoint\nend fn\n```

---

## Summary

1. **Output Issues**: Check buffering, newlines, file permissions
2. **Input Issues**: Use timeouts, validation, default values
3. **Error Handling**: Use match, check error types, log details
4. **File Operations**: Check existence, permissions, file size
5. **Performance**: Buffer output, use streaming, optimize memory
6. **Network Issues**: Use timeouts, retry logic, async
7. **Debugging**: Enable debug mode, use logging, breakpoints
