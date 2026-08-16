# Poly Language Future Migration Guide

## Overview

This guide covers strategies and best practices for migrating to future Poly syntax changes.

---

## 1. Deprecation Strategy

### Deprecation Timeline

~~~poly
// Deprecation phases
enum DeprecationPhase
    Active      // Feature is fully supported
    Deprecated  // Feature shows warnings, still works
    Removed     // Feature no longer works
end enum

// Deprecation warning function
fn deprecation_warning(feature: ustring, removed_in: ustring, alternative: ustring)
    warn "DEPRECATED: " + feature + " will be removed in version " + removed_in
    warn "Use " + alternative + " instead"
end fn

// Example usage
fn old_syntax()
    deprecation_warning(unicode "old_syntax()", unicode "2.0", unicode "new_syntax()")
    // Old implementation
end fn

fn new_syntax()
    // New implementation
end fn
~~~

### Version Migration Timeline

~~~poly fragment
// Migration timeline
struct MigrationTimeline
    deprecated_in: ustring
    removed_in: ustring
    alternative: ustring
end struct

// Check if feature is deprecated
fn check_deprecation(feature: ustring, current_version: ustring): Option<MigrationTimeline>
    var timelines Map<ustring, MigrationTimeline> := {
        unicode "putn": MigrationTimeline {
            deprecated_in: unicode "1.5",
            removed_in: unicode "2.0",
            alternative: unicode "put -n"
        },
        unicode "pute": MigrationTimeline {
            deprecated_in: unicode "1.5",
            removed_in: unicode "2.0",
            alternative: unicode "error"
        },
        unicode "Err(e)": MigrationTimeline {
            deprecated_in: unicode "1.5",
            removed_in: unicode "2.0",
            alternative: unicode "Error(e)"
        }
    }

    return timelines.get(feature)
end fn
~~~

---

## 2. Automated Migration Tools

### Syntax Transformer

~~~poly fragment
// Transform old syntax to new syntax
fn transform_syntax(code: ustring): ustring
    var transformed := code

    // Transform putn to put -n
    transformed = transformed.replace(unicode "putn ", unicode "put -n ")

    // Transform pute to error/warn/info
    transformed = transformed.replace(unicode "pute ", unicode "error ")

    // Transform Err(e) to Error(e)
    transformed = transformed.replace(unicode "Err(e)", unicode "Error(e)")

    // Transform with timeout to --timeout
    transformed = transformed.replace(unicode "with timeout ", unicode "--timeout ")

    // Transform with default to --default
    transformed = transformed.replace(unicode "with default ", unicode "--default ")

    // Transform with mask to --mask
    transformed = transformed.replace(unicode "with mask ", unicode "--mask ")

    // Transform as Type to --as Type
    transformed = transformed.replace(unicode " as ", unicode " --as ")

    // Transform until to --until
    transformed = transformed.replace(unicode " until ", unicode " --until ")

    return transformed
end fn

// Batch transformation
fn transform_directory(dir: ustring): Result<(), TransformError>
    var files := list_files(dir, unicode "*.poly")

    loop: file in files
        var code := read_file(file)
        var transformed := transform_syntax(code)
        write_file(file, transformed)
        put "Transformed: " + file
    end loop

    return Ok(())
end fn
~~~

### Migration Script

~~~poly fragment
// Migration script
fn migrate_project(version: ustring): Result<(), MigrationError>
    put "Migrating to version " + version

    // Step 1: Transform syntax
    put "Step 1: Transforming syntax..."
    try transform_directory(unicode "src/")

    // Step 2: Update dependencies
    put "Step 2: Updating dependencies..."
    try update_dependencies(version)

    // Step 3: Run tests
    put "Step 3: Running tests..."
    try run_tests()

    // Step 4: Verify migration
    put "Step 4: Verifying migration..."
    try verify_migration()

    put "Migration complete!"
    return Ok(())
end fn

// Update dependencies
fn update_dependencies(version: ustring): Result<(), DependencyError>
    var manifest := read_file(unicode "poly.toml")
    var updated := manifest.replace(unicode "poly-version = \"1.4\"", unicode "poly-version = \"" + version + unicode "\"")
    write_file(unicode "poly.toml", updated)
    return Ok(())
end fn
~~~

---

## 3. Backward Compatibility

### Compatibility Mode

~~~poly
// Compatibility mode flag
var compatibility_mode bool := get_env("POLY_COMPAT") or unicode "false" == unicode "true"

// Compatibility wrapper
fn compat_putn(expression: ustring)
    if compatibility_mode,
        // Old syntax
        putn expression
    else
        // New syntax
        put -n expression
    end if
end fn

fn compat_pute(expression: ustring)
    if compatibility_mode,
        // Old syntax
        pute expression
    else
        // New syntax
        error expression
    end if
end fn
~~~

### Version Detection

~~~poly
// Detect Poly version
fn get_poly_version(): ustring
    // This would be provided by the runtime
    return unicode "1.5"
end fn

// Use version-specific syntax
fn version_specific_code()
    var version := get_poly_version()

    if compare_versions(parse_version(version), parse_version(unicode "2.0")) >= 0,
        // Use new syntax
        put -n "Loading..."
    else
        // Use old syntax
        putn unicode "Loading..."
    end if
end fn
~~~

---

## 4. Testing Migration

### Migration Tests

~~~poly fragment
// Test migration correctness
fn test_migration()
    // Test old syntax still works (if compatibility mode)
    if compatibility_mode,
        test_old_syntax()
    end if

    // Test new syntax works
    test_new_syntax()

    // Test transformation
    test_syntax_transformation()
end fn

// Test old syntax
fn test_old_syntax()
    var output := capture putn unicode "test"
    assert(output == unicode "test")
end fn

// Test new syntax
fn test_new_syntax()
    var output := capture put -n unicode "test"
    assert(output == unicode "test")
end fn

// Test transformation
fn test_syntax_transformation()
    var old_code := unicode "putn \"hello\"\npute \"error\""
    var new_code := transform_syntax(old_code)
    assert(new_code == unicode "put -n \"hello\"\nerror \"error\"")
end fn
~~~

### Performance Testing

~~~poly
// Test performance impact
fn test_performance_impact()
    // Benchmark old syntax
    var start := time_now()
    loop: i 0..10000
        putn unicode "test"
    end loop
    var old_duration := time_now() - start

    // Benchmark new syntax
    start = time_now()
    loop: i 0..10000
        put -n unicode "test"
    end loop
    var new_duration := time_now() - start

    put "Old syntax: " + old_duration.to_string() + "ms"
    put "New syntax: " + new_duration.to_string() + "ms"

    // Performance should be similar
    assert(new_duration < old_duration * 1.1)  // Allow 10% variance
end fn
~~~

---

## 5. Documentation Updates

### Update Documentation

~~~poly
// Update documentation for new syntax
fn update_documentation()
    // Update README
    var readme := read_file(unicode "README.md")
    var updated_readme := transform_syntax(readme)
    write_file(unicode "README.md", updated_readme)

    // Update examples
    var examples := list_files(unicode "examples/", unicode "*.poly")
    loop: example in examples
        var code := read_file(example)
        var transformed := transform_syntax(code)
        write_file(example, transformed)
    end loop

    // Update tests
    var tests := list_files(unicode "tests/", unicode "*.poly")
    loop: test in tests
        var code := read_file(test)
        var transformed := transform_syntax(code)
        write_file(test, transformed)
    end loop

    put "Documentation updated"
end fn
~~~

### Changelog

~~~poly
// Generate changelog
fn generate_changelog(version: ustring): ustring
    var changelog ustring := "# Changelog\n\n## " + version + "\n\n"

    // Add breaking changes
    changelog = changelog + "### Breaking Changes\n\n"
    changelog = changelog + "- `putn` replaced with `put -n`\n"
    changelog = changelog + "- `pute` replaced with `error`/`warn`/`info`\n"
    changelog = changelog + "- `Err(e)` replaced with `Error(e)`\n"
    changelog = changelog + "- Input flags changed from `with` to `--`\n\n"

    // Add new features
    changelog = changelog + "### New Features\n\n"
    changelog = changelog + "- Added `error`, `warn`, `info` commands\n"
    changelog = changelog + "- Added input flags: `--timeout`, `--default`, `--mask`, `--as`, `--until`, `--bytes`\n"
    changelog = changelog + "- Added Unicode string inference\n\n"

    // Add deprecations
    changelog = changelog + "### Deprecations\n\n"
    changelog = changelog + "- `putn` deprecated, use `put -n` instead\n"
    changelog = changelog + "- `pute` deprecated, use `error`/`warn`/`info` instead\n"
    changelog = changelog + "- `Err(e)` deprecated, use `Error(e)` instead\n\n"

    return changelog
end fn
~~~

---

## 6. Rollback Strategy

### Rollback Migration

~~~poly fragment
// Rollback migration
fn rollback_migration(version: ustring): Result<(), RollbackError>
    put "Rolling back to version " + version

    // Step 1: Restore from backup
    put "Step 1: Restoring from backup..."
    try restore_backup(version)

    // Step 2: Revert syntax changes
    put "Step 2: Reverting syntax changes..."
    try revert_syntax_changes()

    // Step 3: Update dependencies
    put "Step 3: Updating dependencies..."
    try update_dependencies(version)

    // Step 4: Verify rollback
    put "Step 4: Verifying rollback..."
    try verify_rollback()

    put "Rollback complete!"
    return Ok(())
end fn

// Revert syntax changes
fn revert_syntax_changes(): Result<(), RevertError>
    // Reverse transformations
    var files := list_files(unicode "src/", unicode "*.poly")

    loop: file in files
        var code := read_file(file)
        var reverted := revert_syntax(code)
        write_file(file, reverted)
    end loop

    return Ok(())
end fn

// Revert syntax
fn revert_syntax(code: ustring): ustring
    var reverted := code

    // Reverse transformations
    reverted = reverted.replace(unicode "put -n ", unicode "putn ")
    reverted = reverted.replace(unicode "error ", unicode "pute ")
    reverted = reverted.replace(unicode "Error(e)", unicode "Err(e)")
    reverted = reverted.replace(unicode "--timeout ", unicode "with timeout ")
    reverted = reverted.replace(unicode "--default ", unicode "with default ")
    reverted = reverted.replace(unicode "--mask ", unicode "with mask ")
    reverted = reverted.replace(unicode "--as ", unicode " as ")
    reverted = reverted.replace(unicode "--until ", unicode " until ")

    return reverted
end fn
~~~

---

## 7. Communication Strategy

### Migration Announcement

~~~poly
// Migration announcement
fn announce_migration(version: ustring)
    put "=== Poly Migration Announcement ==="
    put ""
    put "Version " + version + " introduces syntax changes:"
    put ""
    put "Breaking Changes:"
    put "  - `putn` -> `put -n`"
    put "  - `pute` -> `error`/`warn`/`info`"
    put "  - `Err(e)` -> `Error(e)`"
    put "  - Input flags changed from `with` to `--`"
    put ""
    put "Migration Guide: https://poly-lang.org/migration/" + version
    put ""
    put "Timeline:"
    put "  - Deprecation: Version 1.5"
    put "  - Removal: Version 2.0"
    put ""
    put "Tools Available:"
    put "  - `poly migrate` - Automated migration tool"
    put "  - `poly transform` - Syntax transformer"
    put "  - `poly test-migration` - Migration tester"
end fn
~~~

### Migration Checklist

~~~poly
// Migration checklist
fn migration_checklist(): Vec<ustring>
    var checklist Vec<ustring> := []

    checklist.push(unicode "[ ] Review migration guide")
    checklist.push(unicode "[ ] Run automated migration tool")
    checklist.push(unicode "[ ] Update documentation")
    checklist.push(unicode "[ ] Run tests")
    checklist.push(unicode "[ ] Performance testing")
    checklist.push(unicode "[ ] Update CI/CD pipelines")
    checklist.push(unicode "[ ] Communicate changes to team")
    checklist.push(unicode "[ ] Monitor for issues")

    return checklist
end fn

// Display checklist
fn display_checklist()
    var checklist := migration_checklist()
    put "Migration Checklist:"
    put ""
    loop: item in checklist
        put item
    end loop
end fn
~~~

---

## Summary

1. **Deprecation Strategy**: Use phased deprecation with clear timelines
2. **Automated Tools**: Provide syntax transformers and migration scripts
3. **Backward Compatibility**: Support compatibility mode during transition
4. **Testing**: Test migration correctness and performance
5. **Documentation**: Update all documentation and examples
6. **Rollback**: Provide rollback strategy for failed migrations
7. **Communication**: Announce changes clearly and provide migration guides
