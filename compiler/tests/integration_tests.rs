//! Integration tests for the Poly transpiler.
//!
//! These tests verify that Poly source files can be transpiled to valid Rust code.

use poly_transpiler::Transpiler;

#[test]
fn test_transpile_prime_numbers() {
    let source = include_str!("../../examples/prime_numbers.poly");
    let t = Transpiler::new();
    let result = t.transpile(source);
    assert!(result.is_ok(), "Failed to transpile prime_numbers.poly: {:?}", result.err());
    
    let rust_code = result.unwrap();
    // Verify the Rust code contains expected constructs
    assert!(rust_code.contains("fn is_prime"));
    assert!(rust_code.contains("while"));
    assert!(rust_code.contains("return"));
}

#[test]
fn test_transpile_file_processing() {
    let source = include_str!("../../examples/file_processing.poly");
    let t = Transpiler::new();
    let result = t.transpile(source);
    assert!(result.is_ok(), "Failed to transpile file_processing.poly: {:?}", result.err());
}

#[test]
fn test_transpile_error_handling() {
    let source = include_str!("../../examples/error_handling.poly");
    let t = Transpiler::new();
    let result = t.transpile(source);
    assert!(result.is_ok(), "Failed to transpile error_handling.poly: {:?}", result.err());
    
    let rust_code = result.unwrap();
    // Verify enum definitions are generated
    assert!(rust_code.contains("enum FileError"));
    assert!(rust_code.contains("enum ValidationError"));
}

#[test]
fn test_transpile_interactive_menu() {
    let source = include_str!("../../examples/interactive_menu.poly");
    let t = Transpiler::new();
    let result = t.transpile(source);
    assert!(result.is_ok(), "Failed to transpile interactive_menu.poly: {:?}", result.err());
    
    let rust_code = result.unwrap();
    // Verify function definitions
    assert!(rust_code.contains("fn main"));
    assert!(rust_code.contains("fn greet_user"));
    assert!(rust_code.contains("fn calculator"));
}

#[test]
fn test_transpile_loop_ranges() {
    let source = include_str!("../../examples/loop_ranges.poly");
    let t = Transpiler::new();
    let result = t.transpile(source);
    assert!(result.is_ok(), "Failed to transpile loop_ranges.poly: {:?}", result.err());
    
    let rust_code = result.unwrap();
    // Verify loop constructs
    assert!(rust_code.contains("fn main"));
    assert!(rust_code.contains("fn is_prime"));
}

#[test]
fn test_all_examples_transpile() {
    let examples_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples");
    let t = Transpiler::new();
    
    let mut transpiled = 0;
    let mut failed = Vec::new();
    
    if let Ok(entries) = std::fs::read_dir(&examples_dir) {
        for entry in entries.flatten() {
            if entry.path().extension().map(|e| e == "poly").unwrap_or(false) {
                let source = std::fs::read_to_string(entry.path()).unwrap();
                match t.transpile(&source) {
                    Ok(_) => transpiled += 1,
                    Err(e) => failed.push((entry.file_name().to_string_lossy().to_string(), e)),
                }
            }
        }
    }
    
    assert_eq!(failed.len(), 0, "Failed examples: {:?}", failed);
    assert!(transpiled >= 5, "Expected at least 5 examples to transpile, got {}", transpiled);
}

#[test]
fn test_transpile_basic_programs() {
    let test_cases = vec![
        ("var x: i32 = 42", "let mut x: i32 = 42"),
        (r#"put "Hello""#, r#"println!("{}", "Hello")"#),
        ("const PI = 3.14", "const PI"),
        ("fn add(a: i32, b: i32): i32\n    return a + b\nend fn", "fn add"),
    ];
    
    let t = Transpiler::new();
    for (input, expected) in test_cases {
        let result = t.transpile(input);
        assert!(result.is_ok(), "Failed to transpile: {}", input);
        let rust_code = result.unwrap();
        assert!(rust_code.contains(expected), "Expected '{}' in output for input '{}'", expected, input);
    }
}
