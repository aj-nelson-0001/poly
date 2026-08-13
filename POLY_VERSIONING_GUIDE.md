# Poly Language API Versioning Guide

## Overview

This guide covers API versioning best practices for Poly applications using the new I/O and error handling syntax.

**Note:** Some functions used in this guide (like `parse_version()`, `compare_versions()`, `transform_v1_to_v2()`) are standard library functions. See the Standard Library section for details.

---

## 1. Version Numbering

### Semantic Versioning

~~~poly fragment
// Version structure
struct Version
    major: i32
    minor: i32
    patch: i32
end struct

// Parse version string
fn parse_version(version_str: ustring): Result<Version, VersionError>
    var parts Vec<ustring> := version_str.split(unicode ".")
    if parts.len() != 3,
        return Error(VersionError::InvalidFormat)
    end if

    var major := try parts[0].parse::<i32>()
    var minor := try parts[1].parse::<i32>()
    var patch := try parts[2].parse::<i32>()

    return Ok(Version { major: major, minor: minor, patch: patch })
end fn

// Compare versions
fn compare_versions(v1: Version, v2: Version): i32
    if v1.major != v2.major,
        return v1.major - v2.major
    else if v1.minor != v2.minor,
        return v1.minor - v2.minor
    else
        return v1.patch - v2.patch
    end if
end fn

// Check compatibility
fn is_compatible(current: Version, required: Version): bool
    // Major version must match
    if current.major != required.major,
        return false
    end if

    // Minor version must be >= required
    if current.minor < required.minor,
        return false
    end if

    // If minor versions match, patch must be >= required
    if current.minor == required.minor && current.patch < required.patch,
        return false
    end if

    return true
end fn
~~~

### Version String Format

~~~poly
// Version string format: MAJOR.MINOR.PATCH
var version ustring := unicode "1.2.3"

// Pre-release versions
var pre_release ustring := unicode "1.2.3-alpha.1"
var beta ustring := unicode "1.2.3-beta.2"
var rc ustring := unicode "1.2.3-rc.1"

// Build metadata
var build ustring := unicode "1.2.3+build.123"

// Parse version with pre-release and build
fn parse_full_version(version_str: ustring): Result<FullVersion, VersionError>
    // Split on + for build metadata
    var build_parts := version_str.split(unicode "+")
    var version_part := build_parts[0]
    var build_metadata := if build_parts.len() > 1,build_parts[1] else unicode "" end if

    // Split on - for pre-release
    var pre_parts := version_part.split(unicode "-")
    var core_version := pre_parts[0]
    var pre_release := if pre_parts.len() > 1,pre_parts[1] else unicode "" end if

    var version := try parse_version(core_version)

    return Ok(FullVersion {
        version: version,
        pre_release: pre_release,
        build_metadata: build_metadata
    })
end fn
~~~

---

## 2. API Version Management

### Version Header

~~~poly fragment
// API version in request header
fn get_api_version(request: Request): ustring
    return request.headers.get(unicode "X-API-Version") or unicode "1.0"
end fn

// API version in response header
fn set_api_version(response: Response, version: ustring)
    response.headers.set(unicode "X-API-Version", version)
    response.headers.set(unicode "X-API-Compatible-With", unicode "1.0-2.0")
end fn
~~~

### Version Routing

~~~poly
// Route based on API version
fn route_request(request: Request): Response
    var version := get_api_version(request)

    match version
        unicode "1.0" => return handle_v1(request)
        unicode "1.1" => return handle_v1_1(request)
        unicode "2.0" => return handle_v2(request)
        _ =>
            var response := Response()
            response.status = 400
            response.body = unicode "Unsupported API version"
            return response
    end match
end fn

// Version-specific handlers
fn handle_v1(request: Request): Response
    // V1 logic
    return Response { status: 200, body: unicode "V1 response" }
end fn

fn handle_v1_1(request: Request): Response
    // V1.1 logic (backward compatible with V1)
    return Response { status: 200, body: unicode "V1.1 response" }
end fn

fn handle_v2(request: Request): Response
    // V2 logic (may not be backward compatible)
    return Response { status: 200, body: unicode "V2 response" }
end fn
~~~

---

## 3. Backward Compatibility

### Deprecation Warnings

~~~poly
// Deprecation warning function
fn warn_deprecated(feature: ustring, version: ustring, alternative: ustring)
    warn "DEPRECATED: " + feature + " is deprecated since version " + version
    warn "Use " + alternative + " instead"
end fn

// Usage
fn old_function()
    warn_deprecated(unicode "old_function()", unicode "1.5", unicode "new_function()")
    // Old implementation
end fn

fn new_function()
    // New implementation
end fn
~~~

### Version Compatibility Layer

~~~poly
// Compatibility layer for older versions
fn compatibility_layer(request: Request, target_version: ustring): Request
    var current_version := get_api_version(request)

    if compare_versions(parse_version(current_version), parse_version(target_version)) < 0,
        // Transform request to target version format
        return transform_request(request, current_version, target_version)
    end if

    return request
end fn

// Request transformation
fn transform_request(request: Request, from_version: ustring, to_version: ustring): Request
    var transformed := request.clone()

    // Transform based on version difference
    if from_version == unicode "1.0" && to_version == unicode "2.0",
        // V1 to V2 transformation
        transformed.body = transform_v1_to_v2(request.body)
    end if

    return transformed
end fn
~~~

---

## 4. Version Negotiation

### Content Negotiation

~~~poly fragment
// Negotiate API version from Accept header
fn negotiate_version(request: Request): ustring
    var accept := request.headers.get(unicode "Accept") or unicode ""

    // Parse Accept header for version
    if accept.contains(unicode "application/vnd.api.v2+json"),
        return unicode "2.0"
    else if accept.contains(unicode "application/vnd.api.v1+json"),
        return unicode "1.0"
    else
        return unicode "1.0"  // Default version
    end if
end fn

// Response with content negotiation
fn respond_with_version(request: Request, data: ustring): Response
    var version := negotiate_version(request)
    var response := Response()

    match version
        unicode "1.0" =>
            response.headers.set(unicode "Content-Type", unicode "application/vnd.api.v1+json")
            response.body = transform_to_v1(data)
        unicode "2.0" =>
            response.headers.set(unicode "Content-Type", unicode "application/vnd.api.v2+json")
            response.body = transform_to_v2(data)
    end match

    set_api_version(response, version)
    return response
end fn
~~~

### URL Versioning

~~~poly
// URL-based versioning
fn route_by_url(request: Request): Response
    var path := request.path

    // Extract version from URL
    if path.starts_with(unicode "/api/v2/"),
        return handle_v2(request)
    else if path.starts_with(unicode "/api/v1/"),
        return handle_v1(request)
    else
        return handle_v1(request)  // Default to V1
    end if
end fn
~~~

---

## 5. API Documentation

### Version Documentation

~~~poly
// API documentation structure
struct APIDocumentation
    version: ustring
    title: ustring
    description: ustring
    endpoints: Vec<Endpoint>
    deprecated: Vec<DeprecatedEndpoint>
end struct

// Generate documentation
fn generate_docs(version: ustring): APIDocumentation
    var docs := APIDocumentation {
        version: version,
        title: unicode "Poly API",
        description: unicode "API for Poly applications",
        endpoints: get_endpoints(version),
        deprecated: get_deprecated_endpoints(version)
    }

    return docs
end fn

// Get endpoints for version
fn get_endpoints(version: ustring): Vec<Endpoint>
    var endpoints Vec<Endpoint> := []

    // Add version-specific endpoints
    match version
        unicode "1.0" =>
            endpoints.push(Endpoint { path: unicode "/users", method: unicode "GET" })
            endpoints.push(Endpoint { path: unicode "/users", method: unicode "POST" })
        unicode "2.0" =>
            endpoints.push(Endpoint { path: unicode "/users", method: unicode "GET" })
            endpoints.push(Endpoint { path: unicode "/users", method: unicode "POST" })
            endpoints.push(Endpoint { path: unicode "/users/{id}", method: unicode "PUT" })
            endpoints.push(Endpoint { path: unicode "/users/{id}", method: unicode "DELETE" })
    end match

    return endpoints
end fn
~~~

---

## 6. Migration Strategies

### Automated Migration

~~~poly fragment
// Automated migration script
fn migrate_api(old_version: ustring, new_version: ustring): Result<(), MigrationError>
    put "Migrating from " + old_version + " to " + new_version

    // Backup current data
    try backup_data()

    // Run migration steps
    match (old_version, new_version)
        (unicode "1.0", unicode "2.0") => try migrate_v1_to_v2()
        (unicode "1.1", unicode "2.0") => try migrate_v1_1_to_v2()
        _ => return Error(MigrationError::UnsupportedMigration)
    end match

    // Verify migration
    try verify_migration()

    put "Migration complete"
    return Ok(())
end fn

// Migration steps
fn migrate_v1_to_v2(): Result<(), MigrationError>
    put "Step 1: Migrating user data..."
    try migrate_user_data()

    put "Step 2: Migrating order data..."
    try migrate_order_data()

    put "Step 3: Updating indexes..."
    try update_indexes()

    return Ok(())
end fn
~~~

### Rollback Strategy

~~~poly fragment
// Rollback function
fn rollback_migration(version: ustring): Result<(), RollbackError>
    put "Rolling back to version " + version

    // Restore from backup
    try restore_backup(version)

    // Verify restoration
    try verify_restoration()

    put "Rollback complete"
    return Ok(())
end fn

// Backup before migration
fn backup_data(): Result<(), BackupError>
    var timestamp := get_timestamp()
    var backup_name := unicode "backup_" + timestamp.to_string()

    put "Creating backup: " + backup_name
    try create_backup(backup_name)

    return Ok(())
end fn
~~~

---

## 7. Testing Version Compatibility

### Version Tests

~~~poly fragment
// Test version compatibility
fn test_version_compatibility()
    // Test V1 compatibility
    var v1_request := create_request(unicode "1.0")
    var v1_response := handle_request(v1_request)
    assert(v1_response.status == 200)

    // Test V2 compatibility
    var v2_request := create_request(unicode "2.0")
    var v2_response := handle_request(v2_request)
    assert(v2_response.status == 200)

    // Test backward compatibility
    var v1_1_request := create_request(unicode "1.1")
    var v1_1_response := handle_request(v1_1_request)
    assert(v1_1_response.status == 200)
end fn

// Test deprecation warnings
fn test_deprecation_warnings()
    var output := capture warn_deprecated(unicode "old_function()", unicode "1.5", unicode "new_function()")
    assert(output.contains(unicode "DEPRECATED"))
    assert(output.contains(unicode "old_function()"))
    assert(output.contains(unicode "new_function()"))
end fn
~~~

---

## Summary

1. **Version Numbering**: Use semantic versioning (MAJOR.MINOR.PATCH)
2. **API Version Management**: Use headers or URL versioning
3. **Backward Compatibility**: Provide deprecation warnings and compatibility layers
4. **Version Negotiation**: Use content negotiation or URL versioning
5. **API Documentation**: Document version-specific endpoints
6. **Migration Strategies**: Provide automated migration and rollback
7. **Testing**: Test version compatibility and deprecation warnings
