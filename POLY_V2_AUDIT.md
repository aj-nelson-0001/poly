# Poly 2.0 Preview Audit

Updated 2026-08-22.

## Milestone Status

The preview hardening milestone is complete: the C fixture now executes in CI,
C unsupported features have explicit diagnostics, Rust/C target selection is
covered, and `#cpp` is rejected clearly until a backend exists.

## Completed In This Pass

- Replaced the single-language passthrough node with a language-tagged foreign block representation.
- Enforced top-level foreign block placement in the parser.
- Made block end markers line-oriented and opaque inside foreign content.
- Added target selection for Rust and C, including mixed-source filtering.
- Added a standalone C backend for scalar Poly orchestration, plain structs, functions, conditions, loops, arithmetic, and stdout/stderr output.
- Added C syntax checking, native builds, `--emit-c`, and C project generation.
- Repaired the stale append-redirect regression test.
- Added lexer, parser, C backend, CLI target, and C fixture coverage.

## Remaining Risks, In Priority Order

1. **Opaque foreign calls remain permissive, while explicit signatures are available.** Poly accepts legacy calls after extracting names, but argument and return compatibility is delegated entirely to Rust or C unless the source adds `extern rust fn ...` or `extern c fn ...`.
2. **C backend scope is intentionally narrow.** File redirects, stdin, vectors, tuples, pattern matching, closures, async, and complex Poly types should use `#c` helpers or remain unsupported until their C representation is designed.
3. **C++ is syntax-only.** `#cpp` blocks are recognized and preserved in the AST, but no C++ target or native build path exists.
4. **The v2 preview still has compatibility-oriented parser paths.** They are covered by tests and kept for migration diagnostics; future cleanup should remove only paths proven unused by the test suite.
5. **The old v1 documentation set is historical.** It is now indexed and marked as non-normative; the v2 index, spec, grammar, migration guide, and target documents are the maintained sources of truth.
6. **Cross-platform C coverage is now part of the contract.** CI exercises the Rust/C workflow on Linux, macOS, and Windows with an explicit `POLY_CC` compiler. Platform-specific native compiler behavior remains a residual risk and should be reviewed when CI toolchain images change.

## Recommended Next Milestone

The next milestone is preview release hardening under the frozen [support matrix](POLY_V2_SUPPORT_MATRIX.md): finish cross-platform CI, use explicit foreign signatures where stronger diagnostics are needed, and keep C++ out of scope until its emission and build model is explicitly designed.
