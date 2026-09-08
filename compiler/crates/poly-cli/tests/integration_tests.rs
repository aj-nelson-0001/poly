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
fn test_loop_ranges_include_all_endpoints_at_runtime() {
    let source =
        "fn main()\n    loop value 1..3, 7, 19..20\n        put value\n    end loop\nend fn";
    let rust_code = Transpiler::new().transpile(source).unwrap();
    let unique = format!(
        "poly_loop_runtime_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let directory = std::env::temp_dir().join(unique);
    std::fs::create_dir_all(&directory).unwrap();
    let source_path = directory.join("main.rs");
    let binary_path = directory.join("loop_program");
    std::fs::write(&source_path, rust_code).unwrap();

    let compile = std::process::Command::new("rustc")
        .arg("--edition")
        .arg("2021")
        .arg(&source_path)
        .arg("-o")
        .arg(&binary_path)
        .output()
        .unwrap();
    assert!(
        compile.status.success(),
        "generated loop program failed to compile: {}",
        String::from_utf8_lossy(&compile.stderr)
    );

    let run = std::process::Command::new(&binary_path).output().unwrap();
    assert!(
        run.status.success(),
        "generated loop program failed to run: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "1\n2\n3\n7\n19\n20\n");
    std::fs::remove_dir_all(directory).unwrap();
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
    "hello", put "matched"
    _, put "other"
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
fn test_returned_closure_binding_compiles() {
    let source =
        "fn make_adder(n: i32): |x: i32| i32\n    var f := |x| x + n\n    return f\nend fn";
    let rust_code = Transpiler::new().transpile(source).unwrap();
    verify_rust_compiles(&rust_code)
        .unwrap_or_else(|error| panic!("returned closure binding generated invalid Rust: {error}"));
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
                        // Async programs using #[tokio::main] (or referencing
                        // tokio directly, e.g. `delay` lowering to
                        // `tokio::time::sleep`) need Cargo to provide the tokio
                        // dependency; the release example sweep performs that
                        // dependency-aware build. Keep the direct rustc check
                        // for dependency-free output here.
                        if rust_code.contains("#[tokio::main]") || rust_code.contains("tokio::") {
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

/// Normalize platform text-mode line endings so behavioral assertions are
/// identical on Unix and Windows.
fn normalize_newlines(text: &str) -> String {
    text.replace("\r\n", "\n")
}

/// Compile a Poly program to Rust, build it with rustc, run it (optionally
/// feeding `stdin`), and return its stdout. Panics on any failure so each
/// test's assertion stays focused on behavior.
fn compile_and_run(source: &str, stdin: Option<&str>) -> String {
    let rust_code = Transpiler::new().transpile(source).unwrap();
    let unique = format!(
        "poly_runtime_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let directory = std::env::temp_dir().join(unique);
    std::fs::create_dir_all(&directory).unwrap();
    let source_path = directory.join("main.rs");
    let binary_path = directory.join("program");
    std::fs::write(&source_path, rust_code).unwrap();

    let compile = std::process::Command::new("rustc")
        .arg("--edition")
        .arg("2021")
        .arg(&source_path)
        .arg("-o")
        .arg(&binary_path)
        .output()
        .unwrap();
    assert!(
        compile.status.success(),
        "generated program failed to compile: {}",
        String::from_utf8_lossy(&compile.stderr)
    );

    let mut command = std::process::Command::new(&binary_path);
    command.current_dir(&directory);
    if let Some(input) = stdin {
        use std::io::Write;
        use std::process::Stdio;
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "generated program failed to run: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = normalize_newlines(&String::from_utf8_lossy(&output.stdout));
        std::fs::remove_dir_all(directory).unwrap();
        stdout
    } else {
        let run = command.output().unwrap();
        assert!(
            run.status.success(),
            "generated program failed to run: {}",
            String::from_utf8_lossy(&run.stderr)
        );
        let stdout = normalize_newlines(&String::from_utf8_lossy(&run.stdout));
        std::fs::remove_dir_all(directory).unwrap();
        stdout
    }
}

#[test]
fn test_runtime_file_reading_line_by_line() {
    let source = r#"
fn main()
    var file := open("data.txt")
    while not file.eof()
        put file.get_line()
    end while
end fn
"#;
    // `open` resolves relative to the process working directory, so create the
    // fixture file inside the temp directory before compiling and running.
    let unique = format!(
        "poly_file_fixture_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let directory = std::env::temp_dir().join(unique);
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("data.txt"), "hello\nworld\n").unwrap();

    let rust_code = Transpiler::new().transpile(source).unwrap();
    let source_path = directory.join("main.rs");
    let binary_path = directory.join("program");
    std::fs::write(&source_path, rust_code).unwrap();
    let compile = std::process::Command::new("rustc")
        .arg("--edition")
        .arg("2021")
        .arg(&source_path)
        .arg("-o")
        .arg(&binary_path)
        .output()
        .unwrap();
    assert!(
        compile.status.success(),
        "generated file program failed to compile: {}",
        String::from_utf8_lossy(&compile.stderr)
    );
    let run = std::process::Command::new(&binary_path)
        .current_dir(&directory)
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "generated file program failed to run: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "hello\nworld\n");
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn test_runtime_negative_loop_step_magnitude() {
    // `10..1 step -2` must visit 10, 8, 6, 4, 2 — the magnitude of the step
    // drives `step_by`, not its sign (which selects `.rev()`).
    let source = r#"
fn main()
    loop i 10..1 step -2
        put i
    end loop
end fn
"#;
    assert_eq!(compile_and_run(source, None), "10\n8\n6\n4\n2\n");
}

#[test]
fn test_runtime_get_trims_input() {
    // `read_line` keeps the trailing newline; plain `get` must strip it so
    // concatenation and comparisons see exactly what the user typed.
    let source = r#"
fn main()
    var name ustring := get
    put name + unicode "!"
end fn
"#;
    assert_eq!(compile_and_run(source, Some("Alice\n")), "Alice!\n");
}

#[test]
fn test_runtime_get_as_parses_typed_input() {
    // `--as i32` must parse the trimmed input; an untrimmed "21\n" fails
    // `str::parse` and would silently become 0.
    let source = r#"
fn main()
    var n i32 := get --as i32
    put (n * 2).to_string()
end fn
"#;
    assert_eq!(compile_and_run(source, Some("21\n")), "42\n");
}

#[test]
fn test_runtime_map_roundtrip() {
    let source = r#"
fn main()
    var m Map<ustring, i32> := []
    m.insert(unicode "a", 1)
    m.insert(unicode "b", 2)
    put m.get(unicode "a").to_string()
    put m.get(unicode "b").to_string()
    put m.contains_key(unicode "c").to_string()
    put m.len().to_string()
end fn
"#;
    assert_eq!(
        compile_and_run(source, None),
        "Some(1)\nSome(2)\nfalse\n2\n"
    );
}

/// Compile and run an async Poly program through a temporary Cargo project
/// (async output needs the tokio dependency that a bare `rustc` lacks).
/// Returns the program's stdout.
fn compile_and_run_async(source: &str) -> String {
    let rust_code = Transpiler::new().transpile(source).unwrap();
    let unique = format!(
        "poly_async_runtime_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let directory = std::env::temp_dir().join(unique);
    let src_dir = directory.join("src");
    std::fs::create_dir_all(&src_dir).unwrap();
    std::fs::write(src_dir.join("main.rs"), &rust_code).unwrap();
    let rusqlite = if rust_code.contains("rusqlite::") {
        "rusqlite = { version = \"0.31\", features = [\"bundled\"] }\n"
    } else {
        ""
    };
    let manifest = format!(
        "[package]\nname = \"poly_async_runtime\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n[dependencies]\ntokio = {{ version = \"1\", features = [\"macros\", \"rt-multi-thread\", \"time\", \"net\", \"io-util\"] }}\n{rusqlite}[workspace]\n"
    );
    std::fs::write(directory.join("Cargo.toml"), manifest).unwrap();

    let build = std::process::Command::new("cargo")
        .arg("build")
        .arg("--quiet")
        .current_dir(&directory)
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "generated async program failed to build: {}",
        String::from_utf8_lossy(&build.stderr)
    );

    let run = std::process::Command::new(directory.join("target/debug/poly_async_runtime"))
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "generated async program failed to run: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    let stdout = String::from_utf8_lossy(&run.stdout).to_string();
    std::fs::remove_dir_all(directory).unwrap();
    stdout
}

#[test]
fn test_runtime_spawn_runs_background_task() {
    // `spawn background()` lowers to `tokio::spawn(...)`: the spawned task
    // must actually run (interleaved with main's own awaits), not silently
    // never execute as the old stub did.
    let source = r#"
async fn background()
    delay(100).await
    put unicode "background done"
end fn

async fn main()
    spawn background()
    put unicode "main continues"
    delay(300).await
    put unicode "main done"
end fn
"#;
    let output = compile_and_run_async(source);
    assert!(output.contains("main continues"));
    assert!(output.contains("background done"));
    assert!(output.contains("main done"));
    // The spawned task runs concurrently: background completes before main's
    // longer delay finishes, so it appears between the two main lines.
    let main_continues = output.find("main continues").unwrap();
    let background_done = output.find("background done").unwrap();
    let main_done = output.find("main done").unwrap();
    assert!(
        main_continues < background_done && background_done < main_done,
        "spawned task did not run concurrently: {output:?}"
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

#[test]
fn test_runtime_get_until_reads_up_to_delimiter() {
    let source = r#"
fn main()
    var field ustring := get --until unicode ","
    put field
end fn
"#;
    assert_eq!(compile_and_run(source, Some("apple,banana\n")), "apple\n");
}

#[test]
fn test_runtime_get_mask_falls_back_without_terminal() {
    // With piped stdin there is no terminal, so `--mask` must behave like a
    // plain read (the `stty` subprocess fails silently and the input still
    // arrives).
    let source = r#"
fn main()
    var pw ustring := get --mask unicode "*"
    put "got: " + pw
end fn
"#;
    assert_eq!(compile_and_run(source, Some("secret\n")), "got: secret\n");
}

#[test]
fn test_runtime_string_interpolation() {
    let source = r#"
fn main()
    var name ustring := "Alice"
    var age i32 := 30
    put "Name: {name}, Age: {age}"
end fn
"#;
    assert_eq!(compile_and_run(source, None), "Name: Alice, Age: 30\n");
}

#[test]
fn test_runtime_string_repeat_and_vec_join() {
    let source = r##"
fn main()
    put "#".repeat(5)
    var parts Vec<ustring> := [unicode "a", unicode "b", unicode "c"]
    put parts.join(", ")
    put "-".repeat(3)
end fn
"##;
    assert_eq!(compile_and_run(source, None), "#####\na, b, c\n---\n");
}

#[test]
fn test_runtime_string_append_mutation() {
    // `add`/`+=` on a String appends (lowered to `String += &str`).
    let source = r#"
fn main()
    var output ustring := ""
    output := output + "abc"
    output := output + "def"
    output := output + (1 + 2).to_string()
    put output
    var n i32 := 0
    n := n + 5
    n := n + 1
    put n
end fn
"#;
    assert_eq!(compile_and_run(source, None), "abcdef3\n6\n");
}

#[test]
fn test_runtime_checked_index_returns_option() {
    let source = r#"
fn main()
    var xs Vec<i32> := [1, 2, 3]
    put xs.get(0)
    put xs.get(9)
    put "abc".get(1)
end fn
"#;
    assert_eq!(compile_and_run(source, None), "Some(1)\nNone\nSome('b')\n");
}

#[test]
fn test_runtime_assert_and_pass_helpers() {
    let source = r#"
fn main()
    var age i32 := 30
    assert(age > 20)
    pass("all good")
end fn
"#;
    assert_eq!(compile_and_run(source, None), "[PASS] all good\n");
}

#[test]
fn test_runtime_fail_exits_nonzero() {
    let source = r#"
fn main()
    fail("boom")
end fn
"#;
    let rust_code = Transpiler::new().transpile(source).unwrap();
    let unique = format!(
        "poly_runtime_fail_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let directory = std::env::temp_dir().join(unique);
    std::fs::create_dir_all(&directory).unwrap();
    let source_path = directory.join("main.rs");
    let binary_path = directory.join("program");
    std::fs::write(&source_path, rust_code).unwrap();
    let compile = std::process::Command::new("rustc")
        .arg("--edition")
        .arg("2021")
        .arg(&source_path)
        .arg("-o")
        .arg(&binary_path)
        .output()
        .unwrap();
    assert!(compile.status.success());
    let output = std::process::Command::new(&binary_path).output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stdout).contains("[FAIL] boom"));
}

#[test]
fn test_runtime_db_execute_runs_sqlite() {
    // `db_execute` runs SQL against a shared in-memory SQLite database:
    // CREATE TABLE, INSERT, then SELECT must all see the same connection.
    let source = r#"
async fn main()
    db_execute(unicode "CREATE TABLE users (id INTEGER, name TEXT)").await
    db_execute(unicode "INSERT INTO users VALUES (1, 'alice')").await
    db_execute(unicode "INSERT INTO users VALUES (2, 'bob')").await
    var rows := db_execute(unicode "SELECT id, name FROM users ORDER BY id").await
    put rows
end fn
"#;
    let output = compile_and_run_async(source);
    assert_eq!(
        output,
        "[\"Integer(1) | Text(\\\"alice\\\")\", \"Integer(2) | Text(\\\"bob\\\")\"]\n"
    );
}

#[test]
fn test_runtime_tuple_index_access() {
    // Tuple index access `t.0` and chained `t.1.0` must compile and run,
    // yielding the element at that position.
    let source = r#"
fn main()
    var pair := (10, true)
    put pair.0
    put pair.1
    var nested := (1, (2, 3))
    put nested.1.0
    put nested.1.1
    put ("a", 42, 3.5).1
end fn
"#;
    let output = compile_and_run(source, None);
    assert_eq!(output, "10\ntrue\n2\n3\n42\n");
}

#[test]
fn test_runtime_for_loop_tuple_destructuring() {
    // `for (idx, val) in items.enumerate()` must compile, borrow the
    // collection (usable afterwards), and bind the destructured names.
    let source = r#"
fn main()
    var items := [5, 6]
    for (idx, val) in items.enumerate()
        put idx
        put val
    end for
    put items.len()
end fn
"#;
    let output = compile_and_run(source, None);
    assert_eq!(output, "0\n5\n1\n6\n2\n");
}

#[test]
fn test_runtime_vec_clear_reserve_and_iter_chains() {
    // `.clear()`/`.reserve(n)` mutate in place; `.iter()` chains map, filter,
    // sum, collect, and loops all bind owned elements while the collection
    // stays usable.
    let source = r#"
fn main()
    var list Vec<i32> := [1, 2, 3]
    list.reserve(100)
    list.push(4)
    var total := list.iter().sum()
    put total
    var doubled := list.iter().map(|x| x * 2).collect()
    put doubled
    var evens := list.iter().filter(|x| x mod 2 = 0).collect()
    put evens
    var weighted := list.iter().filter(|x| x > 1).map(|x| x * 10).sum()
    put weighted
    var s := 0
    loop x in list.iter()
        s := s + x
    end loop
    put s
    list.clear()
    put list.len()
end fn
"#;
    let output = compile_and_run(source, None);
    assert_eq!(output, "10\n[2, 4, 6, 8]\n[2, 4]\n90\n10\n0\n");
}

#[test]
fn test_runtime_string_append_in_loop_and_vec_put_in_loop() {
    // Variables declared inside loop bodies must register their string/vec
    // kinds: `add row, ...` borrows the amount and `put buf` uses {:?}.
    let source = r#"
fn main()
    var rows ustring := ""
    loop i 1..2
        var row ustring := ""
        row := row + i.to_string()
        rows := rows + row
        var buf Vec<i32> := []
        buf.push(i)
        put buf
    end loop
    put rows
end fn
"#;
    let output = compile_and_run(source, None);
    assert_eq!(output, "[1]\n[2]\n12\n");
}

fn compile_and_run_c(source: &str) -> String {
    compile_and_run_c_capture(source).0
}

fn compile_and_run_c_capture(source: &str) -> (String, String) {
    let c_code = Transpiler::new().transpile_c_checked(source).unwrap();
    let unique = format!(
        "poly_c_runtime_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let directory = std::env::temp_dir().join(unique);
    std::fs::create_dir_all(&directory).unwrap();
    let source_path = directory.join("main.c");
    let binary_path = directory.join(format!("program{}", std::env::consts::EXE_SUFFIX));
    std::fs::write(&source_path, c_code).unwrap();

    let compiler = std::env::var("POLY_CC").unwrap_or_else(|_| {
        if cfg!(windows) {
            "gcc".to_string()
        } else {
            "cc".to_string()
        }
    });
    let compile = std::process::Command::new(compiler)
        .arg("-std=c11")
        .arg("-Wall")
        .arg("-Werror")
        .arg(&source_path)
        .arg("-o")
        .arg(&binary_path)
        .output()
        .unwrap();
    assert!(
        compile.status.success(),
        "generated C failed to compile: {}",
        String::from_utf8_lossy(&compile.stderr)
    );
    let run = std::process::Command::new(&binary_path).output().unwrap();
    assert!(
        run.status.success(),
        "generated C failed to run: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    let stdout = normalize_newlines(&String::from_utf8_lossy(&run.stdout));
    let stderr = normalize_newlines(&String::from_utf8_lossy(&run.stderr));
    std::fs::remove_dir_all(directory).unwrap();
    (stdout, stderr)
}

#[test]
fn test_c_backend_runtime_control_flow_and_foreign_calls() {
    let source = r#"
#c
int double_value(int x) { return x * 2; }
#endc

var total i32 := 0
loop i 1..5
    total := total + double_value(i)
end loop    if total = 30
        put "total=30"
    else
        put "bad"
    end if
    if total > 0
        put "positive"
    end if

    var n i32 := 5
while n > 1
    n := n - 2
end while
put n
loop i 5..1 step -2
    put i
end loop
put "100% complete"
"#;
    assert_eq!(
        compile_and_run_c(source),
        "total=30\npositive\n1\n5\n3\n1\n100% complete\n"
    );
}

#[test]
fn test_c_backend_diagnostic_streams() {
    let source = r#"
error "failure"
warn "warning"
info "details"
put "done"
"#;
    let (stdout, stderr) = compile_and_run_c_capture(source);
    assert_eq!(stdout, "done\n");
    assert_eq!(stderr, "[ERROR] failure\n[WARN] warning\n[INFO] details\n");
}

#[test]
fn test_c_backend_explicit_extern_signature_executes() {
    let source = r#"
extern c fn double_value(value: i32): i32

#c
int double_value(int value) {
    return value * 2;
}
#endc

var result i32 := double_value(21)
put result
"#;
    assert_eq!(compile_and_run_c(source), "42\n");
}

#[test]
fn test_c_backend_reports_unsupported_features() {
    let redirect_error = Transpiler::new()
        .transpile_c_checked("put 1 to \"output.txt\"")
        .unwrap_err();
    assert!(redirect_error.contains("file redirects are not implemented"));

    // Tuples used to be rejected outright; they now compile to anonymous
    // structs with `_N` fields (see test_c_backend_tuples_and_match).
    // Non-capturing closures likewise now lower to a static function plus a
    // function pointer. What must still be rejected is environment capture,
    // which has no C representation in this subset.
    let closure_error = Transpiler::new()
        .transpile_c_checked("var n i32 := 10\nvar f := |x: i32| x + n")
        .unwrap_err();
    assert!(closure_error.contains("non-capturing closures only"));
}

#[test]
fn test_runtime_tuple_element_assignment() {
    // `set pair.0 to value` and `pair.1 = value` must mutate the element in
    // place, and `add nums.1` must apply the mutation to an element.
    let source = r#"
fn main()
    var pair := (10, true)
    pair.0 := 99
    put pair.0
    pair.1 := false
    put pair.1
    var nums := (1, 2, 3)
    nums.1 := nums.1 + 1
    put nums.1
end fn
"#;
    let output = compile_and_run(source, None);
    assert_eq!(output, "99\nfalse\n3\n");
}
