# Poly 2.0 Preview Release Notes

**Status:** Preview release candidate documentation

Poly 2.0.0-preview.17 continues the target-aware compiler model established by the earlier previews. Rust remains the default and most complete backend. C is an intentionally narrow C11 orchestration backend, a Linux x86-64 assembly target is available through `--target asm`, a JavaScript target through `--target js`, and C++ syntax is reserved and explicitly rejected.

## What's New in 2.0.0-preview.17

- Expression and type nesting is capped at 32 levels, so parenthesized or
  type-heavy hostile input cannot exhaust the parser stack either. The cap
  is sized from measurement (~45 KiB per debug precedence-chain level, so
  the worst case peaks near 1.6 MiB inside the 2 MiB default test-thread
  budget), and a chain of `if`s still reports its own, more specific
  depth limit when both budgets run out together.
- Differential coverage deepened for the depth limits: a new
  `tests/diff_deep_nesting` fixture drives deep parens, nested ifs, and
  nested whiles through all four backends, and the fuzz generator wraps
  expressions in grouping parens so random seeds parse near the cap.
- The Linux CI stack-floor guard recentred to 1952 KiB after the parser
  changes moved the healthy floor, keeping the frame-per-level regression
  detector precise.

## What's New in 2.0.0-preview.16

- Error handling hardened workspace-wide: `unwrap()`/`expect()` are banned
  with clippy gates that fail CI on regressions, reachable compiler panics
  now return diagnostics or errors, and generated code reports runtime
  failures with context instead of panicking.
- Parser robustness: program-scope-only statements (`extern fn` and
  foreign `#lang` blocks) are rejected in every nested context including
  match arms, macro and `pub` continuations stay level-faithful, and
  statement nesting is capped at 128 levels so parser stack use is
  provably bounded for any input. CI pins the error-handling suite's
  stack floor to keep deep-nesting handling regression-free.

## What's New in 2.0.0-preview.15

- **A tutorial that is true and teaches:** POLY_TUTORIAL.md is current
  documentation — every complete example verified against the compiler,
  inline comments throughout, and new parts covering structs and methods,
  match expressions and enum payloads, and async.
- **Named-field enum payloads match again:** variants declared with named
  fields previously failed every match with rustc E0164 on the Rust
  target; struct-variant patterns now emit correctly (rust and asm).
- **Docs and backends stay pinned together:** `doc_claim_guards` now
  covers `#`/`//`/`/* */` comment styles on all four backends, the
  rust-only async surface with per-target rejection wording, and the
  payload-matching fix.

## What's New in 2.0.0-preview.14

- **Multi-platform release binaries:** Linux (amd64), macOS (arm64), and
  Windows (amd64) builds ship as versioned assets with a `sha256sums.txt`
  covering all of them. The release body is the committed release-notes
  document, and the workflow refuses to publish a tag whose version is
  absent from the changelog or mismatches the built binary.

## What's New in 2.0.0-preview.13

- **Backend-claim audit shipped:** the maintained docs now state only
  claims verified against the compiler — C supports plain stdin `get`, JS
  supports structs and tuples, interpolation works everywhere, and the
  per-target contract is pinned end-to-end by the new
  `doc_claim_guards` test suite.
- **Fixed:** a prompt-less `get` on the C target emitted a zero-argument
  call to a one-argument runtime helper, so the generated C failed to
  compile.

## What's New in 2.0.0-preview.12

Strings work on every target: C materializes value-position concatenation
and scalar `to_string()` through emitted runtime helpers (with correct
`const char *` inference for chains containing `to_string()`), the asm
target materializes concat through a static bump arena with
`_str_alloc`/`_strlen`/`_poly_concat` runtime helpers, and `.len()` lowers
to `strlen` on C and asm. A compiler audit fixed optimizer edge cases
(over-wide shifts, division by zero, unguarded identity folds, an inliner
that mis-substituted non-arithmetic bodies), a checker bug that
double-collected module functions, and a generator crash on declarations
nested in function bodies. Generated code is faster: the C backend builds
with `-O2`, generated Cargo projects pin `opt-level = 3` in release, and
Rust string-appends in loops lower to in-place `push_str` (~500x on a
100k-append loop). The differential suite grew to four programs,
including `tests/diff_strings.poly`, which pins concat semantics across
all four targets.

## What's New in 2.0.0-preview.10

Backend-robustness release: a differential sweep of the same program through
all four targets found and fixed five backend defects — C and JS now support
match expressions in value position, the asm runtime prints negative
integers correctly, asm expression temporaries no longer share stack slots,
asm passes struct-returning call results as struct arguments correctly, and
C infers struct types from call-result initializers. A new differential
backend suite (`scripts/check_backends.py` + `tests/diff_*.poly`) runs in
every CI push/PR and nightly, failing when any target's output diverges.

## What's New in 2.0.0-preview.9

Documentation release: the spec now defines the `.poly-generated`
generated-project marker (record format, relative-path portability
guarantee, and the refresh-safety rule it enables) and documents match
expressions as variable initializers; the cheat sheet summarizes the
marker.

## What's New in 2.0.0-preview.8

Generated projects now record their source portably: the `.poly-generated`
marker stores the source path relative to the generated project instead of the
generating machine's absolute path, so committed output is byte-identical
across machines. Refresh-mode identity checks resolve the record against the
output directory and remain compatible with older absolute markers. The tetris
example builds warning-free (match-expression gravity table,
non-deprecated `set_target_fps`), and its CI job now diffs the regenerated
project against the committed one, failing on any drift from `tetris.poly`.

The asm backend now supports program-scope structs and enums end to end:
definitions resolve inside any function body, struct parameters pass by
reference, struct-returning functions copy through caller-allocated space,
and inferred declarations from struct literals or call results get
struct-sized storage. `dep` declarations on non-Rust targets now produce an
explicit explanatory warning instead of being silently dropped.

## What's New in 2.0.0-preview.6

- **`dep name = "version"` declarations** for external crate dependencies:
  program-scope declarations that `--project` emits into the generated
  `Cargo.toml` and that `--check` resolves through Cargo, so programs using
  external crates validate standalone. The Tetris example now declares its
  `minifb`/`alsa` dependencies in source.
- The release workflow refuses to publish when the built binary's version
  does not match the tag name, and CI no longer duplicates release
  publishing (`release.yml` is the single publisher).

## What's New in 2.0.0-preview.5

- Re-cut of `2.0.0-preview.4` from a commit whose CI run is fully green on
  Linux, macOS, and Windows (run 35168429374): the compiler code is
  unchanged. The original `preview.4` tag predated the tetris CI job and two
  CI fixes, so its own CI run was red while the release build succeeded.
- Adds the README CI badge for the `v2.0-dev` branch.

## What's New in 2.0.0-preview.4

- **Compiler correctness fixes found by real code:** nested `while` loops now generate a real loop in every position (they previously ran exactly once inside `if` bodies, `match` arms, and loop bodies); range loops with expression start bounds (`loop i BUF..TOTAL - 1`, `loop i TOTAL - 4..TOTAL - 1`) parse correctly; the var-less `loop 0..10` form is a real counted loop instead of a silently infinite one; and a `const` declared inside a function no longer panics the IR generator — it lowers to an immutable local across the Rust, C, JS, and asm backends.
- **Documentation complete and CI-verified:** every current-guide example is an explicit `fn main` program that passes `poly --check` — 579 blocks audited, 0 unmarked failures, verified in CI over all 55 markdown files via `check_poly_examples.py --poly-bin`.
- **CI runs on `v2.0-dev`** and now also builds and self-tests the Tetris example on Linux, including a regeneration-drift guard over the tracked generated project.
- Documented language rules: range-loop grammar, reserved words `spawn`/`step`, take-and-return `impl` method move semantics, and the strict same-type comparison rule (cast with `as`).

For the 2.0.0-preview.3 and earlier highlights, see [CHANGELOG.md](CHANGELOG.md).

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
| JS | ES2020 orchestration subset, no runtime dependencies | `node --check` plus execution (Linux CI) |
| C++ | Reserved only | Explicitly rejected; no backend exists |

For the complete feature boundary, see [POLY_V2_SUPPORT_MATRIX.md](POLY_V2_SUPPORT_MATRIX.md). For foreign block syntax and C type mappings, see [POLY_C_BLOCKS.md](POLY_C_BLOCKS.md); for the JS backend, see [POLY_JS_BLOCKS.md](POLY_JS_BLOCKS.md).

## Compatibility Notes

- v2 uses `:=` for declarations and assignment and `=` for equality. Legacy `==` is rejected with a migration diagnostic.
- Append output uses `put value to "file" -append`.
- `put` always writes a newline.
- Foreign blocks and `extern` declarations must be top-level.
- Opaque foreign calls remain permissive for compatibility. Add an explicit `extern` declaration when Poly-side interface diagnostics are useful, or run `poly --strict` / the CI checks, which enforce declarations.
- C does not provide the full Rust runtime surface. File redirects, file input, typed input flags, vectors, maps/sets, async features, SQLite, and network helpers remain Rust-only or must be implemented behind a foreign helper. Plain `get`, tuples, simple `match` patterns, struct literals, enum variants, and non-capturing closures have C support as of `2.0.0-preview.2`.

## Verification Evidence

The committed local verification completed successfully:

~~~bash
cargo test --workspace -- --test-threads=1
cargo check --workspace --all-targets
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
python3 scripts/check_markdown.py
python3 scripts/check_poly_examples.py --poly-bin compiler/target/release/poly
~~~

The full-repository documentation audit covers 579 example blocks with 0
unmarked failures; the Tetris example regenerates, builds, and passes its
headless self-test with the release compiler.

Additional target checks passed for the Rust examples, the mixed-target fixture, the C fixture, native C execution, explicit C foreign signatures, and unsupported C diagnostics. The asm and JS fixtures were checked, compiled/executed, and their outputs verified on Linux.

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
