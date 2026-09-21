//! Documentation-claim guards: pin the per-backend behavior claims that the
//! maintained docs (README, POLY_SPEC_v2, POLY_GRAMMAR, POLY_QUICK_REFERENCE,
//! POLY_API_REFERENCE, POLY_CHEATSHEET, POLY_JS_BLOCKS,
//! POLY_V2_SUPPORT_MATRIX, POLY_MIGRATION_GUIDE_v2) make about the four
//! targets.
//!
//! Every case was verified end-to-end during the 2.0.0-preview.12
//! documentation audit (probes compiled through every backend, several built
//! and executed). The expectation enum, target list, and runner live in the
//! shared `common` module alongside `capability_parity.rs`, so the two
//! suites cannot disagree about what "supported on target X" means. If one
//! of these tests fails, either the backend changed or the docs did —
//! update the backend, the affected docs, and the pinned expectation in the
//! same commit so the documentation stays truthful.

mod common;

use common::{Case, Expect};

const CASES: &[Case] = &[
    // --- corrected claim: interpolation is NOT rust-only -------------------
    Case::accepted(
        "string interpolation works on all targets",
        "fn main()\n    var n i32 := 42\n    put \"n is {n} and done\"\nend fn\n",
        Expect::All,
    ),
    // --- corrected claim: C backend supports plain stdin `get` -------------
    Case::with_output(
        "c get with prompt",
        "fn main()\n    var name ustring := get unicode \"Name: \"\n    put name\nend fn\n",
        Expect::RustC,
        ("c", "__poly_get_line(\"Name: \")"),
    ),
    Case::with_output(
        "c get without prompt",
        // Regression for the 0-args-to-1-arg-helper bug: the generated C
        // must call the helper with an explicit empty string literal.
        "fn main()\n    var line := get\n    put line\nend fn\n",
        Expect::RustC,
        ("c", "__poly_get_line(\"\")"),
    ),
    // --- corrected claim: JS backend supports structs ----------------------
    Case::with_output(
        "js struct declaration and literal",
        "struct Point\n    x: i32\n    y: i32\nend struct\n\nfn main()\n    var p := Point { x: 3, y: 4 }\n    put p.x + p.y\nend fn\n",
        Expect::All,
        ("js", "Point"),
    ),
    // --- corrected claim: JS backend supports tuples -----------------------
    Case::accepted(
        "js tuple literal and index access",
        "fn main()\n    var t := (10, 20)\n    put t.0\n    put t.1\nend fn\n",
        Expect::RustCJs,
    ),
    // --- corrected claim: match expressions and typed tuples are portable --
    Case::accepted(
        "match expression on all targets",
        "fn main()\n    var m := match 1\n        0, 100\n        _, 200\n    end match\n    put m\nend fn\n",
        Expect::All,
    ),
    Case::accepted(
        "typed tuple on all targets",
        "fn main()\n    var pair (i32, i32) := (1, 2)\n    put pair.0\nend fn\n",
        Expect::RustCJs,
    ),
    // --- corrected claim: `..=` inclusive range loops on all targets -------
    Case::accepted(
        "inclusive range loop",
        "fn main()\n    loop j 0..=4\n        put j\n    end loop\nend fn\n",
        Expect::All,
    ),
    // --- corrected claim: a literal zero step is a compile-time error ------
    // (docs previously said it "yields zero iterations"; only runtime zero
    // steps do — see the next case).
    Case::accepted(
        "literal zero step is rejected",
        "fn main()\n    loop i 0..10 step 0\n        put i\n    end loop\nend fn\n",
        Expect::AllReject,
    ),
    Case::accepted(
        "runtime zero step compiles everywhere",
        "fn main()\n    var s i32 := 0\n    loop i 0..10 step s\n        put i\n    end loop\n    put \"after\"\nend fn\n",
        Expect::All,
    ),
    // --- rows pinned from POLY_V2_SUPPORT_MATRIX.md (probed 2026-09-20) ----
    // Enum declarations: C/asm support unit variants (payload-carrying are
    // rejected); JS rejects enums entirely.
    Case::accepted(
        "unit enum declaration and qualified match",
        "enum Color\n    Red\n    Green\nend enum\n\nfn main()\n    var c := Color::Green\n    match c\n        Color::Red, put 1\n        Color::Green, put 2\n        _, put 0\n    end match\nend fn\n",
        Expect::RustCAsm,
    ),
    // Closures: C compiles non-capturing only; capturing are rejected with
    // guidance; asm and JS reject all closures.
    Case::accepted(
        "non-capturing closure",
        "fn main()\n    var twice := |x: i32| x * 2\n    put twice(21)\nend fn\n",
        Expect::RustC,
    ),
    Case::accepted(
        "capturing closure is rejected outside rust",
        "fn main()\n    var n i32 := 10\n    var addn := |x: i32| x + n\n    put addn(5)\nend fn\n",
        Expect::RustOnly,
    ),
    // Vectors: C has none; JS supports literals/len but no mutators; asm
    // supports literals and mutators via its growable runtime.
    Case::accepted(
        "vector literal and len",
        "fn main()\n    var v Vec<i32> := [1, 2, 3]\n    put v.len()\nend fn\n",
        Expect::RustAsmJs,
    ),
    Case::accepted(
        "vector push",
        "fn main()\n    var v Vec<i32> := [1, 2]\n    v.push(3)\n    put v.len()\nend fn\n",
        Expect::RustAsm,
    ),
    // --- comment styles (POLY_QUICK_REFERENCE.md, POLY_TUTORIAL.md) -------
    // All three comment styles are stripped by the lexer before any backend
    // sees the token stream, so every target must accept them identically —
    // and the statements around each comment must survive compilation.
    Case::accepted(
        "hash line comments on every target",
        "# leading comment\nfn main()\n    var n i32 := 1 # trailing comment\n    # standalone comment\n    put n\nend fn\n",
        Expect::All,
    ),
    Case::accepted(
        "c++-style line comments on every target",
        "// leading comment\nfn main()\n    var n i32 := 2 // trailing comment\n    // standalone comment\n    put n\nend fn\n",
        Expect::All,
    ),
    Case::accepted(
        "block comments on every target",
        "/* leading\n   multi-line */\nfn main()\n    /* inline */ var n i32 := 3\n    put n\nend fn\n",
        Expect::All,
    ),
    // Async: a Rust-target feature (Tokio). C, asm, and JS reject async
    // functions with guidance pointing at foreign blocks (POLY_TUTORIAL.md
    // Part 7, POLY_V2_SUPPORT_MATRIX.md).
    Case::accepted(
        "async fn with await and spawn on rust",
        "async fn compute(x: i32): i32\n    delay(10).await\n    return x * x\nend fn\n\nasync fn main()\n    var a := compute(3).await\n    spawn compute(5)\nend fn\n",
        Expect::RustOnly,
    ),
    // --- named-field enum payload matching (regression: E0164) ------------
    // Variants declared with named fields compile as Rust struct variants,
    // but matches used to render tuple patterns, failing rustc with E0164
    // ("expected tuple struct or tuple variant, found struct variant").
    // Both unqualified and qualified arms must emit `{ field: binding }`.
    Case::with_output(
        "named-field payload match, unqualified arm",
        "enum Badge\n    Level(n: i32)\n    Stars(count: i32)\nend enum\n\nfn main()\n    var b := Badge::Level(3)\n    match b\n        Level(n), put \"level \" + n.to_string()\n        Stars(count), put \"stars \" + count.to_string()\n        _, put \"none\"\n    end match\nend fn\n",
        Expect::RustAsm,
        ("rust", "Badge::Level { n: n }"),
    ),
    Case::with_output(
        "named-field payload match, qualified arm",
        "enum Badge\n    Level(n: i32)\n    None\nend enum\n\nfn main()\n    var b := Badge::Level(3)\n    match b\n        Badge::Level(n), put \"level \" + n.to_string()\n        _, put \"none\"\n    end match\nend fn\n",
        Expect::RustAsm,
        ("rust", "Badge::Level { n: n }"),
    ),
    // File I/O: redirects and file input are Rust-target features; C, asm,
    // and JS reject them with guidance.
    Case::accepted(
        "file output redirect",
        "fn main()\n    put \"x\" to \"out.txt\"\nend fn\n",
        Expect::RustOnly,
    ),
    Case::accepted(
        "file input via get from",
        "fn main()\n    var d bytes := get from \"data.bin\" --bytes 8\n    put d.len()\nend fn\n",
        Expect::RustOnly,
    ),
];

#[test]
fn doc_pinned_claims_hold_on_every_target() {
    common::run_matrix(CASES);
}

#[test]
fn zero_step_rejection_names_the_limitation() {
    // The literal-zero-step refusal must be a clear compile-time error on
    // every target, matching POLY_GRAMMAR.md / POLY_SPEC_v2.md wording.
    let transpiler = poly_transpiler::Transpiler::new();
    let source = "fn main()\n    loop i 0..10 step 0\n        put i\n    end loop\nend fn\n";
    for target in common::TARGETS {
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

#[test]
fn async_rejection_names_the_foreign_block_escape_hatch() {
    // C, asm, and JS must refuse async functions with guidance pointing at
    // a foreign block (POLY_TUTORIAL.md Part 7); rust must accept them.
    let transpiler = poly_transpiler::Transpiler::new();
    let source = "async fn compute(x: i32): i32\n    delay(10).await\n    return x * x\nend fn\n\nfn main()\n    var a := compute(3).await\nend fn\n";
    assert!(
        transpiler.transpile_target(source, "rust").is_ok(),
        "async fn should compile on the rust target"
    );
    for (target, block) in [("c", "#c"), ("asm", "#asm"), ("js", "#js")] {
        let err = match transpiler.transpile_target(source, target) {
            Err(err) => err,
            Ok(_) => panic!("async fn unexpectedly compiled on `{target}`"),
        };
        let message = err.to_lowercase();
        assert!(
            message.contains("async") && message.contains(block),
            "target `{target}`: async rejection should mention async and {block}, got: {err}"
        );
    }
}
