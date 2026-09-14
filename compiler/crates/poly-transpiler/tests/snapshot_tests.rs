//! Snapshot tests for the rust, c, asm, and js codegen targets.
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
        "fn main()\n    put \"hello world\"\n    put 42\nend fn\n",
    ),
    (
        "vars_arith",
        "fn main()\n    var x i32 := 10\n    var y i32 := x * 4 + 2\n    put y\n    put x - 1\nend fn\n",
    ),
    (
        "strings",
        "fn main()\n    var name string := \"Poly\"\n    put name\n    put \"hello \" + name\nend fn\n",
    ),
    (
        "control_flow",
        "fn main()\n    var x i32 := 10\n    if x > 5,\n        put 1\n    else\n        put 0\n    end if\n    var i i32 := 0\n    while i < 3\n        i := i + 1\n    end while\n    loop j 0..3\n        put j\n    end loop\nend fn\n",
    ),
    (
        "functions",
        "fn double(v: i32): i32\n    return v * 2\nend fn\n\nfn main()\n    put double(21)\nend fn\n",
    ),
    (
        "bools",
        "fn main()\n    var a bool := true\n    var b bool := false\n    var c bool := a != b\n    put c\nend fn\n",
    ),
];

/// Fixture files from the repo that exercise foreign blocks per target.
const TARGET_FIXTURES: &[(&str, &str, &str)] = &[
    (
        "asm_target_tests",
        "asm",
        "../../../tests/asm_target_tests.poly",
    ),
    ("c_target_tests", "c", "../../../tests/c_target_tests.poly"),
    (
        "js_target_tests",
        "js",
        "../../../tests/js_target_tests.poly",
    ),
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
    // Compare on normalized line endings: `git checkout` with `core.autocrlf`
    // on Windows converts the committed LF snapshots to CRLF in the working
    // tree, while generated output always uses `\n`. Byte-for-byte comparison
    // would fail on Windows only (CI: Test (windows-latest)).
    let normalize = |text: &str| text.replace("\r\n", "\n");
    assert_eq!(
        normalize(output),
        normalize(&expected),
        "codegen output for '{name}' diverged from the committed snapshot.\n\
         If this change is intentional, regenerate with:\n  \
         POLY_UPDATE_SNAPSHOTS=1 cargo test -p poly-transpiler --test snapshot_tests"
    );
}

#[test]
fn snapshots_all_targets() {
    for (name, source) in SAMPLES {
        for target in ["rust", "c", "asm", "js"] {
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
            Err(err) => check_snapshot(&format!("fixture_{name}"), &format!("ERROR:\n{err}")),
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
