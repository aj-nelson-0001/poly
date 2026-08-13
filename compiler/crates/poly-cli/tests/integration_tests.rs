//! Integration tests for the Poly transpiler.
//!
//! These tests verify that Poly source files can be transpiled to valid Rust code.

use poly_transpiler::Transpiler;
use std::sync::{Mutex, OnceLock};

fn examples_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../examples")
}

#[test]
fn test_transpile_prime_numbers() {
    let source = std::fs::read_to_string(examples_dir().join("prime_numbers.poly")).unwrap();
    let t = Transpiler::new();
    let result = t.transpile(&source);
    assert!(
        result.is_ok(),
        "Failed to transpile prime_numbers.poly: {:?}",
        result.err()
    );

    let rust_code = result.unwrap();
    assert!(rust_code.contains("fn is_prime"));
    assert!(rust_code.contains("while"));
    assert!(rust_code.contains("return"));
}

#[test]
fn test_transpile_file_processing() {
    let source = std::fs::read_to_string(examples_dir().join("file_processing.poly")).unwrap();
    let t = Transpiler::new();
    let result = t.transpile(&source);
    assert!(
        result.is_ok(),
        "Failed to transpile file_processing.poly: {:?}",
        result.err()
    );
}

#[test]
fn test_transpile_error_handling() {
    let source = std::fs::read_to_string(examples_dir().join("error_handling.poly")).unwrap();
    let t = Transpiler::new();
    let result = t.transpile(&source);
    assert!(
        result.is_ok(),
        "Failed to transpile error_handling.poly: {:?}",
        result.err()
    );

    let rust_code = result.unwrap();
    assert!(rust_code.contains("enum FileError"));
    assert!(rust_code.contains("enum ValidationError"));
}

#[test]
fn test_transpile_interactive_menu() {
    let source = std::fs::read_to_string(examples_dir().join("interactive_menu.poly")).unwrap();
    let t = Transpiler::new();
    let result = t.transpile(&source);
    assert!(
        result.is_ok(),
        "Failed to transpile interactive_menu.poly: {:?}",
        result.err()
    );

    let rust_code = result.unwrap();
    assert!(rust_code.contains("fn main"));
    assert!(rust_code.contains("fn greet_user"));
    assert!(rust_code.contains("fn calculator"));
}

#[test]
fn test_transpile_loop_ranges() {
    let source = std::fs::read_to_string(examples_dir().join("loop_ranges.poly")).unwrap();
    let t = Transpiler::new();
    let result = t.transpile(&source);
    assert!(
        result.is_ok(),
        "Failed to transpile loop_ranges.poly: {:?}",
        result.err()
    );

    let rust_code = result.unwrap();
    assert!(rust_code.contains("fn main"));
    assert!(rust_code.contains("fn is_prime"));
}

#[test]
fn test_all_examples_transpile() {
    let t = Transpiler::new();

    let mut transpiled = 0;
    let mut failed = Vec::new();

    if let Ok(entries) = std::fs::read_dir(examples_dir()) {
        for entry in entries.flatten() {
            if entry
                .path()
                .extension()
                .map(|e| e == "poly")
                .unwrap_or(false)
            {
                let name = entry.file_name().to_string_lossy().to_string();
                let source = std::fs::read_to_string(entry.path()).unwrap();
                match t.transpile(&source) {
                    Ok(_) => transpiled += 1,
                    Err(e) => failed.push((name, e)),
                }
            }
        }
    }

    assert!(failed.is_empty(), "Unexpected failures: {:?}", failed);
    assert!(
        transpiled >= 5,
        "Expected at least 5 examples to transpile, got {}",
        transpiled
    );
}

#[test]
fn test_transpile_basic_programs() {
    let test_cases = vec![
        ("var x i32 := 42", "let mut x: i32 = 42"),
        (r#"put "Hello""#, r#"println!("{}", String::from("Hello"))"#),
        ("const PI := 3.14", "const PI"),
        (
            "fn sum(a: i32, b: i32): i32\n    return a + b\nend fn",
            "fn sum",
        ),
    ];

    let t = Transpiler::new();
    for (input, expected) in test_cases {
        let result = t.transpile(input);
        assert!(result.is_ok(), "Failed to transpile: {}", input);
        let rust_code = result.unwrap();
        assert!(
            rust_code.contains(expected),
            "Expected '{}' in output for input '{}'",
            expected,
            input
        );
    }
}

#[test]
fn test_parse_and_transpile_roundtrip() {
    let source = r#"
fn greet(name: ustring): ustring
    return "Hello, " + name
end fn

var greeting := greet("World")
put greeting
"#;
    let t = Transpiler::new();
    let result = t.transpile(source);
    assert!(
        result.is_ok(),
        "Failed to transpile roundtrip program: {:?}",
        result.err()
    );

    let rust_code = result.unwrap();
    assert!(rust_code.contains("fn greet"));
    assert!(rust_code.contains("Hello, "));
}

#[test]
fn test_if_else_chains_compiles_to_valid_rust() {
    let source = std::fs::read_to_string(examples_dir().join("if_else_chains.poly")).unwrap();
    let rust_code = Transpiler::new().transpile(&source).unwrap();
    verify_rust_compiles(&rust_code)
        .unwrap_or_else(|error| panic!("if_else_chains.poly generated invalid Rust: {error}"));
}

#[test]
fn test_string_match_compiles_to_valid_rust() {
    let source = r#"
var value := "hello"
match value
    "hello" => put "matched"
    _ => put "other"
end match
"#;
    let rust_code = Transpiler::new().transpile(source).unwrap();
    verify_rust_compiles(&rust_code)
        .unwrap_or_else(|error| panic!("string match generated invalid Rust: {error}"));
}

#[test]
fn test_representative_generated_rust_codegen_compiles() {
    let source = "fn sum(a: i32, b: i32): i32\n    return a + b\nend fn";
    let rust_code = Transpiler::new().transpile(source).unwrap();
    verify_rust_codegen_compiles(&rust_code)
        .unwrap_or_else(|error| panic!("representative generated Rust failed codegen: {error}"));
}

#[test]
fn test_all_examples_compile_to_valid_rust() {
    let t = poly_transpiler::Transpiler::new();
    let examples_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../examples");

    let mut tested = 0;
    let mut failures = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&examples_dir) {
        for entry in entries.flatten() {
            if entry
                .path()
                .extension()
                .map(|e| e == "poly")
                .unwrap_or(false)
            {
                let name = entry.file_name().to_string_lossy().to_string();
                let source = std::fs::read_to_string(entry.path()).unwrap();

                tested += 1;

                match t.transpile(&source) {
                    Ok(rust_code) => {
                        // Async programs using #[tokio::main] need Cargo to
                        // provide the tokio dependency; the release example
                        // sweep performs that dependency-aware build. Keep the
                        // direct rustc check for dependency-free output here.
                        if rust_code.contains("#[tokio::main]") {
                            assert!(
                                rust_code.contains("async fn"),
                                "{} has a tokio entry point without async code",
                                name
                            );
                        } else if let Err(error) = verify_rust_compiles(&rust_code) {
                            failures.push((name, format!("generated Rust failed: {error}")));
                        }
                    }
                    Err(error) => {
                        failures.push((name, format!("transpilation failed: {error}")));
                    }
                }
            }
        }
    }

    assert!(
        tested >= 5,
        "Expected at least 5 examples to test, got {}",
        tested
    );
    assert!(
        failures.is_empty(),
        "{} of {} examples failed validation: {:?}",
        failures.len(),
        tested,
        failures
    );
}

/// Verify generated Rust without running LLVM code generation.
///
/// Metadata emission still performs parsing, name resolution, type checking,
/// borrow checking, and monomorphization checks needed by these tests, but it
/// avoids producing an object file. The process-wide lock prevents several
/// rustc instances from multiplying their thread usage when the test harness
/// runs the individual integration tests concurrently.
fn verify_rust_compiles(code: &str) -> Result<(), String> {
    verify_rust_compiles_with_emit(code, "metadata")
}

/// Exercise the full rustc code-generation path for one representative output.
fn verify_rust_codegen_compiles(code: &str) -> Result<(), String> {
    verify_rust_compiles_with_emit(code, "link")
}

fn verify_rust_compiles_with_emit(code: &str, emit: &str) -> Result<(), String> {
    use std::process::Command;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
    static RUSTC_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    let _rustc_guard = RUSTC_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| "rustc verification lock was poisoned".to_string())?;

    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let temp_dir = std::env::temp_dir();
    let stem = format!("poly_test_compile_{}_{}", std::process::id(), id);
    let temp_file = temp_dir.join(format!("{stem}.rs"));
    let output_extension = if emit == "metadata" { "rmeta" } else { "rlib" };
    let output_file = temp_dir.join(format!("lib{stem}.{output_extension}"));
    std::fs::write(&temp_file, code).map_err(|e| e.to_string())?;

    let output = Command::new("rustc")
        .arg("--edition")
        .arg("2021")
        .arg("--crate-type")
        .arg("lib")
        .arg("--crate-name")
        .arg(&stem)
        .arg("--emit")
        .arg(emit)
        .arg("--out-dir")
        .arg(&temp_dir)
        .arg(&temp_file)
        .output()
        .map_err(|e| format!("Failed to run rustc: {}", e));

    let _ = std::fs::remove_file(&temp_file);
    let _ = std::fs::remove_file(output_file);
    let output = output?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(stderr.to_string())
    }
}
