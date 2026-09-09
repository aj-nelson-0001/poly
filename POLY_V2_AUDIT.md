# Poly 2.0 Preview Audit

Updated 2026-09-08.

## Milestone Status

The preview hardening milestone is complete: the C fixture now executes in CI,
C unsupported features have explicit diagnostics, Rust/C target selection is
covered, and `#cpp` is rejected clearly until a backend exists.

Since `2.0.0-preview.1`, the follow-up passes added an assembly target
(`--target asm` with `extern asm` declarations, descriptor-backed vectors,
pointer dereference, and string for-in), widened the C backend (tuples, match
statements, struct literals, enum variants in match, plain stdin `get`,
non-capturing closures), added language-wide `pop()`, and made source maps and
LSP symbol ranges span-precise.

## Completed In This Pass

- Added a JavaScript backend (`poly-js-codegen`, `--target js`) lowering the C-target orchestration subset to dependency-free ES2020, with `#js` blocks, `extern js fn` declarations, `--emit-js`, JS project generation, unit/snapshot/fixture tests, and a Linux CI job that verifies emitted output with `node --check` plus execution.
- Adopted strict mode across CI: all examples and fixtures now declare their foreign calls, and every CI check runs `--strict`.
- Fixed a pre-existing Windows-only snapshot failure by normalizing line endings before comparison.
- Replaced the single-language passthrough node with a language-tagged foreign block representation.
- Enforced top-level foreign block placement in the parser.
- Made block end markers line-oriented and opaque inside foreign content.
- Added target selection for Rust and C, including mixed-source filtering.
- Added a standalone C backend for scalar Poly orchestration, plain structs, functions, conditions, loops, arithmetic, and stdout/stderr output.
- Added C syntax checking, native builds, `--emit-c`, and C project generation.
- Repaired the stale append-redirect regression test.
- Added lexer, parser, C backend, CLI target, and C fixture coverage.

## Remaining Risks, In Priority Order

1. **Opaque foreign calls remain permissive by default, while explicit signatures are available.** Poly accepts legacy calls after extracting names, but argument and return compatibility is delegated entirely to the native target compiler unless the source adds `extern rust fn ...`, `extern c fn ...`, `extern asm fn ...`, or `extern js fn ...`. Since 2026-09-09, `poly --strict` rejects calls to foreign functions that lack an explicit `extern <target> fn ...` declaration (ordinary Poly declarations and builtins are unaffected), giving CI and audits an enforcement path; the default remains permissive for compatibility. All repository examples and fixtures now carry explicit declarations, and every CI `--check` step runs with `--strict`.
2. **C backend scope remains intentionally narrow.** File redirects, file input, vectors, async, capturing closures, and complex Poly types should use `#c` helpers or remain unsupported until their C representation is designed. Tuples, match statements, struct literals, enum variants, plain `get`, and non-capturing closures are supported as of `2.0.0-preview.2`.
3. **C++ is syntax-only.** `#cpp` blocks are recognized and preserved in the AST, but no C++ target or native build path exists.
4. **The v2 preview still has compatibility-oriented parser paths.** They are covered by tests and kept for migration diagnostics; future cleanup should remove only paths proven unused by the test suite.
5. **The asm target is a narrow Linux x86-64 subset.** It emits freestanding assembly with Linux syscalls, runs under CI on Linux only, and rejects general string values, maps/sets, and unsupported methods with guidance. Grow-heap exhaustion exits with status 42 rather than corrupting memory.
6. **The old v1 documentation set is historical.** It is now indexed and marked as non-normative; the v2 index, spec, grammar, migration guide, and target documents are the maintained sources of truth.
7. **Cross-platform C coverage is now part of the contract.** CI exercises the Rust/C workflow on Linux, macOS, and Windows with an explicit `POLY_CC` compiler. Platform-specific native compiler behavior remains a residual risk and should be reviewed when CI toolchain images change.

## Recommended Next Milestone

The next milestone is preview release hardening under the frozen [support matrix](POLY_V2_SUPPORT_MATRIX.md): finish cross-platform CI, use explicit foreign signatures where stronger diagnostics are needed, and keep C++ out of scope until its emission and build model is explicitly designed.
