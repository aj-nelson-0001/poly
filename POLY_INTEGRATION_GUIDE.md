# Poly Language External Services Integration Guide

## Overview

This guide covers integrating with external services in Poly applications using the new I/O and error handling syntax.

**Note:** Some functions used in this guide (like `base64_encode()`, `hmac_sha256()`, `connect()`, `post_request()`) are standard library functions or require external dependencies. See the Standard Library section for details.

---

## 1. HTTP Client Integration

### Basic HTTP Requests

~~~poly
// HTTP client
fn http_get(url: ustring): Result<ustring, HttpError>
    match get --timeout 10000 < url
        Ok(response) => return Ok(response)
        Timeout => return Error(HttpError::Timeout)
        Error(e) => return Error(HttpError::NetworkError(e))
    end match
end fn

fn http_post(url: ustring, body: ustring): Result<ustring, HttpError>
    // POST request implementation
    var response := try post_request(url, body)
    return Ok(response)
end fn

// Usage
match http_get(unicode "https://api.example.com/data")
    Ok(data) => process(data)
    Error(HttpError::Timeout) => warn "Request timed out"
    Error(HttpError::NetworkError(e)) => error "Network error: " + e
end match
~~~

### REST API Client

~~~poly fragment
// REST API client
struct APIClient
    base_url: ustring
    api_key: ustring
    timeout_ms: i32
end struct

fn client_get(client: APIClient, endpoint: ustring): Result<ustring, APIError>
    var url := client.base_url + endpoint
    var headers := [
        (unicode "Authorization", unicode "Bearer " + client.api_key),
        (unicode "Content-Type", unicode "application/json")
    ]

    match get --timeout client.timeout_ms < url with headers headers
        Ok(response) => return Ok(response)
        Timeout => return Error(APIError::Timeout)
        Error(e) => return Error(APIError::NetworkError(e))
    end match
end fn

fn client_post(client: APIClient, endpoint: ustring, body: ustring): Result<ustring, APIError>
    var url := client.base_url + endpoint
    var headers := [
        (unicode "Authorization", unicode "Bearer " + client.api_key),
        (unicode "Content-Type", unicode "application/json")
    ]

    match post --timeout client.timeout_ms < url with headers headers and body body
        Ok(response) => return Ok(response)
        Timeout => return Error(APIError::Timeout)
        Error(e) => return Error(APIError::NetworkError(e))
    end match
end fn
~~~

---

## 2. Database Integration

### Database Connection

~~~poly
// Database connection
struct Database
    connection_string: ustring
    pool: ConnectionPool
end struct

fn connect_database(config: DatabaseConfig): Result<Database, DBError>
    var connection := try create_connection(config.connection_string)
    var pool := try create_pool(config.max_connections)

    return Ok(Database {
        connection_string: config.connection_string,
        pool: pool
    })
end fn

// Query execution
fn query(db: Database, sql: ustring, params: Vec<ustring>): Result<Vec<Row>, DBError>
    var conn := try db.pool.acquire()
    defer db.pool.release(conn)

    var result := try conn.query(sql, params)
    return Ok(result)
end fn

// Usage
var db := try connect_database(config)
var users := try query(db, unicode "SELECT * FROM users WHERE age > ?", [unicode "18"])
loop: users
    put unicode "User: " + user.name
end loop
~~~

### ORM Integration

~~~poly fragment
// ORM model
struct User
    id: i32
    name: ustring
    email: ustring
    age: i32
end struct

// CRUD operations
fn create_user(db: Database, user: User): Result<User, DBError>
    var sql := unicode "INSERT INTO users (name, email, age) VALUES (?, ?, ?)"
    var result := try db.execute(sql, [user.name, user.email, user.age.to_string()])
    return Ok(User { id: result.last_insert_id, ..user })
end fn

fn get_user(db: Database, id: i32): Result<User, DBError>
    var sql := unicode "SELECT * FROM users WHERE id = ?"
    var rows := try db.query(sql, [id.to_string()])

    if rows.len() == 0,
        return Error(DBError::NotFound)
    end if

    return Ok(rows[0].to_user())
end fn

fn update_user(db: Database, user: User): Result<(), DBError>
    var sql := unicode "UPDATE users SET name = ?, email = ?, age = ? WHERE id = ?"
    try db.execute(sql, [user.name, user.email, user.age.to_string(), user.id.to_string()])
    return Ok(())
end fn

fn delete_user(db: Database, id: i32): Result<(), DBError>
    var sql := unicode "DELETE FROM users WHERE id = ?"
    try db.execute(sql, [id.to_string()])
    return Ok(())
end fn
~~~

---

## 3. Authentication Integration

### OAuth2 Integration

~~~poly fragment
// OAuth2 client
struct OAuth2Client
    client_id: ustring
    client_secret: ustring
    redirect_uri: ustring
    auth_url: ustring
    token_url: ustring
end struct

fn get_authorization_url(client: OAuth2Client, scopes: Vec<ustring>): ustring
    var scope_string := scopes.join(unicode " ")
    return client.auth_url + \\n        unicode "?client_id=" + client.client_id + \\n        unicode "&redirect_uri=" + client.redirect_uri + \\n        unicode "&scope=" + scope_string + \\n        unicode "&response_type=code"
end fn

fn exchange_code(client: OAuth2Client, code: ustring): Result<Token, OAuthError>
    var body := unicode "grant_type=authorization_code" + \\n        unicode "&code=" + code + \\n        unicode "&redirect_uri=" + client.redirect_uri + \\n        unicode "&client_id=" + client.client_id + \\n        unicode "&client_secret=" + client.client_secret

    var response := try http_post(client.token_url, body)
    var token := try parse_json(response)

    return Ok(Token {
        access_token: token.access_token,
        refresh_token: token.refresh_token,
        expires_in: token.expires_in
    })
end fn
~~~

### JWT Authentication

~~~poly
// JWT token handling
fn create_jwt(payload: Map<ustring, ustring>, secret: ustring): ustring
    var header := unicode "{\"alg\":\"HS256\",\"typ\":\"JWT\"}"
    var payload_json := payload.to_json()

    var header_base64 := base64_encode(header)
    var payload_base64 := base64_encode(payload_json)

    var signature := hmac_sha256(header_base64 + "." + payload_base64, secret)
    var signature_base64 := base64_encode(signature)

    return header_base64 + "." + payload_base64 + "." + signature_base64
end fn

fn verify_jwt(token: ustring, secret: ustring): Result<JWTClaims, JWTError>
    var parts := token.split(unicode ".")
    if parts.len() != 3,
        return Error(JWTError::InvalidFormat)
    end if

    var header := base64_decode(parts[0])
    var payload := base64_decode(parts[1])
    var signature := base64_decode(parts[2])

    var expected_signature := hmac_sha256(parts[0] + "." + parts[1], secret)
    if signature != expected_signature,
        return Error(JWTError::InvalidSignature)
    end if

    var claims := try parse_json(payload)
    if claims.exp < get_timestamp() / 1000,
        return Error(JWTError::Expired)
    end if

    return Ok(claims)
end fn
~~~

---

## 4. Cache Integration

### Redis Cache

~~~poly fragment
// Redis client
struct Redis
    host: ustring
    port: i32
    password: ustring
end struct

fn redis_get(redis: Redis, key: ustring): Result<ustring, RedisError>
    var connection := try connect(redis.host, redis.port)
    defer connection.close()

    var response := try connection.send(unicode "GET " + key)
    return Ok(response)
end fn

fn redis_set(redis: Redis, key: ustring, value: ustring, ttl_ms: i32): Result<(), RedisError>
    var connection := try connect(redis.host, redis.port)
    defer connection.close()

    var command := unicode "SET " + key + unicode " " + value
    if ttl_ms > 0,
        command = command + unicode " PX " + ttl_ms.to_string()
    end if

    try connection.send(command)
    return Ok(())
end fn

// Cache wrapper
struct Cache
    redis: Redis
    default_ttl_ms: i32
end struct

fn cache_get(cache: Cache, key: ustring): Result<Option<ustring>, CacheError>
    match redis_get(cache.redis, key)
        Ok(value) => return Ok(Some(value))
        Error(RedisError::KeyNotFound) => return Ok(None)
        Error(e) => return Error(CacheError::RedisError(e))
    end match
end fn

fn cache_set(cache: Cache, key: ustring, value: ustring): Result<(), CacheError>
    try redis_set(cache.redis, key, value, cache.default_ttl_ms)
    return Ok(())
end fn
~~~

---

## 5. Message Queue Integration

### RabbitMQ Integration

~~~poly fragment
// RabbitMQ client
struct RabbitMQ
    host: ustring
    port: i32
    username: ustring
    password: ustring
end struct

fn publish_message(rmq: RabbitMQ, queue: ustring, message: ustring): Result<(), MQError>
    var connection := try connect(rmq.host, rmq.port, rmq.username, rmq.password)
    defer connection.close()

    try connection.publish(queue, message)
    return Ok(())
end fn

fn consume_messages(rmq: RabbitMQ, queue: ustring, handler: fn(ustring) -> Result<(), MQError>): Result<(), MQError>
    var connection := try connect(rmq.host, rmq.port, rmq.username, rmq.password)
    defer connection.close()

    while var message := connection.consume(queue)
        try handler(message)
    end while

    return Ok(())
end fn
~~~

---

## 6. Email Integration

### SMTP Client

~~~poly fragment
// SMTP client
struct SMTP
    host: ustring
    port: i32
    username: ustring
    password: ustring
    use_tls: bool
end struct

fn send_email(smtp: SMTP, to: ustring, subject: ustring, body: ustring): Result<(), EmailError>
    var connection := try connect(smtp.host, smtp.port, smtp.use_tls)
    defer connection.close()

    try connection.authenticate(smtp.username, smtp.password)

    var message := create_message(
        from: smtp.username,
        to: to,
        subject: subject,
        body: body
    )

    try connection.send(message)
    return Ok(())
end fn

// Email template
fn send_template_email(smtp: SMTP, to: ustring, template: ustring, data: Map<ustring, ustring>): Result<(), EmailError>
    var body := render_template(template, data)
    var subject := data.get(unicode "subject") or unicode "No Subject"

    return send_email(smtp, to, subject, body)
end fn
~~~

---

## 7. Storage Integration

### S3 Storage

~~~poly fragment
// S3 client
struct S3
    access_key: ustring
    secret_key: ustring
    region: ustring
    bucket: ustring
end struct

fn s3_upload(s3: S3, key: ustring, data: bytes): Result<(), S3Error>
    var url := unicode "https://" + s3.bucket + unicode ".s3." + s3.region + unicode ".amazonaws.com/" + key
    var signature := calculate_s3_signature(s3, unicode "PUT", url, data)

    var headers := [
        (unicode "Authorization", signature),
        (unicode "Content-Type", unicode "application/octet-stream")
    ]

    match put --timeout 30000 < url with headers headers and body data
        Ok(_) => return Ok(())
        Error(e) => return Error(S3Error::UploadFailed(e))
    end match
end fn

fn s3_download(s3: S3, key: ustring): Result<bytes, S3Error>
    var url := unicode "https://" + s3.bucket + unicode ".s3." + s3.region + unicode ".amazonaws.com/" + key
    var signature := calculate_s3_signature(s3, unicode "GET", url, unicode "")

    var headers := [
        (unicode "Authorization", signature)
    ]

    match get --timeout 30000 < url with headers headers
        Ok(data) => return Ok(data)
        Error(e) => return Error(S3Error::DownloadFailed(e))
    end match
end fn
~~~

---

## 8. Editor Integration (Language Server)

The Poly compiler ships a **language server** (`poly-lsp`) that speaks the Language Server Protocol (LSP) over stdio. Editors can use it to get live diagnostics, keyword and symbol completion, hover information, and document symbols while editing `.poly` files — no editor plugin is required beyond wiring a client to the binary.

### Building and Running

The server is a workspace member, built like the rest of the compiler:

~~~bash
cd compiler
cargo build --release -p poly-lsp
# binary: target/release/poly-lsp
~~~

It runs as a stdio server; the editor starts it and keeps the pipe open. No configuration file or network port is involved.

### Features

| Feature | LSP method | What it provides |
|---------|------------|------------------|
| Diagnostics | `textDocument/publishDiagnostics` | Lexer, parser, and type-checker errors with precise line/column ranges |
| Completion | `textDocument/completion` | Poly keywords plus symbols declared in the open document |
| Hover | `textDocument/hover` | Declaration detail for functions, structs, and enums |
| Symbols | `textDocument/documentSymbol` | Top-level functions, structs, enums, traits, and impls |

Diagnostics run every compiler phase (lex → parse with recovery → type check), so a single save surfaces syntax and semantic errors together. Completion is triggered by typing and also by `.` and `:` characters.

### VS Code

The repository ships a ready-made extension in the `vscode/` directory. It provides a TextMate grammar (`source.poly`), language configuration (comments, brackets, indentation), and a language-client extension that launches `poly-lsp` automatically with diagnostics, completion, hover, and document symbols. To use it:

~~~bash
cd vscode
npm install
# Run from VS Code (F5, "Extension Development Host") or package:
# npx @vscode/vsce package
~~~

Set the `poly.lsp.path` setting if `poly-lsp` is not on `PATH`, e.g. `compiler/target/release/poly-lsp`. The extension also registers a `Poly: Restart Language Server` command.

Alternatively, install any LSP client extension (e.g. **vscode-languageclient** or **LSP-client**) and register `poly-lsp` as the server for the `poly` language. With a custom client extension, the activation looks like:

~~~json
{
  "activationEvents": ["onLanguage:poly"],
  "contributes": {
    "languages": [
      {
        "id": "poly",
        "extensions": [".poly"],
        "aliases": ["Poly"],
        "configuration": "./language-configuration.json"
      }
    ]
  }
}
~~~

In your extension's `activate()` function, start the server with:

~~~ts
import { LanguageClient, ServerOptions, TransportKind } from 'vscode-languageclient/node';

const serverOptions: ServerOptions = {
  command: 'poly-lsp',
  transport: TransportKind.stdio,
};

const client = new LanguageClient('polyLsp', 'Poly Language Server', serverOptions, {
  documentSelector: [{ scheme: 'file', language: 'poly' }],
});

client.start();
~~~

### Neovim (nvim-lspconfig)

Neovim 0.8+ can attach the server with a small `lspconfig`-style config. Either add a custom config or use `vim.lsp.start` directly:

~~~lua
vim.api.nvim_create_autocmd('FileType', {
  pattern = 'poly',
  callback = function()
    vim.lsp.start({
      name = 'poly-lsp',
      cmd = { 'poly-lsp' },
      root_dir = vim.fs.dirname(vim.fs.find({ 'Cargo.toml', '.git' }, { upward = true })[1]),
      capabilities = vim.lsp.protocol.make_client_capabilities(),
    })
  end,
})

-- Associate *.poly with the poly filetype if your distribution lacks it.
vim.filetype.add({ extension = { poly = 'poly' } })
~~~

If `poly-lsp` is not on `PATH`, give the full path to the compiled binary, e.g. `cmd = { '/home/you/poly/compiler/target/release/poly-lsp' }`.

### Helix and Other Editors

Helix and other LSP-native editors accept a language configuration pointing at the server binary:

~~~toml
[[language]]
name = "poly"
scope = "source.poly"
injection-regex = "poly"
file-types = ["poly"]
comment-tokens = "#"
language-servers = ["poly-lsp"]

[language-server.poly-lsp]
command = "poly-lsp"
~~~

### Notes and Limitations

- The server is dependency-free and intentionally small: full-text document synchronization only, no incremental sync yet.
- Type-check diagnostics locate the *declaring statement* via the statement spans now retained in the AST; complex expressions fall back to a text scan for the offending symbol.
- Unsupported LSP requests are ignored gracefully, so newer clients remain compatible.

---

## Summary

1. **HTTP Client**: Use `get`, `post` with timeout and error handling
2. **Database**: Use connection pooling and parameterized queries
3. **Authentication**: Implement OAuth2 and JWT properly
4. **Cache**: Use Redis for distributed caching
5. **Message Queue**: Use RabbitMQ for async messaging
6. **Email**: Use SMTP for email delivery
7. **Storage**: Use S3 for object storage
8. **Editor Integration**: Run `poly-lsp` from VS Code, Neovim, or Helix for diagnostics, completion, hover, and symbols
