//! Documentation-claim guards: pin the per-backend behavior claims that the
//! maintained docs (README, POLY_SPEC_v2, POLY_GRAMMAR, POLY_QUICK_REFERENCE,
//! POLY_API_REFERENCE, POLY_CHEATSHEET, POLY_JS_BLOCKS,
//! POLY_V2_SUPPORT_MATRIX, POLY_MIGRATION_GUIDE_v2) make about the four
//! targets.
//!
//! Each case was verified end-to-end during the 2.0.0-preview.12
//! documentation audit (probes compiled through every backend, several built
//! and executed). If one of these fails, either the backend changed or the
//! docs did — update the backend, the affected docs, and the pinned
//! expectation here in the same commit so the documentation stays truthful.

use poly_transpiler::Transpiler;

const TARGETS: &[&str] = &["rust", "c", "asm", "js"];

/// A claim to pin: (name, Poly source, expectation).
enum Claim {
    /// Every backend compiles this.
    All,
    /// Every backend compiles this; the JS output must additionally contain
    /// the given needle (proves the construct is really lowered, not just
    /// accepted-and-ignored).
    AllWithJs(&'static str),
    /// Rust and C compile this (asm and JS reject stdin input); the C output
    /// must additionally contain the given needle.
    RustCWithC(&'static str),
    /// Rust, C, and JS compile this; asm rejects the construct (e.g. tuples).
    RustCJs,
    /// Only Rust compiles this (e.g. file redirects and file input).
    RustOnly,
    /// Rust, asm, and JS compile this; C has no vector support (e.g. vector
    /// literal + len).
    RustAsmJs,
    /// Rust and asm compile this; C and JS reject it (e.g. vector push).
    RustAsm,
    /// Rust, C, and asm compile this; JS rejects it (e.g. unit enums).
    RustCAsm,
    /// Rust and C compile this; asm and JS reject it (e.g. non-capturing
    /// closures).
    RustC,
    /// Every backend must refuse this at compile time (docs pin it as an
    /// error, not a runtime behavior).
    AllReject,
}

const CASES: &[(&str, &str, Claim)] = &[
    // --- corrected claim: interpolation is NOT rust-only -------------------
    (
        "string interpolation works on all targets",
        "fn main()\n    var n i32 := 42\n    put \"n is {n} and done\"\nend fn\n",
        Claim::All,
    ),
    // --- corrected claim: C backend supports plain stdin `get` -------------
    (
        "c get with prompt",
        "fn main()\n    var name ustring := get unicode \"Name: \"\n    put name\nend fn\n",
        Claim::RustCWithC("__poly_get_line(\"Name: \")"),
    ),
    (
        "c get without prompt",
        // Regression for the 0-args-to-1-arg-helper bug: the generated C
        // must call the helper with an explicit empty string literal.
        "fn main()\n    var line := get\n    put line\nend fn\n",
        Claim::RustCWithC("__poly_get_line(\"\")"),
    ),
    // --- corrected claim: JS backend supports structs ----------------------
    (
        "js struct declaration and literal",
        "struct Point\n    x: i32\n    y: i32\nend struct\n\nfn main()\n    var p := Point { x: 3, y: 4 }\n    put p.x + p.y\nend fn\n",
        Claim::AllWithJs("Point"),
    ),
    // --- corrected claim: JS backend supports tuples -----------------------
    (
        "js tuple literal and index access",
        "fn main()\n    var t := (10, 20)\n    put t.0\n    put t.1\nend fn\n",
        Claim::RustCJs,
    ),
    // --- corrected claim: match expressions and typed tuples are portable --
    (
        "match expression on all targets",
        "fn main()\n    var m := match 1\n        0, 100\n        _, 200\n    end match\n    put m\nend fn\n",
        Claim::All,
    ),
    (
        "typed tuple on all targets",
        "fn main()\n    var pair (i32, i32) := (1, 2)\n    put pair.0\nend fn\n",
        Claim::RustCJs,
    ),
    // --- corrected claim: `..=` inclusive range loops on all targets -------
    (
        "inclusive range loop",
        "fn main()\n    loop j 0..=4\n        put j\n    end loop\nend fn\n",
        Claim::All,
    ),
    // --- corrected claim: a literal zero step is a compile-time error ------
    // (docs previously said it "yields zero iterations"; only runtime zero
    // steps do — see the next case).
    (
        "literal zero step is rejected",
        "fn main()\n    loop i 0..10 step 0\n        put i\n    end loop\nend fn\n",
        Claim::AllReject,
    ),
    (
        "runtime zero step compiles everywhere",
        "fn main()\n    var s i32 := 0\n    loop i 0..10 step s\n        put i\n    end loop\n    put \"after\"\nend fn\n",
        Claim::All,
    ),
    // --- rows pinned from POLY_V2_SUPPORT_MATRIX.md (probed 2026-09-20) ----
    // Enum declarations: C/asm support unit variants (payload-carrying are
    // rejected); JS rejects enums entirely.
    (
        "unit enum declaration and qualified match",
        "enum Color\n    Red\n    Green\nend enum\n\nfn main()\n    var c := Color::Green\n    match c\n        Color::Red, put 1\n        Color::Green, put 2\n        _, put 0\n    end match\nend fn\n",
        Claim::RustCAsm,
    ),
    // Closures: C compiles non-capturing only; capturing are rejected with
    // guidance; asm and JS reject all closures.
    (
        "non-capturing closure",
        "fn main()\n    var twice := |x: i32| x * 2\n    put twice(21)\nend fn\n",
        Claim::RustC,
    ),
    (
        "capturing closure is rejected outside rust",
        "fn main()\n    var n i32 := 10\n    var addn := |x: i32| x + n\n    put addn(5)\nend fn\n",
        Claim::RustOnly,
    ),
    // Vectors: C has none; JS supports literals/len but no mutators; asm
    // supports literals and mutators via its growable runtime.
    (
        "vector literal and len",
        "fn main()\n    var v Vec<i32> := [1, 2, 3]\n    put v.len()\nend fn\n",
        Claim::RustAsmJs,
    ),
    (
        "vector push",
        "fn main()\n    var v Vec<i32> := [1, 2]\n    v.push(3)\n    put v.len()\nend fn\n",
        Claim::RustAsm,
    ),
    // File I/O: redirects and file input are Rust-target features; C, asm,
    // and JS reject them with guidance.
    (
        "file output redirect",
        "fn main()\n    put \"x\" to \"out.txt\"\nend fn\n",
        Claim::RustOnly,
    ),
    (
        "file input via get from",
        "fn main()\n    var d bytes := get from \"data.bin\" --bytes 8\n    put d.len()\nend fn\n",
        Claim::RustOnly,
    ),
];

impl Claim {
    /// Whether `target` is expected to accept the case's source.
    fn accepted(&self, target: &str) -> bool {
        match self {
            Claim::All | Claim::AllWithJs(_) => true,
            Claim::RustCWithC(_) => matches!(target, "rust" | "c"),
            Claim::RustCJs => matches!(target, "rust" | "c" | "js"),
            Claim::RustOnly => target == "rust",
            Claim::RustAsmJs => matches!(target, "rust" | "asm" | "js"),
            Claim::RustAsm => matches!(target, "rust" | "asm"),
            Claim::RustCAsm => !matches!(target, "js"),
            Claim::RustC => matches!(target, "rust" | "c"),
            Claim::AllReject => false,
        }
    }
}

#[test]
fn doc_pinned_claims_hold_on_every_target() {
    let transpiler = Transpiler::new();
    for (name, source, claim) in CASES {
        for target in TARGETS {
            let result = transpiler.transpile_target(source, target);
            if !claim.accepted(target) {
                assert!(
                    result.is_err(),
                    "doc-claim drift: case `{name}` compiled on `{target}` but the docs pin it \
                     as unsupported — update the backend, the affected docs, and this guard"
                );
                continue;
            }
            match (claim, result) {
                (Claim::AllReject, Err(_)) => {}
                (Claim::AllReject, Ok(_)) => panic!(
                    "doc-claim drift: case `{name}` compiled on `{target}` but the docs pin it \
                     as a compile error — update the backend, the affected docs, and this guard"
                ),
                (_, Err(err)) => panic!(
                    "doc-claim drift: case `{name}` failed on `{target}` but the docs pin it \
                     as supported — error: {err}"
                ),
                (Claim::All, Ok(_)) => {}
                (Claim::AllWithJs(needle), Ok(output)) if *target == "js" => assert!(
                    output.contains(needle),
                    "case `{name}`: js output should contain {needle:?}\n{output}"
                ),
                (Claim::RustCWithC(needle), Ok(output)) if *target == "c" => assert!(
                    output.contains(needle),
                    "case `{name}`: c output should contain {needle:?}\n{output}"
                ),
                (_, Ok(_)) => {}
            }
        }
    }
}

#[test]
fn zero_step_rejection_names_the_limitation() {
    // The literal-zero-step refusal must be a clear compile-time error on
    // every target, matching POLY_GRAMMAR.md / POLY_SPEC_v2.md wording.
    let transpiler = Transpiler::new();
    let source = "fn main()\n    loop i 0..10 step 0\n        put i\n    end loop\nend fn\n";
    for target in TARGETS {
        let err = match transpiler.transpile_target(source, target) {
            Err(err) => err,
            Ok(_) => panic!("literal zero step unexpectedly compiled on `{target}`"),
        };
        assert!(
            err.to_lowercase().contains("zero"),
            "target `{target}`: zero-step error should mention \"zero\", got: {err}"
        );
    }
}
