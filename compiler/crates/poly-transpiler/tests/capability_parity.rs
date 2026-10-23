//! Capability-parity tests: per-backend method-call support, pinned per
//! target.
//!
//! The five codegen backends support overlapping but different subsets of
//! method calls. These tests pin each backend's *accepted* surface so that
//! any change — a backend quietly gaining or losing a capability — fails a
//! test instead of drifting silently. The per-target expected results below
//! are derived from the backend support matrix in
//! `POLY_V2_SUPPORT_MATRIX.md` and were verified end-to-end against the
//! compiled outputs (see also `tests/diff_string_edges.poly` for
//! value-level parity of the string methods).
//!
//! The expectation enum, target list, and runner are shared with
//! `doc_claim_guards.rs` via the `common` module so the two suites cannot
//! disagree about what "supported on target X" means. To change the matrix:
//! update the backend, then update the expected rows here and the support
//! matrix doc in the same commit.

mod common;

use common::{Case, Expect};

/// Rows of the capability matrix: method-call surface per target.
const CASES: &[Case] = &[
    // --- scalar to_string -------------------------------------------------
    Case::accepted(
        "int to_string via variable",
        "fn main()\n    var n i32 := 5\n    var s := n.to_string()\n    put s.len()\nend fn\n",
        Expect::All,
    ),
    Case::accepted(
        "int to_string via literal",
        "fn main()\n    put 5.to_string()\nend fn\n",
        Expect::All,
    ),
    Case::accepted(
        "int to_string decl + put value",
        "fn main()\n    var n i32 := 5\n    var s := n.to_string()\n    put s\nend fn\n",
        Expect::All,
    ),
    Case::accepted(
        "float to_string",
        "fn main()\n    put 1.5.to_string()\nend fn\n",
        Expect::AllExceptAsm,
    ),
    Case::accepted(
        "float to_string decl",
        "fn main()\n    var s := 1.5.to_string()\n    put s\nend fn\n",
        Expect::AllExceptAsm,
    ),
    // --- string len -------------------------------------------------------
    Case::accepted(
        "string len on variable",
        "fn main()\n    var s ustring := \"abcd\"\n    put s.len()\nend fn\n",
        Expect::All,
    ),
    Case::accepted(
        "string len on parenthesized concat",
        "fn main()\n    put (\"a\" + \"b\").len()\nend fn\n\nfn double(v: i32): i32\n    return v * 2\nend fn\n",
        Expect::All,
    ),
    // --- loops ------------------------------------------------------------
    Case::accepted(
        "variable loop step",
        // Runtime steps dispatch on the step's sign on every target
        // (regression: they used to yield zero iterations silently).
        "fn main()\n    var s i32 := 2\n    loop i 0..6 step s\n        put i\n    end loop\nend fn\n",
        Expect::All,
    ),
    // --- vectors ----------------------------------------------------------
    Case::accepted(
        "vector literal len",
        "fn main()\n    var v Vec<i32> := [1, 2, 3]\n    put v.len()\nend fn\n",
        Expect::RustAsmJs,
    ),
    Case::accepted(
        "vector push",
        "fn main()\n    var v Vec<i32> := [1, 2]\n    v.push(3)\n    put v.len()\nend fn\n",
        Expect::RustAsm,
    ),
    // --- exponentiation (`^`) --------------------------------------------
    Case::with_output(
        "int power",
        "fn main()\n    var x i32 := 7\n    var square i32 := x ^ 2\n    put square\nend fn\n",
        Expect::All,
        ("rust", "wrapping_pow"),
    ),
    Case::accepted(
        "float power",
        "fn main()\n    var base f64 := 2.0\n    put base ^ 10.0\nend fn\n",
        // The asm backend has no floating-point support at all.
        Expect::RustCJs,
    ),
    Case::accepted(
        "mixed int/float power",
        "fn main()\n    var f f64 := 2.0\n    put f ^ 2\nend fn\n",
        // Same rule as `*`: the Rust target rejects mixed arithmetic, so
        // every target demands an explicit `as` cast.
        Expect::AllReject,
    ),
    // --- rejection set (same error on every target that refuses) ----------
    Case::accepted(
        "unknown method",
        "fn main()\n    var s ustring := \"ab\"\n    put s.reverse()\nend fn\n",
        Expect::AllReject,
    ),
    Case::accepted(
        "len on integer",
        "fn main()\n    var n i32 := 5\n    put n.len()\nend fn\n\nfn main2()\n    put 1\nend fn\n",
        Expect::AllReject,
    ),
];

#[test]
fn backend_capability_matrix_is_pinned() -> Result<(), Box<dyn std::error::Error>> {
    common::run_matrix(CASES);
    Ok(())
}

#[test]
fn rejection_errors_name_the_backend_or_limitation() -> Result<(), Box<dyn std::error::Error>> {
    // The refusal path must be a *clear* compile-time error, not a wrong
    // binary or a panic. Pin the distinguishing words per refusing target.
    let transpiler = poly_transpiler::Transpiler::new();
    let cases: &[(&str, &str, &str, &[&str])] = &[
        (
            // Every backend's type checker rejects before codegen.
            "unknown method",
            "fn main()\n    var s ustring := \"ab\"\n    put s.reverse()\nend fn\n",
            "rust",
            &["no method"],
        ),
        (
            // The checker's mixed-class rule fires before codegen.
            "mixed int/float power",
            "fn main()\n    var f f64 := 2.0\n    put f ^ 2\nend fn\n",
            "rust",
            &["explicit cast"],
        ),
        (
            "vector push on c",
            "fn main()\n    var v Vec<i32> := [1, 2]\n    v.push(3)\nend fn\n",
            "c",
            &["not supported by the C backend"],
        ),
        (
            "vector push on js",
            "fn main()\n    var v Vec<i32> := [1, 2]\n    v.push(3)\nend fn\n",
            "js",
            &["JS backend does not support method calls"],
        ),
        (
            // Goose has `__poly_pow` for integers only; float `^` points at
            // the escape hatch just like the other non-rust backends.
            "float power on goose",
            "fn main()\n    var base f64 := 2.0\n    put base ^ 10.0\nend fn\n",
            "goose",
            &["float exponentiation", "#goose"],
        ),
        (
            "get input on goose",
            "fn main()\n    var x := get\n    put x\nend fn\n",
            "goose",
            &["`get` input", "#goose"],
        ),
    ];
    for (name, source, target, needles) in cases {
        let err = match transpiler.transpile_target(source, target) {
            Err(err) => err,
            Ok(_) => panic!("case `{name}` unexpectedly compiled"),
        };
        for needle in needles.iter() {
            assert!(
                err.contains(needle),
                "case `{name}`: error should mention {needle:?}, got: {err}"
            );
        }
    }
    Ok(())
}
