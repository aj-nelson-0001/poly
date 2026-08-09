# Poly Language External Services Integration Guide

## Overview

This guide covers integrating with external services in Poly applications using the new I/O and error handling syntax.

**Note:** Some functions used in this guide (like `base64_encode()`, `hmac_sha256()`, `connect()`, `post_request()`) are standard library functions or require external dependencies. See the Standard Library section for details.

---

## 1. HTTP Client Integration

### Basic HTTP Requests

```poly\n// HTTP client\nfn http_get(url: ustring): Result<ustring, HttpError>\n    match get --timeout 10000 < url\n        Ok(response) => return Ok(response)\n        Timeout => return Error(HttpError::Timeout)\n        Error(e) => return Error(HttpError::NetworkError(e))\n    end match\nend fn\n\nfn http_post(url: ustring, body: ustring): Result<ustring, HttpError>\n    // POST request implementation\n    var response = try post_request(url, body)\n    return Ok(response)\nend fn\n\n// Usage\nmatch http_get(u\"https://api.example.com/data\")\n    Ok(data) => process(data)\n    Error(HttpError::Timeout) => warn \"Request timed out\"\n    Error(HttpError::NetworkError(e)) => error \"Network error: \" + e\nend match\n```

### REST API Client

```poly\n// REST API client\nstruct APIClient\n    base_url: ustring\n    api_key: ustring\n    timeout_ms: i32\nend struct\n\nfn client_get(client: APIClient, endpoint: ustring): Result<ustring, APIError>\n    var url = client.base_url + endpoint\n    var headers = [\n        (u\"Authorization\", u\"Bearer \" + client.api_key),\n        (u\"Content-Type\", u\"application/json\")\n    ]\n    \n    match get --timeout client.timeout_ms < url with headers headers\n        Ok(response) => return Ok(response)\n        Timeout => return Error(APIError::Timeout)\n        Error(e) => return Error(APIError::NetworkError(e))\n    end match\nend fn\n\nfn client_post(client: APIClient, endpoint: ustring, body: ustring): Result<ustring, APIError>\n    var url = client.base_url + endpoint\n    var headers = [\n        (u\"Authorization\", u\"Bearer \" + client.api_key),\n        (u\"Content-Type\", u\"application/json\")\n    ]\n    \n    match post --timeout client.timeout_ms < url with headers headers and body body\n        Ok(response) => return Ok(response)\n        Timeout => return Error(APIError::Timeout)\n        Error(e) => return Error(APIError::NetworkError(e))\n    end match\nend fn\n```

---

## 2. Database Integration

### Database Connection

```poly\n// Database connection\nstruct Database\n    connection_string: ustring\n    pool: ConnectionPool\nend struct\n\nfn connect_database(config: DatabaseConfig): Result<Database, DBError>\n    var connection = try create_connection(config.connection_string)\n    var pool = try create_pool(config.max_connections)\n    \n    return Ok(Database {\n        connection_string: config.connection_string,\n        pool: pool\n    })\nend fn\n\n// Query execution\nfn query(db: Database, sql: ustring, params: Vec<ustring>): Result<Vec<Row>, DBError>\n    var conn = try db.pool.acquire()\n    defer db.pool.release(conn)\n    \n    var result = try conn.query(sql, params)\n    return Ok(result)\nend fn\n\n// Usage\nvar db = try connect_database(config)\nvar users = try query(db, u\"SELECT * FROM users WHERE age > ?\", [u\"18\"])\nloop: users\n    put u\"User: \" + user.name\nend loop\n```

### ORM Integration

```poly\n// ORM model\nstruct User\n    id: i32\n    name: ustring\n    email: ustring\n    age: i32\nend struct\n\n// CRUD operations\nfn create_user(db: Database, user: User): Result<User, DBError>\n    var sql = u\"INSERT INTO users (name, email, age) VALUES (?, ?, ?)\"\n    var result = try db.execute(sql, [user.name, user.email, user.age.to_string()])\n    return Ok(User { id: result.last_insert_id, ..user })\nend fn\n\nfn get_user(db: Database, id: i32): Result<User, DBError>\n    var sql = u\"SELECT * FROM users WHERE id = ?\"\n    var rows = try db.query(sql, [id.to_string()])\n    \n    if rows.len() == 0,\n        return Error(DBError::NotFound)\n    end if\n    \n    return Ok(rows[0].to_user())\nend fn\n\nfn update_user(db: Database, user: User): Result<(), DBError>\n    var sql = u\"UPDATE users SET name = ?, email = ?, age = ? WHERE id = ?\"\n    try db.execute(sql, [user.name, user.email, user.age.to_string(), user.id.to_string()])\n    return Ok(())\nend fn\n\nfn delete_user(db: Database, id: i32): Result<(), DBError>\n    var sql = u\"DELETE FROM users WHERE id = ?\"\n    try db.execute(sql, [id.to_string()])\n    return Ok(())\nend fn\n```

---

## 3. Authentication Integration

### OAuth2 Integration

```poly\n// OAuth2 client\nstruct OAuth2Client\n    client_id: ustring\n    client_secret: ustring\n    redirect_uri: ustring\n    auth_url: ustring\n    token_url: ustring\nend struct\n\nfn get_authorization_url(client: OAuth2Client, scopes: Vec<ustring>): ustring\n    var scope_string = scopes.join(u\" \")\n    return client.auth_url + \\\n        u\"?client_id=\" + client.client_id + \\\n        u\"&redirect_uri=\" + client.redirect_uri + \\\n        u\"&scope=\" + scope_string + \\\n        u\"&response_type=code\"\nend fn\n\nfn exchange_code(client: OAuth2Client, code: ustring): Result<Token, OAuthError>\n    var body = u\"grant_type=authorization_code\" + \\\n        u\"&code=\" + code + \\\n        u\"&redirect_uri=\" + client.redirect_uri + \\\n        u\"&client_id=\" + client.client_id + \\\n        u\"&client_secret=\" + client.client_secret\n    \n    var response = try http_post(client.token_url, body)\n    var token = try parse_json(response)\n    \n    return Ok(Token {\n        access_token: token.access_token,\n        refresh_token: token.refresh_token,\n        expires_in: token.expires_in\n    })\nend fn\n```

### JWT Authentication

```poly\n// JWT token handling\nfn create_jwt(payload: Map<ustring, ustring>, secret: ustring): ustring\n    var header = u\"{\\\"alg\\\":\\\"HS256\\\",\\\"typ\\\":\\\"JWT\\\"}\"\n    var payload_json = payload.to_json()\n    \n    var header_base64 = base64_encode(header)\n    var payload_base64 = base64_encode(payload_json)\n    \n    var signature = hmac_sha256(header_base64 + \".\" + payload_base64, secret)\n    var signature_base64 = base64_encode(signature)\n    \n    return header_base64 + \".\" + payload_base64 + \".\" + signature_base64\nend fn\n\nfn verify_jwt(token: ustring, secret: ustring): Result<JWTClaims, JWTError>\n    var parts = token.split(u\".\")\n    if parts.len() != 3,\n        return Error(JWTError::InvalidFormat)\n    end if\n    \n    var header = base64_decode(parts[0])\n    var payload = base64_decode(parts[1])\n    var signature = base64_decode(parts[2])\n    \n    var expected_signature = hmac_sha256(parts[0] + \".\" + parts[1], secret)\n    if signature != expected_signature,\n        return Error(JWTError::InvalidSignature)\n    end if\n    \n    var claims = try parse_json(payload)\n    if claims.exp < get_timestamp() / 1000,\n        return Error(JWTError::Expired)\n    end if\n    \n    return Ok(claims)\nend fn\n```

---

## 4. Cache Integration

### Redis Cache

```poly\n// Redis client\nstruct Redis\n    host: ustring\n    port: i32\n    password: ustring\nend struct\n\nfn redis_get(redis: Redis, key: ustring): Result<ustring, RedisError>\n    var connection = try connect(redis.host, redis.port)\n    defer connection.close()\n    \n    var response = try connection.send(u\"GET \" + key)\n    return Ok(response)\nend fn\n\nfn redis_set(redis: Redis, key: ustring, value: ustring, ttl_ms: i32): Result<(), RedisError>\n    var connection = try connect(redis.host, redis.port)\n    defer connection.close()\n    \n    var command = u\"SET \" + key + u\" \" + value\n    if ttl_ms > 0,\n        command = command + u\" PX \" + ttl_ms.to_string()\n    end if\n    \n    try connection.send(command)\n    return Ok(())\nend fn\n\n// Cache wrapper\nstruct Cache\n    redis: Redis\n    default_ttl_ms: i32\nend struct\n\nfn cache_get(cache: Cache, key: ustring): Result<Option<ustring>, CacheError>\n    match redis_get(cache.redis, key)\n        Ok(value) => return Ok(Some(value))\n        Error(RedisError::KeyNotFound) => return Ok(None)\n        Error(e) => return Error(CacheError::RedisError(e))\n    end match\nend fn\n\nfn cache_set(cache: Cache, key: ustring, value: ustring): Result<(), CacheError>\n    try redis_set(cache.redis, key, value, cache.default_ttl_ms)\n    return Ok(())\nend fn\n```

---

## 5. Message Queue Integration

### RabbitMQ Integration

```poly\n// RabbitMQ client\nstruct RabbitMQ\n    host: ustring\n    port: i32\n    username: ustring\n    password: ustring\nend struct\n\nfn publish_message(rmq: RabbitMQ, queue: ustring, message: ustring): Result<(), MQError>\n    var connection = try connect(rmq.host, rmq.port, rmq.username, rmq.password)\n    defer connection.close()\n    \n    try connection.publish(queue, message)\n    return Ok(())\nend fn\n\nfn consume_messages(rmq: RabbitMQ, queue: ustring, handler: fn(ustring) -> Result<(), MQError>): Result<(), MQError>\n    var connection = try connect(rmq.host, rmq.port, rmq.username, rmq.password)\n    defer connection.close()\n    \n    while var message = connection.consume(queue)\n        try handler(message)\n    end while\n    \n    return Ok(())\nend fn\n```

---

## 6. Email Integration

### SMTP Client

```poly\n// SMTP client\nstruct SMTP\n    host: ustring\n    port: i32\n    username: ustring\n    password: ustring\n    use_tls: bool\nend struct\n\nfn send_email(smtp: SMTP, to: ustring, subject: ustring, body: ustring): Result<(), EmailError>\n    var connection = try connect(smtp.host, smtp.port, smtp.use_tls)\n    defer connection.close()\n    \n    try connection.authenticate(smtp.username, smtp.password)\n    \n    var message = create_message(\n        from: smtp.username,\n        to: to,\n        subject: subject,\n        body: body\n    )\n    \n    try connection.send(message)\n    return Ok(())\nend fn\n\n// Email template\nfn send_template_email(smtp: SMTP, to: ustring, template: ustring, data: Map<ustring, ustring>): Result<(), EmailError>\n    var body = render_template(template, data)\n    var subject = data.get(u\"subject\") or u\"No Subject\"\n    \n    return send_email(smtp, to, subject, body)\nend fn\n```

---

## 7. Storage Integration

### S3 Storage

```poly\n// S3 client\nstruct S3\n    access_key: ustring\n    secret_key: ustring\n    region: ustring\n    bucket: ustring\nend struct\n\nfn s3_upload(s3: S3, key: ustring, data: bytes): Result<(), S3Error>\n    var url = u\"https://\" + s3.bucket + u\".s3.\" + s3.region + u\".amazonaws.com/\" + key\n    var signature = calculate_s3_signature(s3, u\"PUT\", url, data)\n    \n    var headers = [\n        (u\"Authorization\", signature),\n        (u\"Content-Type\", u\"application/octet-stream\")\n    ]\n    \n    match put --timeout 30000 < url with headers headers and body data\n        Ok(_) => return Ok(())\n        Error(e) => return Error(S3Error::UploadFailed(e))\n    end match\nend fn\n\nfn s3_download(s3: S3, key: ustring): Result<bytes, S3Error>\n    var url = u\"https://\" + s3.bucket + u\".s3.\" + s3.region + u\".amazonaws.com/\" + key\n    var signature = calculate_s3_signature(s3, u\"GET\", url, u\"\")\n    \n    var headers = [\n        (u\"Authorization\", signature)\n    ]\n    \n    match get --timeout 30000 < url with headers headers\n        Ok(data) => return Ok(data)\n        Error(e) => return Error(S3Error::DownloadFailed(e))\n    end match\nend fn\n```

---

## Summary

1. **HTTP Client**: Use `get`, `post` with timeout and error handling
2. **Database**: Use connection pooling and parameterized queries
3. **Authentication**: Implement OAuth2 and JWT properly
4. **Cache**: Use Redis for distributed caching
5. **Message Queue**: Use RabbitMQ for async messaging
6. **Email**: Use SMTP for email delivery
7. **Storage**: Use S3 for object storage
