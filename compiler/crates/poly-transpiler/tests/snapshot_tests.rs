//! Snapshot tests for the rust, c, and asm codegen targets.
//!
//! Each test transpiles a fixed Poly source file and compares the generated
//! output byte-for-byte against a snapshot in `snapshots/`. To regenerate all
//! snapshots after an intentional codegen change, run:
//!
//! ```bash
//! POLY_UPDATE_SNAPSHOTS=1 cargo test -p poly-transpiler --test snapshot_tests
//! ```
//!
//! Snapshot files live in `compiler/crates/poly-transpiler/tests/snapshots/`
//! and are committed, so any unintended codegen change fails CI immediately.

use std::path::{Path, PathBuf};

use poly_transpiler::Transpiler;

fn update_snapshots_requested() -> bool {
    std::env::var("POLY_UPDATE_SNAPSHOTS").is_ok()
}

fn snapshot_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots")
}

fn snapshot_path(name: &str) -> PathBuf {
    snapshot_dir().join(format!("{name}.snap"))
}

/// Shared sample programs covering the core language surface that all three
/// targets must support: variables, arithmetic, strings, conditionals, loops,
/// functions, and foreign blocks for the relevant target.
const SAMPLES: &[(&str, &str)] = &[
    (
        "hello",
        "put \"hello world\"\nput 42\n",
    ),
    (
        "vars_arith",
        "var x i32 := 10\nvar y i32 := x * 4 + 2\nput y\nput x - 1\n",
    ),
    (
        "strings",
        "var name string := \"Poly\"\nput name\nput \"hello \" + name\n",
    ),
    (
        "control_flow",
        "var x i32 := 10\nif x > 5,\n    put 1\nelse\n    put 0\nend if\nvar i i32 := 0\nwhile i < 3\n    i := i + 1\nend while\nloop j 0..3\n    put j\nend loop\n",
    ),
    (
        "functions",
        "fn double(v: i32): i32\n    return v * 2\nend fn\nput double(21)\n",
    ),
    (
        "bools",
        "var a bool := true\nvar b bool := false\nvar c bool := a != b\nput c\n",
    ),
];

/// Fixture files from the repo that exercise foreign blocks per target.
const TARGET_FIXTURES: &[(&str, &str, &str)] = &[
    ("asm_target_tests", "asm", "../../../tests/asm_target_tests.poly"),
    ("c_target_tests", "c", "../../../tests/c_target_tests.poly"),
];

fn transpile_source(source: &str, target: &str) -> Result<String, String> {
    Transpiler::new().transpile_target(source, target)
}

fn check_snapshot(name: &str, output: &str) {
    let path = snapshot_path(name);
    if update_snapshots_requested() {
        std::fs::create_dir_all(snapshot_dir()).expect("create snapshot dir");
        std::fs::write(&path, output).expect("write snapshot");
        return;
    }
    let expected = std::fs::read_to_string(&path).unwrap_or_else(|_| {
        panic!(
            "snapshot {} missing — run with POLY_UPDATE_SNAPSHOTS=1 to create it",
            path.display()
        )
    });
    assert_eq!(
        output,
        expected,
        "codegen output for '{name}' diverged from the committed snapshot.\n\
         If this change is intentional, regenerate with:\n  \
         POLY_UPDATE_SNAPSHOTS=1 cargo test -p poly-transpiler --test snapshot_tests"
    );
}

#[test]
fn snapshots_all_targets() {
    for (name, source) in SAMPLES {
        for target in ["rust", "c", "asm"] {
            let snapshot_name = format!("{name}_{target}");
            match transpile_source(source, target) {
                Ok(output) => check_snapshot(&snapshot_name, &output),
                Err(err) => {
                    // Some samples intentionally use features not supported by
                    // a given backend; record the error as the snapshot.
                    check_snapshot(&snapshot_name, &format!("ERROR:\n{err}"));
                }
            }
        }
    }
}

#[test]
fn snapshots_target_fixtures() {
    for (name, target, fixture) in TARGET_FIXTURES {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(fixture);
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|_| panic!("fixture missing: {}", path.display()));
        match transpile_source(&source, target) {
            Ok(output) => check_snapshot(&format!("fixture_{name}"), &output),
            Err(err) => check_snapshot(
                &format!("fixture_{name}"),
                &format!("ERROR:\n{err}"),
            ),
        }
    }
}

#[test]
fn snapshots_rust_pipeline() {
    // The rust target goes through the intermediate representation pipeline;
    // snapshot it separately since its output shape differs from c/asm.
    let transpiler = Transpiler::new();
    for (name, source) in SAMPLES {
        let output = transpiler
            .transpile_with_intermediate_representation(source)
            .unwrap_or_else(|err| format!("ERROR:\n{err}"));
        check_snapshot(&format!("ir_{name}"), &output);
    }
}
