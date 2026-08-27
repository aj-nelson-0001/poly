# Poly Language Security Guide

> **Historical guide:** This document predates the v2 target contract. Treat its examples as advisory and consult [POLY_C_BLOCKS.md](POLY_C_BLOCKS.md) for current C limitations.

## Overview

This guide covers security best practices for the new Poly I/O and error handling syntax.

**Note:** Some functions used in this guide (like `bcrypt_hash()`, `generate_secure_token()`, `aes_encrypt()`) are standard library functions or require external dependencies. See the Standard Library section for details.

---

## 1. Input Validation

### Always Validate User Input

~~~poly
// Good: Validate all input
var age i32 := get with validate |x| x > 0 && x < 150
var email ustring := get with validate |e| e.contains(unicode "@") && e.len() < 255
var name ustring := get with validate |n| n.len() >= 1 && n.len() <= 100

// Bad: Trust input
var age i32 := get  // Could be negative or huge
var email ustring := get  // Could be invalid
var name ustring := get  // Could be empty or malicious
~~~

### Sanitize Input

~~~poly fragment
// Good: Sanitize input
fn sanitize(input: ustring): ustring
    // Remove potentially dangerous characters
    var sanitized ustring := input.replace(unicode "<", unicode "&lt;")
    sanitized := sanitized.replace(unicode ">", unicode "&gt;")
    sanitized := sanitized.replace(unicode "\"", unicode "&quot;")
    return sanitized
end fn

var name ustring := get with validate |n| n.len() > 0
var safe_name ustring := sanitize(name)
~~~

### Use Whitelisting

~~~poly fragment
// Good: Whitelist allowed characters
var username ustring := get with validate |u|
    u.len() >= 3 && u.len() <= 20 &&
    u.all(|c| c.is_alphanumeric() || c = unicode '_' || c = unicode '-')
end

// Bad: Blacklist characters
var username ustring := get  // No validation
~~~

---

## 2. Password Security

### Always Mask Password Input

~~~poly
// Good: Mask passwords
put "Enter password: "
var password ustring := get --mask unicode "*"

// Bad: Expose passwords
put "Enter password: "
var password ustring := get  // Visible on screen
~~~

### Validate Password Strength

~~~poly fragment
// Good: Validate password strength
var password ustring := get --mask unicode "*" with validate |p|
    p.len() >= 8 &&
    p.any(|c| c.is_uppercase()) &&
    p.any(|c| c.is_lowercase()) &&
    p.any(|c| c.is_digit()) &&
    p.any(|c| !c.is_alphanumeric())
end

// Bad: No password validation
var password ustring := get --mask unicode "*"  // Weak password allowed
~~~

### Never Store Plain Text Passwords

~~~poly fragment
// Good: Hash passwords
fn hash_password(password: ustring): ustring
    // Use proper hashing algorithm (e.g., bcrypt, argon2)
    return bcrypt_hash(password)
end fn

var password ustring := get --mask unicode "*"
var hashed ustring := hash_password(password)
store_user(username, hashed)

// Bad: Store plain text
var password ustring := get --mask unicode "*"
store_user(username, password)  // Insecure!
~~~

---

## 3. File Operations

### Validate File Paths

~~~poly
// Good: Validate file paths
fn is_valid_path(path: ustring): bool
    // Check for path traversal
    if path.contains(unicode ".."),
        return false
    end if
    
    // Check for absolute paths (if not allowed)
    if path.starts_with(unicode "/"),
        return false
    end if
    
    return true
end fn

var filename ustring := get with validate |f| is_valid_path(f)
var content ustring := get from  filename

// Bad: No path validation
var filename ustring := get
var content ustring := get from  filename  // Could access any file
~~~

### Use Safe File Permissions

~~~poly fragment
// Good: Set restrictive permissions
put "sensitive data" to "secret.txt"
set_file_permissions("secret.txt", 0o600)  // Owner read/write only

// Bad: Default permissions
put "sensitive data" to "secret.txt"  // World-readable by default
~~~

### Validate File Content

~~~poly fragment
// Good: Validate file content
var content ustring := get from "config.txt" with validate |c|
    c.len() < 1000000 &&  // Limit file size
    not c.contains(unicode "<script") &&  // Basic XSS prevention
    not c.contains(unicode "javascript:")  // Basic XSS prevention
end

// Bad: No content validation
var content ustring := get from "config.txt"  // Could be malicious
~~~

---

## 4. Error Handling

### Don't Expose Sensitive Information

~~~poly fragment
// Good: Generic error messages
match read_file(unicode "config.txt")
    Ok(content), process(content)
    Error(_), error "Failed to load configuration"  // Generic message
end match

// Bad: Expose sensitive information
match read_file(unicode "config.txt")
    Ok(content), process(content)
    Error(e), error "Error: " + e.to_string()  // Could expose file paths, etc.
end match
~~~

### Log Errors Securely

~~~poly fragment
// Good: Log to secure location
fn log_error(error: ustring, context: ustring)
    var timestamp ustring := get_timestamp()
    var log_entry ustring := timestamp + " | " + context + " | " + error
    put log_entry >to "app.log"
    set_file_permissions("app.log", 0o640)
end fn

// Bad: Log to insecure location
fn log_error(error: ustring)
    put error >to "/tmp/error.log"  // World-readable
end fn
~~~

### Handle Errors Gracefully

~~~poly fragment
// Good: Graceful error handling
fn process_data(): Result<ustring, ustring>
    var data := try read_file(unicode "data.txt")
    var validated := try validate_data(data)
    return Ok(validated)
end fn

// Don't panic on errors
match process_data()
    Ok(data), use(data)
    Error(e),
        error "Processing failed"
        return Default::default()  // Return sensible default
end match

// Bad: Panic on errors
fn process_data(): Result<ustring, ustring>
    var data := read_file(unicode "data.txt").unwrap()  // Panics on error
    return Ok(data)
end fn
~~~

---

## 5. Input/Output Security

### Use Timeouts

~~~poly fragment
// Good: Prevent DoS attacks
match get --timeout 5000
    Ok(input), process(input)
    Timeout, warn "Input timeout"
    Error(e), error "Input error"
end match

// Bad: No timeout
var input ustring := get  // Can hang forever, allowing DoS
~~~

### Limit Input Size

~~~poly
// Good: Limit input size
var input ustring := get with validate |i| i.len() <= 10000

// Bad: No size limit
var input ustring := get  // Could be huge, causing memory issues
~~~

### Sanitize Output

~~~poly fragment
// Good: Sanitize output for HTML
fn html_escape(input: ustring): ustring
    var output ustring := input.replace(unicode "&", unicode "&amp;")
    output := output.replace(unicode "<", unicode "&lt;")
    output := output.replace(unicode ">", unicode "&gt;")
    output := output.replace(unicode "\"", unicode "&quot;")
    return output
end fn

var user_input ustring := get
put html_escape(user_input)  // Safe for HTML

// Bad: No output sanitization
var user_input ustring := get
put user_input  // Could contain malicious HTML
~~~

---

## 6. Authentication and Authorization

### Validate Credentials

~~~poly fragment
// Good: Validate credentials
fn authenticate(username: ustring, password: ustring): Result<User, AuthError>
    var user := try get_user(username)
    var hashed_password := try get_password_hash(username)
    
    if not verify_password(password, hashed_password),
        return Error(AuthError::InvalidCredentials)
    end if
    
    return Ok(user)
end fn

// Bad: No validation
fn authenticate(username: ustring, password: ustring): Result<User, AuthError>
    var user := get_user(username).unwrap()  // Panics if user not found
    return Ok(user)  // No password check
end fn
~~~

### Use Secure Session Management

~~~poly fragment
// Good: Secure session handling
fn create_session(user: User): Session
    var session_id := generate_secure_token()
    var expiry := get_timestamp() + 3600  // 1 hour
    
    store_session(session_id, user.id, expiry)
    return Session(id: session_id, expiry: expiry)
end fn

// Bad: Insecure session handling
fn create_session(user: User): Session
    var session_id := user.id.to_string()  // Predictable
    return Session(id: session_id)
end fn
~~~

---

## 7. Data Protection

### Encrypt Sensitive Data

~~~poly fragment
// Good: Encrypt sensitive data
fn encrypt_data(data: ustring, key: ustring): ustring
    // Use proper encryption (e.g., AES-256)
    return aes_encrypt(data, key)
end fn

var sensitive_data ustring := get
var encrypted ustring := encrypt_data(sensitive_data, encryption_key)
store_encrypted(encrypted)

// Bad: Store plain text
var sensitive_data ustring := get
store_plain(sensitive_data)  // Insecure!
~~~

### Use Secure Random Generation

~~~poly fragment
// Good: Secure random tokens (requires a random-index source, e.g. from a
// cryptographically secure RNG exposed by the runtime)
fn generate_token(length: i32): ustring
    var chars ustring := "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789"
    var token ustring := ""
    loop i 0..length
        token := token + chars[random_index(chars.len())]
    end loop
    return token
end fn

// Bad: Predictable tokens (timestamps are guessable)
fn generate_token(length: i32): ustring
    return get_timestamp().to_string()  // Predictable
end fn
~~~

---

## 8. Network Security

### Use HTTPS

~~~poly
// Good: Use HTTPS
var url ustring := "https://api.example.com/data"
var response := get from  url --timeout 5000

// Bad: Use HTTP
var url ustring := "http://api.example.com/data"  // Insecure
var response := get from  url
~~~

### Validate Certificates

~~~poly fragment
// Good: Validate SSL certificates
var response := get from "https://api.example.com" with verify_certificate(true)

// Bad: Skip certificate validation
var response := get from "https://api.example.com" with verify_certificate(false)  // Insecure
~~~

---

## 9. Code Security

### Avoid Code Injection

~~~poly fragment
// Good: Use parameterized queries
fn get_user(username: ustring): Result<User, DBError>
    var query ustring := "SELECT * FROM users WHERE username = ?"
    return db.query(query, [username])
end fn

// Bad: String concatenation
fn get_user(username: ustring): Result<User, DBError>
    var query ustring := "SELECT * FROM users WHERE username = '" + username + "'"  // SQL injection!
    return db.query(query)
end fn
~~~

### Validate External Data

~~~poly fragment
// Good: Validate external data
fn process_external_data(data: ustring): Result<ustring, ustring>
    // Validate data format
    if not data.is_valid_json(),
        return Error(unicode "Invalid JSON format")
    end if
    
    // Validate data size
    if data.len() > 1000000,
        return Error(unicode "Data too large")
    end if
    
    // Validate data content
    var parsed := try parse_json(data)
    if not validate_schema(parsed),
        return Error(unicode "Data doesn't match schema")
    end if
    
    return Ok(data)
end fn

// Bad: No validation
fn process_external_data(data: ustring): Result<ustring, ustring>
    return Ok(data)  // No validation
end fn
~~~

---

## Summary

1. **Input Validation**: Always validate and sanitize input
2. **Password Security**: Mask input, validate strength, hash storage
3. **File Operations**: Validate paths, use safe permissions, validate content
4. **Error Handling**: Don't expose sensitive info, log securely
5. **I/O Security**: Use timeouts, limit input size, sanitize output
6. **Authentication**: Validate credentials, use secure sessions
7. **Data Protection**: Encrypt sensitive data, use secure random
8. **Network Security**: Use HTTPS, validate certificates
9. **Code Security**: Avoid injection, validate external data
