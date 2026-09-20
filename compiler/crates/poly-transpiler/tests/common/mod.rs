//! Shared backend capability matrix for the integration test suites.
//!
//! Both `capability_parity.rs` (method-call surface) and
//! `doc_claim_guards.rs` (documentation claims) pin the same thing: which of
//! the four codegen backends accept which Poly constructs. This module holds
//! the single definition of the per-target expectation enum, the target
//! list, and the runner, so the two suites cannot disagree about what
//! "supported on target X" means.
//!
//! To change the matrix: update the backend, then update the affected cases
//! here and the support matrix doc (POLY_V2_SUPPORT_MATRIX.md) in the same
//! commit.
//!
//! The expectations below were verified end-to-end against the compiled
//! outputs (probes through `poly --target <t> --emit-<t>`, several built and
//! executed) during the 2.0.0-preview.12 documentation audit.

// Each integration-test binary compiles this module independently, so items
// used only by the sibling suite appear dead here. That is inherent to the
// shared-module structure, not a real dead-code signal.
#![allow(dead_code)]

use poly_transpiler::Transpiler;

pub const TARGETS: &[&str] = &["rust", "c", "asm", "js"];

/// Per-target acceptance for a matrix row.
#[derive(Clone, Copy)]
pub enum Expect {
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
    /// Accepted by rust, c, js; asm rejects the construct (e.g. tuples).
    RustCJs,
    /// Accepted by rust, c, asm; JS rejects it (e.g. unit enums).
    RustCAsm,
    /// Accepted by rust and c only (e.g. non-capturing closures, stdin `get`).
    RustC,
    /// Accepted by rust only (e.g. file redirects, file input, capturing
    /// closures).
    RustOnly,
    /// Rejected by every target — either the type checker refuses it before
    /// codegen (identical behavior everywhere) or the docs pin it as a
    /// compile error on all backends.
    AllReject,
}

impl Expect {
    pub fn accepted(self, target: &str) -> bool {
        match self {
            Expect::All => true,
            Expect::AllExceptAsm => target != "asm",
            Expect::RustAsmJs => matches!(target, "rust" | "asm" | "js"),
            Expect::RustAsm => matches!(target, "rust" | "asm"),
            Expect::RustCJs => matches!(target, "rust" | "c" | "js"),
            Expect::RustCAsm => target != "js",
            Expect::RustC => matches!(target, "rust" | "c"),
            Expect::RustOnly => target == "rust",
            Expect::AllReject => false,
        }
    }
}

/// One pinned row of the capability matrix.
pub struct Case {
    /// Human-readable name used in drift diagnostics.
    pub name: &'static str,
    /// Complete Poly program the case compiles.
    pub source: &'static str,
    /// Which targets must accept the source.
    pub expect: Expect,
    /// Optional output assertion: `(target, needle)`. When `expect` accepts
    /// `target`, the generated output must contain `needle` — this proves
    /// the construct is really lowered, not just accepted-and-ignored.
    pub needle: Option<(&'static str, &'static str)>,
}

impl Case {
    /// A case with no output assertion.
    pub const fn accepted(name: &'static str, source: &'static str, expect: Expect) -> Self {
        Case {
            name,
            source,
            expect,
            needle: None,
        }
    }

    /// A case whose generated output on `needle.0` must contain `needle.1`.
    pub const fn with_output(
        name: &'static str,
        source: &'static str,
        expect: Expect,
        needle: (&'static str, &'static str),
    ) -> Self {
        Case {
            name,
            source,
            expect,
            needle: Some(needle),
        }
    }
}

/// Run every case against every target, panicking on any drift between the
/// pinned expectation and actual transpiler behavior.
pub fn run_matrix(cases: &[Case]) {
    let transpiler = Transpiler::new();
    for case in cases {
        for target in TARGETS {
            let result = transpiler.transpile_target(case.source, target);
            if case.expect.accepted(target) {
                let output = match result {
                    Ok(output) => output,
                    Err(err) => panic!(
                        "capability matrix drift: case `{}` failed on `{}` but is pinned as \
                         accepted — if intentional, update the backend docs and this matrix; \
                         error: {err}",
                        case.name, target
                    ),
                };
                if let Some((needle_target, needle)) = case.needle {
                    if *target == needle_target {
                        assert!(
                            output.contains(needle),
                            "case `{}`: {} output should contain {:?}\n{}",
                            case.name,
                            target,
                            needle,
                            output
                        );
                    }
                }
            } else {
                assert!(
                    result.is_err(),
                    "capability matrix drift: case `{}` compiled on `{}` but is pinned as \
                     rejected — if intentional, update the backend docs and this matrix",
                    case.name,
                    target
                );
            }
        }
    }
}
