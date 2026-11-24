# Poly Preview Release Guide

**Applies to:** any `2.0.0-preview.N` release. This guide covers release
readiness, the version-bearing file map, and the mechanical release path. It
does not create or tag a release automatically; the agent workflow never tags
or pushes.

## Consistency bar

The workspace version, `poly --version`, the REPL banner, and the playground
footer must all state the same version (convention from the
`RELEASE_PR_v2.0.0-preview.2.md` release PR, archived in `docs/archive/`).
`poly --version` and the REPL banner read `CARGO_PKG_VERSION` at compile time,
so keeping the workspace version correct keeps those two correct for free.

## Version-bearing files

Every file in this repository that states a version, and the mechanism that
keeps it correct. Consult this map before a release; the release script
enforces most of it mechanically.

| File | States | Updated by | Enforced by |
|---|---|---|---|
| `compiler/Cargo.toml` | workspace version | `prepare_release.py` (step 1) | `prepare_release.py` verification |
| `compiler/Cargo.lock` | workspace member versions | `prepare_release.py` (`cargo update -w`) | lockfile diff in CI tetris job |
| `tetris/rust_output/tetris/Cargo.toml` | generated project version | `prepare_release.py` regenerates the project | consistency check + CI reproducibility guard |
| `tetris/rust_output/tetris/Cargo.lock` | resolved dependency versions | `prepare_release.py` (`cargo generate-lockfile`) | CI reproducibility guard |
| `tetris/rust_output/tetris/.poly-generated` | owning source path (relative) | `poly --project` | CI reproducibility guard; `scripts/check_generated_paths.py` |
| `tetris/rust_output/tetris/src/main.rs` | (no version; must match source) | `poly --project` / `--emit-rust` | CI reproducibility guard |
| `playground/index.html` | footer version + dialect comment | `prepare_release.py` (`sync_playground_version`) | consistency check |
| `CHANGELOG.md` | dated release section | **by hand** before running the script | `DOC_BASELINE_FILES` check |
| `README.md` ("What's New") | release highlights | **by hand** | not mechanical |
| `POLY_DOCUMENTATION_INDEX.md` | baseline statement | **by hand** | consistency check |
| `POLY_V2_PREVIEW_RELEASE_NOTES.md` | intro line + highlights | **by hand** | consistency check |

## What is mechanical vs. manual

- **Mechanical (never edit by hand):** workspace version, lockfiles, the
  tetris generated project, the playground footer and dialect comment.
  `python3 scripts/prepare_release.py <version>` rewrites all of them.
- **Manual (edit before running the script):** `CHANGELOG.md`, README
  "What's New", `POLY_DOCUMENTATION_INDEX.md`,
  `POLY_V2_PREVIEW_RELEASE_NOTES.md`. These carry prose the script cannot
  generate; the script's consistency check fails on a stale baseline so the
  manual step cannot be silently skipped.

## Related guards

- `scripts/check_generated_paths.py` — no machine-specific paths in tracked
  generated files (also wired as a pre-commit hook via `.githooks/`;
  activate with `git config core.hooksPath .githooks`).
- The tetris CI job diffs the regenerated `src/main.rs` against the committed
  file and checks `--emit-rust` agreement.
- `release.yml` refuses to publish when `poly --version` does not equal the
  tag name minus the `v` prefix, and when `CHANGELOG.md` has no section for
  the tag.
- `.github/workflows/wasm-build.yml` is the only playground-wasm build: both
  `ci.yml` (build and gate) and `release.yml` (build, gate, upload) call it.
  It installs the checksummed `wasm-opt` (binaryen 130), runs the smoke test,
  and owns the 660,000-byte `WASM_SIZE_BUDGET`, so CI and release cannot
  diverge on toolchain or number. Before the shared definition each workflow
  built its own module and only CI installed binaryen, so releases shipped the
  unoptimized build (748,320 B for `2.0.0-preview.18` against 636,157 B
  committed) and the gate measured bytes nobody downloaded.
- A post-publish `verify-release` job re-downloads every asset from the
  published release and checks: `sha256sums.txt` matches the bytes and covers
  exactly the release's files, asset names carry the tag version, the Linux
  binary reports it, the WASM header is version 1 and under budget, the macOS
  asset is 64-bit little-endian Mach-O, the Windows asset is PE, and the
  release is published (not a draft). It also records the tag's base branch as
  `target_commitish` instead of defaulting to `master`.

## Tag naming convention

**Standard: unprefixed version tags** — `2.0.0-preview.13`, not
`v2.0.0-preview.13`. The tag text equals the version string everywhere else
(`compiler/Cargo.toml`, `poly --version`, the playground footer), so nothing
needs mental stripping, and the tag matches the version the release publishes.

The early preview tags used both spellings (`v2.0.0-preview.10` and
`v2.0.0-preview.11` prefixed; `2.0.0-preview.12` onward unprefixed), so
`release.yml` triggers on both (`v*` and `2.*`). Its version guard strips a
leading `v` only when present, so either spelling still releases the version
it names. New tags must be unprefixed.

## Readiness checklist

### Contract

- [x] Review `POLY_SPEC_v2.md` and `POLY_V2_SUPPORT_MATRIX.md` together.
- [x] Confirm Rust remains the default, C remains the documented C11 subset, and asm remains the documented Linux x86-64
subset.
- [x] Confirm `#cpp` remains explicitly rejected; do not advertise a C++ backend.
- [x] Confirm all unsupported constructs have actionable diagnostics.
- [x] Confirm `extern rust fn`, `extern c fn`, and `extern asm fn` declarations are documented as opt-in interface checking.
- [x] Confirm historical v1 documents live under `docs/archive/` and are reached only through
`POLY_DOCUMENTATION_INDEX.md`, never presented as current syntax.

### Implementation

- [x] No known builtin lowers to a placeholder value or emits an “implemented” warning while doing nothing.
- [x] Opaque foreign calls remain compatible with existing v2 source.
- [x] Explicit foreign signatures validate Poly-visible arity and types.
- [x] Target filtering removes non-selected foreign blocks and extern declarations before checking/codegen.
- [x] C compiler selection works through `POLY_CC` and documented platform defaults.
- [x] C++ is not included in the target enum, CLI help, or CI matrix.

### Verification

- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --workspace --all-targets -- -D warnings`
- [x] `cargo test --workspace -- --test-threads=1`
- [x] `cargo check --workspace --all-targets`
- [x] Markdown convention audit
- [x] Maintained v2 Poly documentation audit
- [x] Rust examples checked and native Rust fixtures compiled.
- [x] C fixture checked, built, and executed.
- [x] Cross-platform Rust and C matrix passes on Linux, macOS, and Windows.
  Recorded 2026-09-17, CI run
  [35168429374](https://github.com/aj-nelson-0001/poly/actions/runs/35168429374)
  (`v2.0-dev`, commit `b7658f0`): Test matrix green on ubuntu-latest,
  macos-latest, and windows-latest; lint (full documentation audit),
  dependency audit, Tetris build + self-test, and WASM artifact jobs also
  green. Note: the tagged `v2.0.0-preview.4` run predated this fix and the
  Tetris CI job; its release build succeeded independently.
- [x] Playground WASM artifact is reproducible locally under the pinned Rust toolchain.
- [x] Hosted CI rebuilds and validates a non-empty WASM artifact in the canonical Linux environment; byte identity is not
required across machines.

### Repository hygiene

- [x] Remove generated binaries, generated C files, temporary Cargo projects, and local output logs.
- [x] Review `git diff` and `git status --short` for this change; generated test outputs are removed.
- [x] Repeat the review from a clean checkout before tagging the preview.
  Recorded 2026-09-17 for `v2.0.0-preview.5` (commit `739d79c`): fresh clone
  of the tag, release build from scratch (`poly 2.0.0-preview.5`),
  workspace tests 452/0, markdown 52 files, doc audit 573 blocks / 0
  failures, tetris `src/main.rs` reproduced byte-for-byte via
  `--emit-rust`, tetris release build + `--test` self-test passed.
- [x] Separate implementation, tests, CI, documentation, and release metadata into focused commits before merging.
  (Verified across the `v2.0-dev` series through `v2.0.0-preview.6`: every
  commit is type-prefixed (`feat:`, `fix:`, `test:`, `ci:`, `docs:`), release
  metadata is isolated in `chore(release):` commits, and progress notes land
  as standalone `docs:` commits. Feature-scoped docs and tests accompany the
  `feat:` commit that introduces them by convention; splitting already-pushed
  commits further would require rewriting published history, so the series
  was reviewed as-is and found focused.)
- [x] Do not commit credentials, machine-specific paths, or generated build directories.
- [x] Confirm changelog and version metadata agree.

## Release evidence

Record the following in the release PR:

- compiler version and Rust toolchain version
- operating systems and C compilers used
- exact verification commands
- test counts and relevant runtime fixtures
- known preview limitations and migration notes
- links to the support matrix and the archived audit report

A preview tag should be created only after all mandatory checks pass from a
clean checkout.

## Release procedure

The mechanical steps are automated by `scripts/prepare_release.py`; it never
commits, tags, or pushes.

~~~sh
# 1. Update the docs that must describe the new version by hand:
#    CHANGELOG.md (date the [Unreleased] section), README.md
#    ("What's New"), POLY_DOCUMENTATION_INDEX.md (baseline),
#    POLY_V2_PREVIEW_RELEASE_NOTES.md (highlights).

# 2. Bump, regenerate, and verify (workspace version, tetris project,
#    lockfiles, tests, fmt, clippy, markdown, doc audit):
python3 scripts/prepare_release.py 2.0.0-preview.N

# 2b. Lint tracked generated files for machine-specific paths (home
#     directories, runner paths) that would break byte-reproducibility:
python3 scripts/check_generated_paths.py

# 3. Review the diff, then commit and tag:
git add -A
git commit -m "chore(release): prepare 2.0.0-preview.N"
git push origin v2.0-dev
git tag -a 2.0.0-preview.N -m "Poly 2.0.0-preview.N"
git push origin 2.0.0-preview.N

# 4. Confirm the results:
#    - .github/workflows/release.yml builds and publishes the binary; tags
#      containing "preview" are marked as prereleases automatically
#      (verified end to end with a throwaway tag on 2026-09-17).
#    - The verify-release job is green: it re-downloads the published assets
#      and checksum-verifies them, matches the Linux version to the tag, and
#      checks the wasm header/size and platform format magic.
#    - CI on the branch is green for the release commit.
~~~

When the tag is pushed, `release.yml` builds, verifies the binary version
against the tag, renames the asset to `poly-linux-amd64-<version>`, generates
`sha256sums.txt` over the uploaded assets, and publishes a prerelease whose
body is the committed `POLY_V2_PREVIEW_RELEASE_NOTES.md`; a follow-up
`verify-release` job then re-downloads and checks what was published, so there
are no manual post-release steps. If that job fails, the release is live but
unverified: delete it with `gh release delete <tag>` and re-cut the tag after
fixing the cause — do not quietly leave a red verification run behind.
(Verified end-to-end by re-publishing 2.0.0-preview.13 through the workflow.)
Because the tagged commit's notes become the release
body, its What's New section must already describe the release being cut, and
the verification-evidence numbers in it should be re-checked at tag time.
