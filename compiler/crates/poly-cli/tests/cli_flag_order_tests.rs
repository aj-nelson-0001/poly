//! End-to-end CLI tests: flags must work in any position, the JS default
//! build must execute via Node, and retired syntax must produce migration
//! diagnostics. These run the compiled `poly` binary directly.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn poly() -> Command {
    Command::new(env!("CARGO_BIN_EXE_poly"))
}

fn unique_temp_dir(label: &str) -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "poly-cli-e2e-{}-{}-{}",
        label,
        std::process::id(),
        timestamp
    ))
}

const SIMPLE_PROGRAM: &str = "fn main()\n    put \"hello\"\nend fn\n";

#[test]
fn emit_flag_is_honored_when_the_file_comes_first() {
    let dir = unique_temp_dir("emit");
    fs::create_dir_all(&dir).unwrap();
    let source = dir.join("ok.poly");
    fs::write(&source, SIMPLE_PROGRAM).unwrap();

    // Historically this silently fell through to a default Cargo build.
    let output = poly().arg(&source).arg("--emit-rust").output().unwrap();
    assert!(
        output.status.success(),
        "file-first --emit-rust failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("fn main"),
        "expected generated Rust on stdout, got: {stdout}"
    );

    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn check_flag_is_honored_when_the_file_comes_first() {
    let dir = unique_temp_dir("check");
    fs::create_dir_all(&dir).unwrap();
    let source = dir.join("ok.poly");
    fs::write(&source, SIMPLE_PROGRAM).unwrap();

    let output = poly().arg(&source).arg("--check").output().unwrap();
    assert!(
        output.status.success(),
        "file-first --check failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn target_js_default_build_runs_with_node() {
    let dir = unique_temp_dir("jsrun");
    fs::create_dir_all(&dir).unwrap();
    let source = dir.join("ok.poly");
    fs::write(&source, SIMPLE_PROGRAM).unwrap();

    // README documents `poly --target js file.poly` as "Emit JavaScript and
    // run it with Node". The program must execute, not merely be emitted.
    let output = poly()
        .arg("--target")
        .arg("js")
        .arg(&source)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "js default build failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("Generated"),
        "expected an emission notice, got: {stdout}"
    );
    assert!(
        stdout.contains("hello"),
        "expected the program's Node output, got: {stdout}"
    );

    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn unknown_flags_are_rejected_after_the_file() {
    let dir = unique_temp_dir("unknown");
    fs::create_dir_all(&dir).unwrap();
    let source = dir.join("ok.poly");
    fs::write(&source, SIMPLE_PROGRAM).unwrap();

    let output = poly().arg(&source).arg("--frobnicate").output().unwrap();
    assert!(
        !output.status.success(),
        "unknown flag after the file must fail"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Unknown option"),
        "expected an unknown-option error, got: {stderr}"
    );

    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn retired_statements_report_migration_suggestions() {
    let dir = unique_temp_dir("retired");
    fs::create_dir_all(&dir).unwrap();

    for (snippet, expected_hint) in [
        ("set x to 2", "Use `x := y` instead"),
        ("add x, 5", "Use `x := x + n` instead"),
        ("inc x", "Use `x := x + 1` instead"),
        ("dec x", "Use `x := x - 1` instead"),
    ] {
        let source = dir.join("retired.poly");
        fs::write(&source, snippet).unwrap();
        let output = poly().arg("--check").arg(&source).output().unwrap();
        assert!(!output.status.success(), "`{snippet}` must be rejected");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("has been removed"),
            "`{snippet}` missing named diagnostic, got: {stderr}"
        );
        assert!(
            stderr.contains(expected_hint),
            "`{snippet}` missing migration hint, got: {stderr}"
        );
    }

    fs::remove_dir_all(&dir).unwrap();
}
