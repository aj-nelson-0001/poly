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

#[test]
#[cfg(target_os = "linux")]
// The asm target emits Linux x86-64 syscall assembly; as/ld exist only where
// that output can assemble.
fn asm_backend_resolves_program_scope_structs_and_enums() {
    // The asm backend must pre-register program-scope struct/enum metadata
    // before emitting function bodies, and must infer struct types for
    // annotated vars, struct-literal vars, call-return vars, and parameters.
    // Regression: layouts were registered during _start emission (after all
    // functions), so every use inside `fn main` failed with "Unknown struct".
    let dir = unique_temp_dir("asmstruct");
    fs::create_dir_all(&dir).unwrap();
    let source = dir.join("asm_structs.poly");
    fs::write(
        &source,
        concat!(
            "struct Point\n",
            "    x: i32\n",
            "    y: i32\n",
            "end struct\n",
            "\n",
            "enum Color\n",
            "    Red\n",
            "    Green\n",
            "end enum\n",
            "\n",
            "fn make(): Point\n",
            "    return Point { x: 7, y: 8 }\n",
            "end fn\n",
            "\n",
            "fn sum(p: Point): i32\n",
            "    return p.x + p.y\n",
            "end fn\n",
            "\n",
            "fn main()\n",
            "    var a := make()\n",
            "    var q Point := Point { x: 3, y: 4 }\n",
            "    var lit := Point { x: 1, y: 2 }\n",
            "    put sum(a)\n",
            "    put sum(q)\n",
            "    put lit.x\n",
            "    var c Color := Color::Green\n",
            "    match c\n",
            "        Color::Red, put 100\n",
            "        Color::Green, put 200\n",
            "        _, put 0\n",
            "    end match\n",
            "end fn\n",
        ),
    )
    .unwrap();

    let output = poly()
        .arg(&source)
        .arg("--check")
        .arg("--target")
        .arg("asm")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "--check asm with structs/enums failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // The emitted assembly must assemble and run with correct results.
    let asm_path = dir.join("out.S");
    let output = poly()
        .arg(&source)
        .arg("--target")
        .arg("asm")
        .arg("-o")
        .arg(&asm_path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "asm emission failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let obj_path = dir.join("out.o");
    let bin_path = dir.join("out");
    for (tool, args) in [
        (
            "as",
            vec!["-o", obj_path.to_str().unwrap(), asm_path.to_str().unwrap()],
        ),
        (
            "ld",
            vec!["-o", bin_path.to_str().unwrap(), obj_path.to_str().unwrap()],
        ),
    ] {
        let output = Command::new(tool).args(&args).output().unwrap();
        assert!(
            output.status.success(),
            "{tool} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let output = Command::new(&bin_path).output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.lines().collect::<Vec<_>>(),
        vec!["15", "7", "1", "200"],
        "unexpected program output"
    );

    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn match_expressions_work_on_c_and_js_targets() {
    // Match expressions as variable initializers (e.g. tetris's gravity
    // table) were rejected by the C and JS backends while Rust and asm
    // accepted them. C now lowers to a ternary chain, JS to an IIFE switch,
    // both mirroring their statement-form pattern support. The no-wildcard
    // case must also agree with the Rust target's panic-on-unmatched
    // semantics (C: exit(1); JS: thrown Error).
    let dir = unique_temp_dir("matchexpr");
    fs::create_dir_all(&dir).unwrap();
    let source = dir.join("matchexpr.poly");
    fs::write(
        &source,
        concat!(
            "fn gravity(level: i32): i32\n",
            "    var ms i32 := match level\n",
            "        0, 48\n",
            "        1, 43\n",
            "        _, 38\n",
            "    end match\n",
            "    return ms\n",
            "end fn\n",
            "\n",
            "fn main()\n",
            "    put gravity(0)\n",
            "    put gravity(1)\n",
            "    put gravity(9)\n",
            "    var m i32 := match 2 + 3\n",
            "        5, 10\n",
            "        6, 20\n",
            "        _, 0\n",
            "    end match\n",
            "    put m\n",
            "end fn\n",
        ),
    )
    .unwrap();

    // JS: emitted and executed via Node.
    let output = poly()
        .arg(&source)
        .arg("--target")
        .arg("js")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "js build failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    // The CLI prints a "Generated ... -> ...js" status line on stdout before
    // executing; compare only the program's own output lines.
    let program_lines: Vec<&str> = stdout
        .lines()
        .filter(|line| !line.starts_with("Generated"))
        .collect();
    assert_eq!(
        program_lines,
        vec!["48", "43", "38", "10"],
        "unexpected js match-expression output"
    );

    // C: emitted, compiled with cc, and executed.
    let c_path = dir.join("out.c");
    let output = poly()
        .arg(&source)
        .arg("--target")
        .arg("c")
        .arg("-o")
        .arg(&c_path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "c emission failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let bin_path = dir.join("out");
    let output = Command::new("cc")
        .arg("-o")
        .arg(&bin_path)
        .arg(&c_path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "cc failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output = Command::new(&bin_path).output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.lines().collect::<Vec<_>>(),
        vec!["48", "43", "38", "10"],
        "unexpected c match-expression output"
    );

    fs::remove_dir_all(&dir).unwrap();
}

#[test]
#[cfg(target_os = "linux")]
// The asm runtime's itoa path only exists for the Linux syscall target.
fn asm_prints_negative_integers_correctly() {
    // Regression: _print_int negated the wrong register after the minus-sign
    // syscall (negating the syscall return value instead of the number), so
    // every negative integer printed as -1. It now reduces to the absolute
    // value before the syscall and stashes it in the reserved local slot.
    let dir = unique_temp_dir("asmneg");
    fs::create_dir_all(&dir).unwrap();
    let source = dir.join("asm_neg.poly");
    fs::write(&source, "fn main()\n    put 7 - 10\n    put -5\nend fn\n").unwrap();

    let asm_path = dir.join("out.S");
    let output = poly()
        .arg(&source)
        .arg("--target")
        .arg("asm")
        .arg("-o")
        .arg(&asm_path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "asm emission failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let obj_path = dir.join("out.o");
    let bin_path = dir.join("out");
    for (tool, args) in [
        (
            "as",
            vec!["-o", obj_path.to_str().unwrap(), asm_path.to_str().unwrap()],
        ),
        (
            "ld",
            vec!["-o", bin_path.to_str().unwrap(), obj_path.to_str().unwrap()],
        ),
    ] {
        let output = Command::new(tool).args(&args).output().unwrap();
        assert!(
            output.status.success(),
            "{tool} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let output = Command::new(&bin_path).output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.lines().collect::<Vec<_>>(),
        vec!["-3", "-5"],
        "unexpected negative-integer output"
    );

    fs::remove_dir_all(&dir).unwrap();
}
