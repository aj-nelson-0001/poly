# Poly 2.0 Preview Release Notes

**Status:** Preview release candidate documentation

Poly 2.0.0-preview.2 extends the target-aware compiler model established by 2.0.0-preview.1. Rust remains the default and most complete backend. C is an intentionally narrow C11 orchestration backend, a Linux x86-64 assembly target is available through `--target asm`, and C++ syntax is reserved and explicitly rejected.

## What's New in 2.0.0-preview.2

- Added `--target asm`: freestanding Linux x86-64 assembly with Linux syscalls, a `_start` entry point that calls `fn main`, `#asm` foreign blocks, `extern asm fn ...` interface declarations, static and growable descriptor-backed vectors (`push`/`pop`/`len`), pointer dereference, and `for x in <string>` iteration.
- Widened the C backend: tuples (anonymous structs with `_N` fields), `match` statements (literal, wildcard, range, and identifier arms), struct literals (C99 compound literals), enum declarations with qualified enumerators in expressions and match patterns, plain stdin `get` with optional prompt, and non-capturing closures.
- Added language-wide `pop()` on vectors: it type-checks as the element type and lowers to `pop().unwrap_or_default()` on the Rust target.
- Added span-precise source maps (`poly --source-map` reports exact statement-level line mappings) and UTF-16 LSP document-symbol ranges.
- Fixed `--target c|asm -o <path>` output-path handling, the asm frame-size bug that clobbered locals below `%rsp`, `#asm` section restoration, the asm `_start`-to-`fn main` call, the duplicate C `main` definition for `fn main` programs, and `--help` inaccuracies.
- CI now smoke-tests the rebuilt playground wasm artifact and verifies asm-target fixture output end-to-end on Linux.

For the 2.0.0-preview.1 highlights, see [CHANGELOG.md](CHANGELOG.md).

## Target Contract

| Target | Use | Verification |
|---|---|---|
| Rust | Default target and complete v2 baseline | `rustc` or generated Cargo project |
| C | C11 orchestration subset | `POLY_CC` C compiler |
| Asm | Linux x86-64 assembly subset | GNU assembler and linker (Linux CI) |
| C++ | Reserved only | Explicitly rejected; no backend exists |

For the complete feature boundary, see [POLY_V2_SUPPORT_MATRIX.md](POLY_V2_SUPPORT_MATRIX.md). For foreign block syntax and C type mappings, see [POLY_C_BLOCKS.md](POLY_C_BLOCKS.md).

## Compatibility Notes

- v2 uses `:=` for declarations and assignment and `=` for equality. Legacy `==` is rejected with a migration diagnostic.
- Append output uses `put value to "file" -append`.
- `put` always writes a newline.
- Foreign blocks and `extern` declarations must be top-level.
- Opaque foreign calls remain permissive for compatibility. Add an explicit `extern` declaration when Poly-side interface diagnostics are useful.
- C does not provide the full Rust runtime surface. File redirects, file input, typed input flags, vectors, maps/sets, async features, SQLite, and network helpers remain Rust-only or must be implemented behind a foreign helper. Plain `get`, tuples, simple `match` patterns, struct literals, enum variants, and non-capturing closures have C support as of `2.0.0-preview.2`.

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
