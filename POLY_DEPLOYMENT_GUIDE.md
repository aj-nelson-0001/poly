# Poly Language Deployment Guide

## Overview

This guide covers deployment best practices for Poly applications using the new I/O and error handling syntax.

**Note:** Some functions used in this guide (like `get_env()`, `set_env()`, `get_timestamp()`, `get_version()`) are standard library functions. See the Standard Library section for details.

---

## 1. Build Process

### Production Build

~~~poly
// Build configuration
enum BuildMode
    Debug
    Release
end enum

fn build_project(mode: BuildMode): Result<ustring, BuildError>
    match mode
        Debug,
            put "Building in debug mode..."
            return try compile_with_debug()
        Release,
            put "Building in release mode..."
            return try compile_with_optimizations()
    end match
end fn

// Optimize for production
fn compile_with_optimizations(): Result<ustring, BuildError>
    var flags Vec<ustring> := [
        unicode "--release",
        unicode "--optimize",
        unicode "--strip-debug",
        unicode "--compress"
    ]
    return try run_compiler(flags)
end fn
~~~

### Build Scripts

~~~poly
// build.poly
fn main()
    put "Starting build..."

    // Clean previous build
    put "Cleaning..."
    delete_dir("build/")

    // Compile
    put "Compiling..."
    var result := try compile_project()

    // Test
    put "Testing..."
    var tests := try run_tests()

    // Package
    put "Packaging..."
    var package := try create_package()

    put "Build complete!"
    put "Output: " + package
end fn
~~~

---

## 2. Configuration Management

### Environment Variables

~~~poly fragment
// Configuration loading
fn load_config(): Result<Config, ConfigError>
    var config Config := {
        database_url: get_env("DATABASE_URL") or unicode "localhost",
        port: get_env("PORT") or unicode "8080",
        debug: get_env("DEBUG") or unicode "false",
        log_level: get_env("LOG_LEVEL") or unicode "info"
    }

    // Validate configuration
    if config.port.parse::<i32>().is_err(),
        return Error(ConfigError::InvalidPort)
    end if

    return Ok(config)
end fn

// Usage
match load_config()
    Ok(config), start_server(config)
    Error(e), error "Failed to load config: " + e.to_string()
end match
~~~

### Environment-Specific Configuration

~~~poly
// config.poly
enum Environment
    Development
    Staging
    Production
end enum

fn get_config(env: Environment): Config
    match env
        Development,
            return Config {
                database_url: unicode "localhost:5432/dev",
                debug: true,
                log_level: unicode "debug"
            }
        Staging,
            return Config {
                database_url: unicode "staging-db:5432/staging",
                debug: false,
                log_level: unicode "info"
            }
        Production,
            return Config {
                database_url: unicode "prod-db:5432/prod",
                debug: false,
                log_level: unicode "warn"
            }
    end match
end fn
~~~

---

## 3. Logging and Monitoring

### Structured Logging

~~~poly fragment
// Logging setup
fn setup_logging(level: ustring)
    match level
        unicode "debug", set_log_level(LogLevel::Debug)
        unicode "info", set_log_level(LogLevel::Info)
        unicode "warn", set_log_level(LogLevel::Warn)
        unicode "error", set_log_level(LogLevel::Error)
        _, set_log_level(LogLevel::Info)
    end match
end fn

// Request logging
fn log_request(method: ustring, path: ustring, status: i32, duration_ms: i32)
    var log_entry ustring := {
        timestamp: get_timestamp(),
        method: method,
        path: path,
        status: status,
        duration_ms: duration_ms
    }

    put log_entry.to_json() >> "access.log"
end fn

// Error logging
fn log_error(error: Error, context: ustring)
    var log_entry ustring := {
        timestamp: get_timestamp(),
        level: "ERROR",
        context: context,
        message: error.message,
        stack_trace: error.stack_trace
    }

    put log_entry.to_json() >> "error.log"
    error "Error in " + context + ": " + error.message
end fn
~~~

### Health Checks

~~~poly fragment
// Health check endpoint
fn health_check(): Result<HealthStatus, ustring>
    var status HealthStatus := {
        status: unicode "healthy",
        timestamp: get_timestamp(),
        version: get_version(),
        checks: []
    }

    // Check database
    match check_database()
        Ok(_), status.checks.push(unicode "database: ok")
        Error(e),
            status.checks.push(unicode "database: " + e)
            status.status = unicode "unhealthy"
    end match

    // Check cache
    match check_cache()
        Ok(_), status.checks.push(unicode "cache: ok")
        Error(e),
            status.checks.push(unicode "cache: " + e)
            status.status = unicode "degraded"
    end match

    return Ok(status)
end fn
~~~

---

## 4. Error Handling in Production

### Graceful Shutdown

~~~poly
// Graceful shutdown handler
fn setup_signal_handlers()
    on_signal(Signal::SIGTERM, shutdown_handler)
    on_signal(Signal::SIGINT, shutdown_handler)
end fn

fn shutdown_handler()
    put "Shutting down gracefully..."

    // Stop accepting new connections
    stop_server()

    // Wait for existing requests to complete
    wait_for_requests()

    // Close database connections
    close_database()

    // Close file handles
    close_file_handles()

    put "Shutdown complete"
    exit(0)
end fn
~~~

### Circuit Breaker Pattern

~~~poly fragment
// Circuit breaker for external services
enum CircuitState
    Closed
    Open
    HalfOpen
end enum

struct CircuitBreaker
    state: CircuitState
    failure_count: i32
    last_failure_time: i64
    success_count: i32
end struct

fn circuit_breaker_call(cb: CircuitBreaker, service: fn(): Result<T, E>): Result<T, E>
    match cb.state
        Closed,
            match service()
                Ok(result),
                    cb.success_count = cb.success_count + 1
                    if cb.success_count >= 5,
                        cb.failure_count = 0
                    end if
                    return Ok(result)
                Error(e),
                    cb.failure_count = cb.failure_count + 1
                    cb.last_failure_time = get_timestamp()
                    if cb.failure_count >= 3,
                        cb.state = Open
                    end if
                    return Error(e)
            end match
        Open,
            if get_timestamp() - cb.last_failure_time > 30000,
                cb.state = HalfOpen
                return circuit_breaker_call(cb, service)
            else
                return Error(unicode "Circuit breaker is open")
            end if
        HalfOpen,
            match service()
                Ok(result),
                    cb.state = Closed
                    cb.failure_count = 0
                    return Ok(result)
                Error(e),
                    cb.state = Open
                    cb.last_failure_time = get_timestamp()
                    return Error(e)
            end match
    end match
end fn
~~~

---

## 5. Performance Optimization

### Caching

~~~poly fragment
// Simple cache
struct Cache<T>
    data: Map<ustring, (T, i64)>
    ttl_ms: i64
end struct

fn cache_get<T>(cache: Cache<T>, key: ustring): Option<T>
    match cache.data.get(key)
        Some((value, expiry)),
            if get_timestamp() < expiry,
                return Some(value)
            else
                cache.data.remove(key)
                return None
            end if
        None, return None
    end match
end fn

fn cache_set<T>(cache: Cache<T>, key: ustring, value: T)
    var expiry i64 := get_timestamp() + cache.ttl_ms
    cache.data.insert(key, (value, expiry))
end fn
~~~

### Connection Pooling

~~~poly
// Database connection pool
struct ConnectionPool
    connections: Vec<Connection>
    max_size: i32
    available: i32
end struct

fn pool_acquire(pool: ConnectionPool): Result<Connection, PoolError>
    if pool.available > 0,
        // Get available connection
        var conn := pool.connections.pop()
        pool.available = pool.available - 1
        return Ok(conn)
    else if pool.connections.len() < pool.max_size,
        // Create new connection
        var conn := try create_connection()
        return Ok(conn)
    else
        // Wait for available connection
        return Error(PoolError::PoolExhausted)
    end if
end fn

fn pool_release(pool: ConnectionPool, conn: Connection)
    pool.connections.push(conn)
    pool.available = pool.available + 1
end fn
~~~

---

## 6. Security in Production

### HTTPS Configuration

~~~poly
// HTTPS server setup
fn create_https_server(config: Config): Result<Server, ServerError>
    var server := try create_server(config.port)

    // Load SSL certificates
    var cert := try load_certificate(config.cert_path)
    var key := try load_private_key(config.key_path)

    // Configure TLS
    server.set_tls(cert, key)
    server.set_min_tls_version(TlsVersion::Tls12)

    return Ok(server)
end fn
~~~

### Rate Limiting

~~~poly fragment
// Rate limiter
struct RateLimiter
    requests: Map<ustring, Vec<i64>>
    max_requests: i32
    window_ms: i64
end struct

fn rate_limit_check(limiter: RateLimiter, client_id: ustring): bool
    var now := get_timestamp()
    var requests := limiter.requests.get(client_id) or Vec::new()

    // Remove old requests
    requests = requests.filter(|t| now - t < limiter.window_ms)

    if requests.len() >= limiter.max_requests,
        return false  // Rate limit exceeded
    end if

    requests.push(now)
    limiter.requests.insert(client_id, requests)
    return true
end fn
~~~

---

## 7. Deployment Strategies

### Blue-Green Deployment

~~~poly fragment
// Blue-green deployment
fn deploy_blue_green(new_version: ustring): Result<(), DeployError>
    // Deploy to green environment
    put "Deploying to green environment..."
    try deploy_to_green(new_version)

    // Test green environment
    put "Testing green environment..."
    try test_green_environment()

    // Switch traffic
    put "Switching traffic to green..."
    try switch_traffic(unicode "green")

    // Keep blue as backup
    put "Deployment complete"
    put "Blue environment kept as backup"

    return Ok(())
end fn
~~~

### Rolling Deployment

~~~poly fragment
// Rolling deployment
fn deploy_rolling(new_version: ustring, batch_size: i32): Result<(), DeployError>
    var instances := get_all_instances()
    var batches := instances.chunks(batch_size)

    loop: batch in batches
        put "Deploying batch: " + batch.to_string()

        // Deploy to batch
        try deploy_to_batch(batch, new_version)

        // Wait for health check
        wait_for_health_check(batch)

        // Verify deployment
        try verify_deployment(batch)
    end loop

    put "Rolling deployment complete"
    return Ok(())
end fn
~~~

---

## 8. Monitoring and Alerting

### Metrics Collection

~~~poly
// Metrics collector
struct Metrics
    request_count: i64
    error_count: i64
    response_time_ms: Vec<i64>
    active_connections: i32
end struct

fn collect_metrics(metrics: Metrics)
    // Request rate
    var request_rate := metrics.request_count / 60
    put "request_rate: " + request_rate.to_string()

    // Error rate
    var error_rate := (metrics.error_count as f64) / (metrics.request_count as f64) * 100.0
    put "error_rate: " + error_rate.to_string() + "%"

    // Response time
    var avg_response_time := metrics.response_time_ms.iter().sum() / metrics.response_time_ms.len()
    put "avg_response_time: " + avg_response_time.to_string() + "ms"

    // Active connections
    put "active_connections: " + metrics.active_connections.to_string()
end fn
~~~

### Alerting

~~~poly
// Alert manager
fn check_alerts(metrics: Metrics)
    // High error rate
    var error_rate := (metrics.error_count as f64) / (metrics.request_count as f64) * 100.0
    if error_rate > 5.0,
        send_alert(unicode "High error rate: " + error_rate.to_string() + "%")
    end if

    // High response time
    var avg_response_time := metrics.response_time_ms.iter().sum() / metrics.response_time_ms.len()
    if avg_response_time > 1000,
        send_alert(unicode "High response time: " + avg_response_time.to_string() + "ms")
    end if

    // Low active connections
    if metrics.active_connections < 10,
        send_alert(unicode "Low active connections: " + metrics.active_connections.to_string())
    end if
end fn
~~~

---

## Summary

1. **Build Process**: Optimize for production, use build scripts
2. **Configuration**: Use environment variables, environment-specific configs
3. **Logging**: Use structured logging, implement health checks
4. **Error Handling**: Implement graceful shutdown, circuit breaker pattern
5. **Performance**: Use caching, connection pooling
6. **Security**: Configure HTTPS, implement rate limiting
7. **Deployment**: Use blue-green or rolling deployment strategies
8. **Monitoring**: Collect metrics, implement alerting
