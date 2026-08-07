# Poly Language API Versioning Guide

## Overview

This guide covers API versioning best practices for Poly applications using the new I/O and error handling syntax.

**Note:** Some functions used in this guide (like `parse_version()`, `compare_versions()`, `transform_v1_to_v2()`) are standard library functions. See the Standard Library section for details.

---

## 1. Version Numbering

### Semantic Versioning

```poly\n// Version structure\nstruct Version\n    major: i32\n    minor: i32\n    patch: i32\nend struct\n\n// Parse version string\nfn parse_version(version_str: ustring): Result<Version, VersionError>\n    var parts: Vec<ustring> = version_str.split(u\".\")\n    if parts.len() != 3 then\n        return Error(VersionError::InvalidFormat)\n    end if\n    \n    var major = try parts[0].parse::<i32>()\n    var minor = try parts[1].parse::<i32>()\n    var patch = try parts[2].parse::<i32>()\n    \n    return Ok(Version { major: major, minor: minor, patch: patch })\nend fn\n\n// Compare versions\nfn compare_versions(v1: Version, v2: Version): i32\n    if v1.major != v2.major then\n        return v1.major - v2.major\n    else if v1.minor != v2.minor then\n        return v1.minor - v2.minor\n    else\n        return v1.patch - v2.patch\n    end if\nend fn\n\n// Check compatibility\nfn is_compatible(current: Version, required: Version): bool\n    // Major version must match\n    if current.major != required.major then\n        return false\n    end if\n    \n    // Minor version must be >= required\n    if current.minor < required.minor then\n        return false\n    end if\n    \n    // If minor versions match, patch must be >= required\n    if current.minor == required.minor && current.patch < required.patch then\n        return false\n    end if\n    \n    return true\nend fn\n```

### Version String Format

```poly\n// Version string format: MAJOR.MINOR.PATCH\nvar version: ustring = u\"1.2.3\"\n\n// Pre-release versions\nvar pre_release: ustring = u\"1.2.3-alpha.1\"\nvar beta: ustring = u\"1.2.3-beta.2\"\nvar rc: ustring = u\"1.2.3-rc.1\"\n\n// Build metadata\nvar build: ustring = u\"1.2.3+build.123\"\n\n// Parse version with pre-release and build\nfn parse_full_version(version_str: ustring): Result<FullVersion, VersionError>\n    // Split on + for build metadata\n    var build_parts = version_str.split(u\"+\")\n    var version_part = build_parts[0]\n    var build_metadata = if build_parts.len() > 1 then build_parts[1] else u\"\" end if\n    \n    // Split on - for pre-release\n    var pre_parts = version_part.split(u\"-\")\n    var core_version = pre_parts[0]\n    var pre_release = if pre_parts.len() > 1 then pre_parts[1] else u\"\" end if\n    \n    var version = try parse_version(core_version)\n    \n    return Ok(FullVersion {\n        version: version,\n        pre_release: pre_release,\n        build_metadata: build_metadata\n    })\nend fn\n```

---

## 2. API Version Management

### Version Header

```poly\n// API version in request header\nfn get_api_version(request: Request): ustring\n    return request.headers.get(u\"X-API-Version\") or u\"1.0\"\nend fn\n\n// API version in response header\nfn set_api_version(response: Response, version: ustring)\n    response.headers.set(u\"X-API-Version\", version)\n    response.headers.set(u\"X-API-Compatible-With\", u\"1.0-2.0\")\nend fn\n```

### Version Routing

```poly\n// Route based on API version\nfn route_request(request: Request): Response\n    var version = get_api_version(request)\n    \n    match version\n        u\"1.0\" => return handle_v1(request)\n        u\"1.1\" => return handle_v1_1(request)\n        u\"2.0\" => return handle_v2(request)\n        _ => \n            var response = Response()\n            response.status = 400\n            response.body = u\"Unsupported API version\"\n            return response\n    end match\nend fn\n\n// Version-specific handlers\nfn handle_v1(request: Request): Response\n    // V1 logic\n    return Response { status: 200, body: u\"V1 response\" }\nend fn\n\nfn handle_v1_1(request: Request): Response\n    // V1.1 logic (backward compatible with V1)\n    return Response { status: 200, body: u\"V1.1 response\" }\nend fn\n\nfn handle_v2(request: Request): Response\n    // V2 logic (may not be backward compatible)\n    return Response { status: 200, body: u\"V2 response\" }\nend fn\n```

---

## 3. Backward Compatibility

### Deprecation Warnings

```poly\n// Deprecation warning function\nfn warn_deprecated(feature: ustring, version: ustring, alternative: ustring)\n    warn \"DEPRECATED: \" + feature + \" is deprecated since version \" + version\n    warn \"Use \" + alternative + \" instead\"\nend fn\n\n// Usage\nfn old_function()\n    warn_deprecated(u\"old_function()\", u\"1.5\", u\"new_function()\")\n    // Old implementation\nend fn\n\nfn new_function()\n    // New implementation\nend fn\n```

### Version Compatibility Layer

```poly\n// Compatibility layer for older versions\nfn compatibility_layer(request: Request, target_version: ustring): Request\n    var current_version = get_api_version(request)\n    \n    if compare_versions(parse_version(current_version), parse_version(target_version)) < 0 then\n        // Transform request to target version format\n        return transform_request(request, current_version, target_version)\n    end if\n    \n    return request\nend fn\n\n// Request transformation\nfn transform_request(request: Request, from_version: ustring, to_version: ustring): Request\n    var transformed = request.clone()\n    \n    // Transform based on version difference\n    if from_version == u\"1.0\" && to_version == u\"2.0\" then\n        // V1 to V2 transformation\n        transformed.body = transform_v1_to_v2(request.body)\n    end if\n    \n    return transformed\nend fn\n```

---

## 4. Version Negotiation

### Content Negotiation

```poly\n// Negotiate API version from Accept header\nfn negotiate_version(request: Request): ustring\n    var accept = request.headers.get(u\"Accept\") or u\"\"\n    \n    // Parse Accept header for version\n    if accept.contains(u\"application/vnd.api.v2+json\") then\n        return u\"2.0\"\n    else if accept.contains(u\"application/vnd.api.v1+json\") then\n        return u\"1.0\"\n    else\n        return u\"1.0\"  // Default version\n    end if\nend fn\n\n// Response with content negotiation\nfn respond_with_version(request: Request, data: ustring): Response\n    var version = negotiate_version(request)\n    var response = Response()\n    \n    match version\n        u\"1.0\" =>\n            response.headers.set(u\"Content-Type\", u\"application/vnd.api.v1+json\")\n            response.body = transform_to_v1(data)\n        u\"2.0\" =>\n            response.headers.set(u\"Content-Type\", u\"application/vnd.api.v2+json\")\n            response.body = transform_to_v2(data)\n    end match\n    \n    set_api_version(response, version)\n    return response\nend fn\n```

### URL Versioning

```poly\n// URL-based versioning\nfn route_by_url(request: Request): Response\n    var path = request.path\n    \n    // Extract version from URL\n    if path.starts_with(u\"/api/v2/\") then\n        return handle_v2(request)\n    else if path.starts_with(u\"/api/v1/\") then\n        return handle_v1(request)\n    else\n        return handle_v1(request)  // Default to V1\n    end if\nend fn\n```

---

## 5. API Documentation

### Version Documentation

```poly\n// API documentation structure\nstruct APIDocumentation\n    version: ustring\n    title: ustring\n    description: ustring\n    endpoints: Vec<Endpoint>\n    deprecated: Vec<DeprecatedEndpoint>\nend struct\n\n// Generate documentation\nfn generate_docs(version: ustring): APIDocumentation\n    var docs = APIDocumentation {\n        version: version,\n        title: u\"Poly API\",\n        description: u\"API for Poly applications\",\n        endpoints: get_endpoints(version),\n        deprecated: get_deprecated_endpoints(version)\n    }\n    \n    return docs\nend fn\n\n// Get endpoints for version\nfn get_endpoints(version: ustring): Vec<Endpoint>\n    var endpoints: Vec<Endpoint> = []\n    \n    // Add version-specific endpoints\n    match version\n        u\"1.0\" =>\n            endpoints.push(Endpoint { path: u\"/users\", method: u\"GET\" })\n            endpoints.push(Endpoint { path: u\"/users\", method: u\"POST\" })\n        u\"2.0\" =>\n            endpoints.push(Endpoint { path: u\"/users\", method: u\"GET\" })\n            endpoints.push(Endpoint { path: u\"/users\", method: u\"POST\" })\n            endpoints.push(Endpoint { path: u\"/users/{id}\", method: u\"PUT\" })\n            endpoints.push(Endpoint { path: u\"/users/{id}\", method: u\"DELETE\" })\n    end match\n    \n    return endpoints\nend fn\n```

---

## 6. Migration Strategies

### Automated Migration

```poly\n// Automated migration script\nfn migrate_api(old_version: ustring, new_version: ustring): Result<(), MigrationError>\n    put \"Migrating from \" + old_version + \" to \" + new_version\n    \n    // Backup current data\n    try backup_data()\n    \n    // Run migration steps\n    match (old_version, new_version)\n        (u\"1.0\", u\"2.0\") => try migrate_v1_to_v2()\n        (u\"1.1\", u\"2.0\") => try migrate_v1_1_to_v2()\n        _ => return Error(MigrationError::UnsupportedMigration)\n    end match\n    \n    // Verify migration\n    try verify_migration()\n    \n    put \"Migration complete\"\n    return Ok(())\nend fn\n\n// Migration steps\nfn migrate_v1_to_v2(): Result<(), MigrationError>\n    put \"Step 1: Migrating user data...\"\n    try migrate_user_data()\n    \n    put \"Step 2: Migrating order data...\"\n    try migrate_order_data()\n    \n    put \"Step 3: Updating indexes...\"\n    try update_indexes()\n    \n    return Ok(())\nend fn\n```

### Rollback Strategy

```poly\n// Rollback function\nfn rollback_migration(version: ustring): Result<(), RollbackError>\n    put \"Rolling back to version \" + version\n    \n    // Restore from backup\n    try restore_backup(version)\n    \n    // Verify restoration\n    try verify_restoration()\n    \n    put \"Rollback complete\"\n    return Ok(())\nend fn\n\n// Backup before migration\nfn backup_data(): Result<(), BackupError>\n    var timestamp = get_timestamp()\n    var backup_name = u\"backup_\" + timestamp.to_string()\n    \n    put \"Creating backup: \" + backup_name\n    try create_backup(backup_name)\n    \n    return Ok(())\nend fn\n```

---

## 7. Testing Version Compatibility

### Version Tests

```poly\n// Test version compatibility\nfn test_version_compatibility()\n    // Test V1 compatibility\n    var v1_request = create_request(u\"1.0\")\n    var v1_response = handle_request(v1_request)\n    assert(v1_response.status == 200)\n    \n    // Test V2 compatibility\n    var v2_request = create_request(u\"2.0\")\n    var v2_response = handle_request(v2_request)\n    assert(v2_response.status == 200)\n    \n    // Test backward compatibility\n    var v1_1_request = create_request(u\"1.1\")\n    var v1_1_response = handle_request(v1_1_request)\n    assert(v1_1_response.status == 200)\nend fn\n\n// Test deprecation warnings\nfn test_deprecation_warnings()\n    var output = capture warn_deprecated(u\"old_function()\", u\"1.5\", u\"new_function()\")\n    assert(output.contains(u\"DEPRECATED\"))\n    assert(output.contains(u\"old_function()\"))\n    assert(output.contains(u\"new_function()\"))\nend fn\n```

---

## Summary

1. **Version Numbering**: Use semantic versioning (MAJOR.MINOR.PATCH)
2. **API Version Management**: Use headers or URL versioning
3. **Backward Compatibility**: Provide deprecation warnings and compatibility layers
4. **Version Negotiation**: Use content negotiation or URL versioning
5. **API Documentation**: Document version-specific endpoints
6. **Migration Strategies**: Provide automated migration and rollback
7. **Testing**: Test version compatibility and deprecation warnings
