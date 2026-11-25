# Poly 2.0 Preview Release Notes

**Status:** Preview release candidate documentation

Poly 2.0.0-preview.21 is a release-tooling release: the language surface is unchanged from preview.20, the size release
that dropped the assembly backend from the browser module. Rust remains the default and most complete backend. C is an
intentionally narrow C11 orchestration backend, a Linux x86-64 assembly target is available through `--target asm` from the CLI, a JavaScript
target through `--target js`, a Goose target through `--target goose`, and C++ syntax is reserved and explicitly rejected.

## What's New in 2.0.0-preview.21

- **A release cannot tag a commit before CI passes.** `scripts/release.py`
  runs `prepare_release.py`, commits only release metadata (it aborts and
  lists the paths if anything outside the release surface changed), pushes the
  branch, waits for the `ci.yml` push run on that exact commit, and only then
  creates and pushes the tag. A red run stops it with instructions and leaves
  no tag behind — the failure mode that forced preview.20's first tag to be
  deleted and re-cut. `--resume` continues after a red run, `--skip-tests`
  forwards to the prep, and `--dry-run` prints the plan without writing.
- **The summary reports wasm headroom.** The driver prints the published
  playground module against the budget it reads from `wasm-build.yml`, so a
  release that spends the headroom says so when it ships rather than at the
  next failed size gate.
- **The asm guard is proven where it runs.** A `wasm-asm-guard` CI job builds
  the module with `poly-transpiler/asm` and fails unless the smoke test
  rejects it for both reasons — the target-list parity message and the notes
  message — so a smoke test broken for an unrelated cause cannot satisfy it.
  The job runs in `$RUNNER_TEMP` and never touches the committed module.
- **The long-lived branches are locked against rewrites.** Rulesets
  `protect-v2.0-dev` and `protect-master` block deletion and non-fast-forward
  pushes, with no bypass actors; `master` additionally requires a pull request
  with no approvals, so it can no longer be fast-forwarded by a raw push. The
  guide records both rulesets and the read-back command.

## What's New in 2.0.0-preview.20

- **The browser module drops the assembly backend.** `poly.wasm` ships at
  548,899 bytes, down from 635,084 in preview.19 (-86,185 bytes, about 14%),
  with the 660,000-byte budget left unchanged and 111,101 bytes of headroom.
  The asm backend now sits behind a default-on `asm` cargo feature, the same
  shape the `goose` target already used: the CLI, the LSP, and the test suite
  still compile it, while `poly-wasm` builds `poly-transpiler` with
  `default-features = false`. Asking the playground for `--target asm` now
  reports that the target is not part of that build rather than carrying a
  backend the page cannot use.
- **The published module is the module CI measured.** The wasm workflow
  rebuilds the playground module, compares it byte for byte against the
  committed `playground/poly.wasm`, and then gates its size, so a hand-edited
  or stale artifact fails CI instead of shipping quietly.
- **The size pass runs to a fixed point.** The build applies `wasm-opt -Oz`
  repeatedly (`--converge`) until a pass changes nothing, which is where the
  last few hundred bytes of the smaller module come from.
- **Release verification runs on demand.** `verify-release.yml` is now
  dispatchable as well as reusable, so any published tag can be re-checked
  without cutting a new release. The first dispatched run re-verified
  `2.0.0-preview.19` (run
  [45515305170](https://github.com/aj-nelson-0001/poly/actions/runs/45515305170)):
  all eight checks passed, and the published module measured 635,084 bytes
  against the 660,000 budget.
- **The default branch is `v2.0-dev`.** The repository default moved off
  `master`, which had been a strict ancestor of `v2.0-dev` since October, so
  clones, links, and dispatches all land on the code that ships.

## What's New in 2.0.0-preview.19

- **The playground module is optimized again.** preview.16, preview.17, and
  preview.18 published the unoptimized wasm build — 705,154, 707,584, and
  748,320 bytes — because only the CI workflow installed `wasm-opt`; the
  release workflow built its own copy without binaryen, so the 660,000-byte
  size gate measured a build nobody downloaded. Both workflows now call one
  reusable `.github/workflows/wasm-build.yml` that installs pinned binaryen
  130 (checksummed), runs the playground smoke test, and owns the budget. This
  release ships 635,076 bytes, about 15% smaller than preview.18.
- **Releases record the right base branch.** `create-release` derives the
  tag's branch (`fetch-depth: 0`) and passes it as `target_commitish`; before,
  every release pointed at the stale `master` branch.
- **Published assets are verified.** A new `verify-release` job re-downloads
  every asset and checks the checksum manifest against the bytes, asset
  naming, the Linux binary's reported version, the WASM header and size
  budget, and the macOS/Windows binary formats, so a broken release fails the
  workflow instead of staying published.

## What's New in 2.0.0-preview.18

- **The Goose target ships.** `--target goose` emits Goose orchestration
  source through `poly-goose-codegen`: `#goose ... #endgoose` blocks are
  copied verbatim at file scope, `extern goose fn ...` declarations are
  checked by Poly but never emitted, and `--check` runs `goose check` (with
  `POLY_GOOSE` overriding the toolchain binary). The supported surface
  follows the JS backend plus vector literals and mutators; anything else —
  enums, tuples, closures, async, generics, methods, guards, `get` input,
  file redirects, `error`/`warn`/`info`, and more — is rejected with a
  diagnostic naming the limitation and the `#goose` escape hatch.
- **Simpler mutation and new operators.** `add x, n` / `sub x, n` and
  `inc x` / `dec x` lower to `x := x ± n` and `x := x ± 1` over plain
  variables, `put ... to "file" -prepend` writes in front of the existing
  contents, and `^` is exponentiation on every target
  (right-associative, tighter than `*`, with compile-time folding for
  constant exponents in `0..=63`).
- **Security hardening.** `dep` version strings are validated against an
  allowlist, closing a TOML-injection path that could execute a build
  script during `--check`; generated C, asm, and JS escape NUL and every
  other control character; scratch files and temporary Cargo projects use
  private, unpredictable directories with `0700` permissions on Unix.
- **Documentation consolidated.** Point-in-time guides, audit reports, blog
  drafts, and old release material moved to `docs/archive/` (indexed by
  `docs/archive/README.md`). `POLY_QUICK_REFERENCE.md` now merges the
  former cheatsheet and quick-reference card, and `POLY_RELEASE_GUIDE.md`
  merges the preview checklist and version-bearing file map. The Tetris CI
  guard now diffs the regenerated `src/main.rs` against the committed file,
  so a stale checked-in project fails CI instead of passing silently.

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

- **Compiler correctness fixes found by real code:** nested `while` loops now generate a real loop in every position (they
previously ran exactly once inside `if` bodies, `match` arms, and loop bodies); range loops with expression start bounds (`loop
i BUF..TOTAL - 1`, `loop i TOTAL - 4..TOTAL - 1`) parse correctly; the var-less `loop 0..10` form is a real counted loop
instead
of a silently infinite one; and a `const` declared inside a function no longer panics the IR generator — it lowers to an
immutable local across the Rust, C, JS, and asm backends.
- **Documentation complete and CI-verified:** every current-guide example is an explicit `fn main` program that passes `poly
--check` — 579 blocks audited, 0 unmarked failures, verified in CI over all 55 markdown files via `check_poly_examples.py
--poly-bin`.
- **CI runs on `v2.0-dev`** and now also builds and self-tests the Tetris example on Linux, including a regeneration-drift
guard
over the tracked generated project.
- Documented language rules: range-loop grammar, reserved words `spawn`/`step`, take-and-return `impl` method move semantics,
and the strict same-type comparison rule (cast with `as`).

For the 2.0.0-preview.3 and earlier highlights, see [CHANGELOG.md](CHANGELOG.md).

## What's New in 2.0.0-preview.2

- Added `--target asm`: freestanding Linux x86-64 assembly with Linux syscalls, a `_start` entry point that calls `fn main`,
`#asm` foreign blocks, `extern asm fn ...` interface declarations, static and growable descriptor-backed vectors
(`push`/`pop`/`len`), pointer dereference, and `for x in <string>` iteration.
- Widened the C backend: tuples (anonymous structs with `_N` fields), `match` statements (literal, wildcard, range, and
identifier arms), struct literals (C99 compound literals), enum declarations with qualified enumerators in expressions and
match
patterns, plain stdin `get` with optional prompt, and non-capturing closures.
- Added language-wide `pop()` on vectors: it type-checks as the element type and lowers to `pop().unwrap_or_default()` on the
Rust target.
- Added span-precise source maps (`poly --source-map` reports exact statement-level line mappings) and UTF-16 LSP
document-symbol ranges.
- Fixed `--target c|asm -o <path>` output-path handling, the asm frame-size bug that clobbered locals below `%rsp`, `#asm`
section restoration, the asm `_start`-to-`fn main` call, the duplicate C `main` definition for `fn main` programs, and `--help`
inaccuracies.
- CI now smoke-tests the rebuilt playground wasm artifact and verifies asm-target fixture output end-to-end on Linux.

For the 2.0.0-preview.1 highlights, see [CHANGELOG.md](CHANGELOG.md).

## Target Contract

| Target | Use | Verification |
|---|---|---|
| Rust | Default target and complete v2 baseline | `rustc` or generated Cargo project |
| C | C11 orchestration subset | `POLY_CC` C compiler |
| Asm | Linux x86-64 assembly subset (CLI only; absent from the browser module) | GNU assembler and linker (Linux CI) |
| JS | ES2020 orchestration subset, no runtime dependencies | `node --check` plus execution (Linux CI) |
| Goose | Goose orchestration subset | `goose check` plus execution (when the `goose` toolchain is available) |
| C++ | Reserved only | Explicitly rejected; no backend exists |

For the complete feature boundary, see [POLY_V2_SUPPORT_MATRIX.md](POLY_V2_SUPPORT_MATRIX.md). For foreign block syntax and C
type mappings, see [POLY_C_BLOCKS.md](POLY_C_BLOCKS.md); for the JS backend, see [POLY_JS_BLOCKS.md](POLY_JS_BLOCKS.md).

## Compatibility Notes

- v2 uses `:=` for declarations and assignment and `=` for equality. Legacy `==` is rejected with a migration diagnostic.
- Append output uses `put value to "file" -append`.
- `put` always writes a newline.
- Foreign blocks and `extern` declarations must be top-level.
- `--target asm` is a CLI target. The playground module is built without the asm backend and reports that the target is not part of that build.
- Opaque foreign calls remain permissive for compatibility. Add an explicit `extern` declaration when Poly-side interface
diagnostics are useful, or run `poly --strict` / the CI checks, which enforce declarations.
- C does not provide the full Rust runtime surface. File redirects, file input, typed input flags, vectors, maps/sets, async
features, SQLite, and network helpers remain Rust-only or must be implemented behind a foreign helper. Plain `get`, tuples,
simple `match` patterns, struct literals, enum variants, and non-capturing closures have C support as of `2.0.0-preview.2`.

## Verification Evidence

The checks for this release ran in hosted CI and were reproduced locally:

~~~bash
cargo test -p poly-transpiler -p poly-wasm
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
node scripts/playground_wasm_smoke.mjs
python3 scripts/check_markdown.py
~~~The tag is cut only from a `v2.0-dev` commit whose push-triggered run is green:
the workspace test suite on Linux, macOS, and Windows, the dependency audit, the
lint and documentation checks, the Tetris example's regeneration and
headless self-test, the asm-parity guard, and the wasm build with its drift and
size gates. That ordering is now enforced by `scripts/release.py`, which waits
for the CI run on the exact release commit before it creates the tag. The
release workflow then rebuilds the module through that same reusable job,
publishes those bytes, and runs `verify-release` to re-download every asset and
check the checksum manifest, asset naming, the reported compiler version, the
WASM header and budget, and the macOS/Windows binaries. The wasm job logs
`poly.wasm: 548185 bytes (budget: 660000)`, and building the tagged sources
repeatedly locally produced the same 548,185-byte module and SHA-256, so the
published artifact is reproducible from the repository.

Additional target checks passed for the Rust examples, the mixed-target fixture, the C fixture, native C execution, explicit
C \
  \
foreign signatures, and unsupported C diagnostics. The asm and JS fixtures were checked, compiled/executed, and their
outputs  \
verified on Linux.

Hosted-runner results for the shipping commit and for a re-verified published
tag are recorded above; they are the release evidence for this preview.

## Known Preview Limitations

- The C backend is not a general Poly-to-C translator. Use `#c` helpers or the Rust target for unsupported constructs.
- Foreign function ABI, pointer, ownership, lifetime, and target-specific type correctness remain the responsibility of the  \
native compiler.
- `#cpp` has no implementation.
- The playground WASM artifact is byte-identical between the repository-pinned local toolchain and hosted CI: the workflow  \
rebuilds it, compares it with the committed file, and gates the size of the same bytes users download. A build outside the  \
pinned toolchain is not covered by that comparison.
- Hosted cross-platform CI evidence is required before tagging or publishing the preview. A final clean-checkout review
should \
  \
also be completed before tagging.

## Release References

- [Poly v2 documentation index](POLY_DOCUMENTATION_INDEX.md)
- [Poly v2 specification](POLY_SPEC_v2.md)
- [Poly v2 support matrix](POLY_V2_SUPPORT_MATRIX.md)
- [C11 backend guide](POLY_C_BLOCKS.md)
- [Preview release checklist](POLY_RELEASE_GUIDE.md)
- [v2 audit](docs/archive/POLY_V2_AUDIT.md)
