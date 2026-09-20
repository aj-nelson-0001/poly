# Poly Version-Bearing Files Map

**Status:** Maintainer reference for the current preview series

Every file in this repository that states a version, and the mechanism that
keeps it correct. Consult this map before a release; the release script
enforces most of it mechanically.

## The consistency bar

The workspace version, `poly --version`, the REPL banner, and the playground
footer must all state the same version (convention from the
`RELEASE_PR_v2.0.0-preview.2.md` release PR). `poly --version` and the REPL
banner read `CARGO_PKG_VERSION` at compile time, so keeping the workspace
version correct keeps those two correct for free.

## File-by-file map

| File | States | Updated by | Enforced by |
|---|---|---|---|
| `compiler/Cargo.toml` | workspace version | `prepare_release.py` (step 1) | `prepare_release.py` verification |
| `compiler/Cargo.lock` | workspace member versions | `prepare_release.py` (`cargo update -w`) | lockfile diff in CI tetris job |
| `tetris/rust_output/tetris/Cargo.toml` | generated project version | `prepare_release.py` regenerates the project | consistency check + CI reproducibility guard |
| `tetris/rust_output/tetris/Cargo.lock` | resolved dependency versions | `prepare_release.py` (`cargo generate-lockfile`) | CI reproducibility guard |
| `tetris/rust_output/tetris/.poly-generated` | owning source path (relative) | `poly --project` | CI reproducibility guard; `scripts/check_generated_paths.py` |
| `tetris/rust_output/tetris/src/main.rs` | (no version; must match source) | `poly --project` / `--emit-rust` | CI reproducibility guard |
| `playground/index.html` | footer version + dialect comment | `prepare_release.py` (`sync_playground_version`) | consistency check |
| `CHANGELOG.md` | dated release section | **by hand** before running the script | `DOC_BASELINE_FILES`-style check notes below |
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
- The tetris CI job diffs regenerated output against the committed project.
- `release.yml` refuses to publish when `poly --version` does not equal the
  tag name minus the `v` prefix.

## Tag naming convention

**Standard: unprefixed version tags** — `2.0.0-preview.13`, not
`v2.0.0-preview.13`. The tag text equals the version string everywhere
else (`compiler/Cargo.toml`, `poly --version`, the playground footer), so
nothing needs mental stripping, and the tag matches the version the
release publishes.

The early preview tags used both spellings (`v2.0.0-preview.10` and
`v2.0.0-preview.11` prefixed; `2.0.0-preview.12` onward unprefixed), so
`release.yml` triggers on both (`v*` and `2.*`). Its version guard strips
a leading `v` only when present, so either spelling still releases the
version it names. New tags must be unprefixed.

Publishing flow for a new release:

1. Edit the manual docs (CHANGELOG section, README "What's New", the two
   doc baselines) **before** running `prepare_release.py`.
2. Run `python3 scripts/prepare_release.py <version>`; review the diff.
3. Commit, then `git tag -a <version> -m "Poly <version>"` (unprefixed).
4. Push the branch and tag; `release.yml` builds, verifies the binary
   version against the tag, and publishes a prerelease with the binary
   asset. Replace its generated notes with the content of
   `POLY_V2_PREVIEW_RELEASE_NOTES.md` (plus a `sha256sums.txt` asset for
   the binary) to match the preview.12/.13 release shape.
