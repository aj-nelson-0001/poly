# Poly 2.0.0-preview.1 Release Notes

**Status:** Preview release candidate documentation

Poly 2.0.0-preview.1 establishes the target-aware Rust/C compiler model. Rust remains the default and most complete backend. C is an intentionally narrow C11 orchestration backend, while C++ syntax is reserved and explicitly rejected.

## Highlights

- Added `--target rust` and `--target c` target selection across checking, emission, project generation, and native builds.
- Added top-level `#rust` and `#c` foreign blocks. Only the block matching the selected target is emitted; foreign bodies remain opaque to Poly and are validated by the native compiler.
- Added optional top-level `extern rust fn ...` and `extern c fn ...` declarations for Poly-side arity, argument-type, and return-type checks.
- Added the C11 backend with scalar declarations, control flow, simple functions, plain structs, diagnostics, and calls into `#c` helpers.
- Added native C syntax checks, executable builds, C project generation, and end-to-end runtime regression coverage.
- Added `POLY_CC` configuration with platform compiler fallbacks and CI coverage for Linux, macOS, and Windows.
- Removed the unfinished `process_config` compatibility path so unknown calls fail instead of silently lowering to `()`.
- Reconciled maintained documentation around a frozen Rust/C support matrix and migration guide.

## Target Contract

| Target | Use | Verification |
|---|---|---|
| Rust | Default target and complete v2 baseline | `rustc` or generated Cargo project |
| C | C11 orchestration subset | `POLY_CC` C compiler |
| C++ | Reserved only | Explicitly rejected; no backend exists |

For the complete feature boundary, see [POLY_V2_SUPPORT_MATRIX.md](POLY_V2_SUPPORT_MATRIX.md). For foreign block syntax and C type mappings, see [POLY_C_BLOCKS.md](POLY_C_BLOCKS.md).

## Compatibility Notes

- v2 uses `:=` for declarations and assignment and `=` for equality. Legacy `==` is rejected with a migration diagnostic.
- Append output uses `put value to "file" -append`.
- `put` always writes a newline.
- Foreign blocks and `extern` declarations must be top-level.
- Opaque foreign calls remain permissive for compatibility. Add an explicit `extern` declaration when Poly-side interface diagnostics are useful.
- C does not provide the Rust runtime surface. Input, file redirects, collections, tuples, pattern matching, closures, async features, SQLite, and network helpers remain Rust-only or must be implemented behind a `#c` helper.

## Verification Evidence

The committed local verification completed successfully:

~~~bash
cargo test --workspace -- --test-threads=1
cargo check --workspace --all-targets
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
python3 scripts/check_markdown.py
python3 scripts/check_poly_examples.py README.md POLY_SPEC_v2.md POLY_ROADMAP_v2.md POLY_C_BLOCKS.md POLY_V2_AUDIT.md POLY_MIGRATION_GUIDE_v2.md POLY_V2_SUPPORT_MATRIX.md POLY_PREVIEW_RELEASE_CHECKLIST.md
~~~

Additional target checks passed for all 22 Rust examples, the mixed-target fixture, the C fixture, native C execution, explicit C foreign signatures, and unsupported C diagnostics. The C check was also run with `POLY_CC=cc`.

The committed CI workflow runs the same Rust/C test path on `ubuntu-latest`, `macos-latest`, and `windows-latest`. Hosted-runner results are release evidence that must be recorded after the workflow executes; they are not claimed by this local verification.

## Known Preview Limitations

- The C backend is not a general Poly-to-C translator. Use `#c` helpers or the Rust target for unsupported constructs.
- Foreign function ABI, pointer, ownership, lifetime, and target-specific type correctness remain the responsibility of the native compiler.
- `#cpp` has no implementation.
- The playground WASM artifact is reproducible locally under the repository-pinned Rust toolchain. Hosted CI builds and validates a non-empty artifact in its canonical Linux environment; byte identity is not expected across different build environments.
- Hosted cross-platform CI evidence is required before tagging or publishing the preview. A final clean-checkout review should also be completed before tagging.

## Release References

- [Poly v2 documentation index](POLY_DOCUMENTATION_INDEX.md)
- [Poly v2 specification](POLY_SPEC_v2.md)
- [Poly v2 support matrix](POLY_V2_SUPPORT_MATRIX.md)
- [C11 backend guide](POLY_C_BLOCKS.md)
- [Preview release checklist](POLY_PREVIEW_RELEASE_CHECKLIST.md)
- [v2 audit](POLY_V2_AUDIT.md)
