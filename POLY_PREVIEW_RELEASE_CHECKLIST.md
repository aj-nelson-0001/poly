# Poly v2 Preview Release Checklist

**Target release:** Poly `2.0.0-preview.2`

This checklist describes release readiness for the current preview. It does not create a release or tag one automatically.

## Contract

- [x] Review `POLY_SPEC_v2.md` and `POLY_V2_SUPPORT_MATRIX.md` together.
- [x] Confirm Rust remains the default, C remains the documented C11 subset, and asm remains the documented Linux x86-64 subset.
- [x] Confirm `#cpp` remains explicitly rejected; do not advertise a C++ backend.
- [x] Confirm all unsupported constructs have actionable diagnostics.
- [x] Confirm `extern rust fn`, `extern c fn`, and `extern asm fn` declarations are documented as opt-in interface checking.
- [x] Confirm historical v1 documents are linked through `POLY_DOCUMENTATION_INDEX.md` and are not presented as current syntax.

## Implementation

- [x] No known builtin lowers to a placeholder value or emits an “implemented” warning while doing nothing.
- [x] Opaque foreign calls remain compatible with existing v2 source.
- [x] Explicit foreign signatures validate Poly-visible arity and types.
- [x] Target filtering removes non-selected foreign blocks and extern declarations before checking/codegen.
- [x] C compiler selection works through `POLY_CC` and documented platform defaults.
- [x] C++ is not included in the target enum, CLI help, or CI matrix.

## Verification

- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --workspace --all-targets -- -D warnings`
- [x] `cargo test --workspace -- --test-threads=1`
- [x] `cargo check --workspace --all-targets`
- [x] Markdown convention audit
- [x] Maintained v2 Poly documentation audit
- [x] Rust examples checked and native Rust fixtures compiled.
- [x] C fixture checked, built, and executed.
- [ ] Cross-platform Rust and C matrix passes on Linux, macOS, and Windows.
- [x] Playground WASM artifact is reproducible locally under the pinned Rust toolchain.
- [x] Hosted CI rebuilds and validates a non-empty WASM artifact in the canonical Linux environment; byte identity is not required across machines.

## Repository Hygiene

- [x] Remove generated binaries, generated C files, temporary Cargo projects, and local output logs.
- [x] Review `git diff` and `git status --short` for this change; generated test outputs are removed.
- [ ] Repeat the review from a clean checkout before tagging the preview.
- [ ] Separate implementation, tests, CI, documentation, and release metadata into focused commits before merging.
- [x] Do not commit credentials, machine-specific paths, or generated build directories.
- [x] Confirm changelog and version metadata agree.

## Release Evidence

Record the following in the release PR:

- compiler version and Rust toolchain version
- operating systems and C compilers used
- exact verification commands
- test counts and relevant runtime fixtures
- known preview limitations and migration notes
- links to the support matrix and audit report

A preview tag should be created only after all mandatory checks pass from a clean checkout. This agent does not create or push tags.
