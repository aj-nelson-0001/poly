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

#[test]
fn function_local_consts_work_across_backends() {
    // Regression: a `const` declared inside a function body used to panic the
    // IR generator ("unreachable: constants are collected first"). It must
    // now compile through every backend and run correctly.
    let dir = unique_temp_dir("fnconst");
    fs::create_dir_all(&dir).unwrap();
    let source = dir.join("fnconst.poly");
    fs::write(
        &source,
        "fn main()\n    const base := 10\n    const scale := 2\n    put base * scale + 5\nend fn\n",
    )
    .unwrap();

    // JS: emitted and executed via Node, so the printed value is checked.
    let output = poly()
        .arg("--target")
        .arg("js")
        .arg(&source)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "js build failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("25"),
        "expected the computed constant value 25 on stdout, got: {stdout}"
    );

    // Rust: generated code must compile, with the const lowered to a local.
    let output = poly().arg(&source).arg("--emit-rust").output().unwrap();
    assert!(
        output.status.success(),
        "rust emit failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let rust = String::from_utf8_lossy(&output.stdout);
    assert!(
        rust.contains("let mut base = 10;"),
        "expected fn-local const lowered to a Rust local, got: {rust}"
    );

    // C: same lowering.
    let output = poly().arg(&source).arg("--emit-c").output().unwrap();
    assert!(
        output.status.success(),
        "c emit failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let c = String::from_utf8_lossy(&output.stdout);
    assert!(
        c.contains("const int32_t base = 10;"),
        "expected fn-local const emitted as a C local, got: {c}"
    );

    // Parser-level check must stay green for all of the above to be reached.
    let output = poly().arg(&source).arg("--check").output().unwrap();
    assert!(
        output.status.success(),
        "--check failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn dep_declarations_drive_project_manifest_and_check() {
    // `dep name = "version"` declares an external crate: --project must emit
    // it into Cargo.toml, and --check must resolve it via a cargo-based
    // verification instead of bare rustc. Pure-Rust crates are used so the
    // test has no system-library requirements (alsa-sys, for example, needs
    // ALSA headers that plain CI runners lack).
    let dir = unique_temp_dir("depdecl");
    fs::create_dir_all(&dir).unwrap();
    let source = dir.join("deps.poly");
    fs::write(
        &source,
        "dep rand = \"0.8\"\ndep libc = \"0.2\"\n\nfn main()\n    put \"ok\"\nend fn\n",
    )
    .unwrap();

    let output = poly().arg(&source).arg("--check").output().unwrap();
    assert!(
        output.status.success(),
        "--check with deps failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let project = dir.join("proj");
    let output = poly()
        .arg("--project")
        .arg(&project)
        .arg(&source)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "--project failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let manifest = fs::read_to_string(project.join("Cargo.toml")).unwrap();
    assert!(
        manifest.contains("rand = \"0.8\""),
        "expected rand dependency in manifest: {manifest}"
    );
    assert!(
        manifest.contains("libc = \"0.2\""),
        "expected libc dependency in manifest: {manifest}"
    );

    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn dep_declarations_warn_on_non_rust_targets() {
    // Only the Rust target has a dependency mechanism (Cargo). Checking or
    // building for C/asm/JS must warn that `dep` declarations are ignored
    // instead of dropping them silently.
    let dir = unique_temp_dir("depwarn");
    fs::create_dir_all(&dir).unwrap();
    let source = dir.join("deps.poly");
    fs::write(
        &source,
        "dep rand = \"0.8\"\n\nfn main()\n    put \"ok\"\nend fn\n",
    )
    .unwrap();

    for target in ["c", "asm", "js"] {
        let output = poly()
            .arg(&source)
            .arg("--check")
            .arg("--target")
            .arg(target)
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(&format!("the {target} target has no dependency support")),
            "expected ignored-dep warning for target {target}, got: {stderr}"
        );
    }

    // The Rust target must not emit the warning.
    let output = poly().arg(&source).arg("--check").output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("no dependency support"),
        "unexpected ignored-dep warning for rust target: {stderr}"
    );

    fs::remove_dir_all(&dir).unwrap();
}
