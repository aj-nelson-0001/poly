//! Capability-parity tests: per-backend method-call support, pinned per
//! target.
//!
//! The four codegen backends support overlapping but different subsets of
//! method calls. These tests pin each backend's *accepted* surface so that
//! any change — a backend quietly gaining or losing a capability — fails a
//! test instead of drifting silently. The per-target expected results below
//! are derived from the backend support matrix in
//! `POLY_V2_SUPPORT_MATRIX.md` and were verified end-to-end against the
//! compiled outputs (see also `tests/diff_string_edges.poly` for
//! value-level parity of the string methods).
//!
//! To change the matrix: update the backend, then update the expected rows
//! here and the support matrix doc in the same commit.

use poly_transpiler::Transpiler;

/// Row of the capability matrix: (case name, source, per-target expectation).
///
/// `Expect::Accept`  — the backend must compile this source.
/// `Expect::Reject`  — the backend must refuse (any error text is fine;
///                     the error usually names the limitation).
const CASES: &[(&str, &str, Expect)] = &[
    // --- scalar to_string -------------------------------------------------
    (
        "int to_string via variable",
        "fn main()\n    var n i32 := 5\n    var s := n.to_string()\n    put s.len()\nend fn\n",
        Expect::All,
    ),
    (
        "int to_string via literal",
        "fn main()\n    put 5.to_string()\nend fn\n",
        Expect::All,
    ),
    (
        "int to_string decl + put value",
        "fn main()\n    var n i32 := 5\n    var s := n.to_string()\n    put s\nend fn\n",
        Expect::All,
    ),
    (
        "float to_string",
        "fn main()\n    put 1.5.to_string()\nend fn\n",
        Expect::AllExceptAsm,
    ),
    (
        "float to_string decl",
        "fn main()\n    var s := 1.5.to_string()\n    put s\nend fn\n",
        Expect::AllExceptAsm,
    ),
    // --- string len -------------------------------------------------------
    (
        "string len on variable",
        "fn main()\n    var s ustring := \"abcd\"\n    put s.len()\nend fn\n",
        Expect::All,
    ),
    (
        "string len on parenthesized concat",
        "fn main()\n    put (\"a\" + \"b\").len()\nend fn\n\nfn double(v: i32): i32\n    return v * 2\nend fn\n",
        Expect::All,
    ),
    // --- vectors ----------------------------------------------------------
    (
        "vector literal len",
        "fn main()\n    var v Vec<i32> := [1, 2, 3]\n    put v.len()\nend fn\n",
        Expect::RustAsmJs,
    ),
    (
        "vector push",
        "fn main()\n    var v Vec<i32> := [1, 2]\n    v.push(3)\n    put v.len()\nend fn\n",
        Expect::RustAsm,
    ),
    // --- rejection set (same error on every target that refuses) ----------
    (
        "unknown method",
        "fn main()\n    var s ustring := \"ab\"\n    put s.reverse()\nend fn\n",
        Expect::AllReject,
    ),
    (
        "len on integer",
        "fn main()\n    var n i32 := 5\n    put n.len()\nend fn\n\nfn main2()\n    put 1\nend fn\n",
        Expect::AllReject,
    ),
];

/// Per-target expectation for a matrix row.
#[derive(Clone, Copy)]
enum Expect {
    /// Accepted by all four backends.
    All,
    /// Accepted by rust, c, js; asm refuses (its scalar MethodCall gate
    /// covers only integer `.to_string()`).
    AllExceptAsm,
    /// Accepted by rust, asm, js; c has no vector support at all.
    RustAsmJs,
    /// Accepted by rust and asm only: c has no vectors, and the JS backend
    /// supports vector `.len()` but no mutators like `.push`.
    RustAsm,
    /// Rejected by the type checker before codegen — identical behavior on
    /// every target (used for cases whose acceptance would diverge).
    AllReject,
}

impl Expect {
    fn accepted(self, target: &str) -> bool {
        match self {
            Expect::All => true,
            Expect::AllExceptAsm => target != "asm",
            Expect::RustAsmJs => target != "c",
            Expect::RustAsm => matches!(target, "rust" | "asm"),
            Expect::AllReject => false,
        }
    }
}

const TARGETS: &[&str] = &["rust", "c", "asm", "js"];

#[test]
fn backend_capability_matrix_is_pinned() {
    let transpiler = Transpiler::new();
    for (name, source, expect) in CASES {
        for target in TARGETS {
            let result = transpiler.transpile_target(source, target);
            match (&result, expect.accepted(target)) {
                (Ok(_), true) => {}
                (Err(_), false) => {}
                (Ok(_), false) => panic!(
                    "capability matrix drift: case `{name}` compiled on `{target}` \
                     but is pinned as rejected — if intentional, update the \
                     backend docs and this matrix"
                ),
                (Err(err), true) => panic!(
                    "capability matrix drift: case `{name}` failed on `{target}` \
                     but is pinned as accepted — error: {err}"
                ),
            }
        }
    }
}

#[test]
fn rejection_errors_name_the_backend_or_limitation() {
    // The refusal path must be a *clear* compile-time error, not a wrong
    // binary or a panic. Pin the distinguishing words per refusing target.
    let transpiler = Transpiler::new();
    let cases: &[(&str, &str, &str, &[&str])] = &[
        (
            // Every backend's type checker rejects before codegen.
            "unknown method",
            "fn main()\n    var s ustring := \"ab\"\n    put s.reverse()\nend fn\n",
            "rust",
            &["no method"],
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
}
