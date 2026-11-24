# Poly Documentation Index

Poly 2.0.0-preview.18 is the latest released implementation baseline; work landed after that tag is listed under
`[Unreleased]` in [CHANGELOG.md](CHANGELOG.md). The files below are the maintained sources of truth for syntax,
targets, tooling, and migration.

## Current Documentation

| Document | Purpose |
|---|---|
| [README.md](README.md) | Project overview, quick start, features, CLI, and links |
| [POLY_SPEC_v2.md](POLY_SPEC_v2.md) | Current v2 syntax and language contract |
| [POLY_GRAMMAR.md](POLY_GRAMMAR.md) | Compact v2 grammar accepted by the parser |
| [POLY_MIGRATION_GUIDE_v2.md](POLY_MIGRATION_GUIDE_v2.md) | Migration from v1.8 to the v2 preview and target selection |
| [POLY_C_BLOCKS.md](POLY_C_BLOCKS.md) | C11 backend scope and `#c` block behavior |
| [POLY_JS_BLOCKS.md](POLY_JS_BLOCKS.md) | JavaScript backend scope and `#js` block behavior |
| [POLY_GOOSE_BLOCKS.md](POLY_GOOSE_BLOCKS.md) | Goose backend scope and `#goose` block behavior |
| [POLY_API_REFERENCE.md](POLY_API_REFERENCE.md) | Current I/O, builtin, and runtime API reference |
| [POLY_TUTORIAL.md](POLY_TUTORIAL.md) | Beginner tutorial covering I/O, error handling, and loop ranges |
| [POLY_QUICK_REFERENCE.md](POLY_QUICK_REFERENCE.md) | Current syntax, I/O, strings, foreign blocks, and CLI quick reference |
| [POLY_TROUBLESHOOTING_GUIDE.md](POLY_TROUBLESHOOTING_GUIDE.md) | Current error, input/output, and target troubleshooting |
| [POLY_DOCUMENTATION_STYLE_GUIDE.md](POLY_DOCUMENTATION_STYLE_GUIDE.md) | Example, fence, terminology, and line-length rules |
| [POLY_V2_SUPPORT_MATRIX.md](POLY_V2_SUPPORT_MATRIX.md) | Frozen Rust/C/asm/JS/Goose feature contract and per-target summary |
| [POLY_RELEASE_GUIDE.md](POLY_RELEASE_GUIDE.md) | Preview release readiness, version-bearing file map, and release procedure |
| [POLY_V2_PREVIEW_RELEASE_NOTES.md](POLY_V2_PREVIEW_RELEASE_NOTES.md) | Preview highlights, compatibility notes, and verification evidence |
| [POLY_ROADMAP_v2.md](POLY_ROADMAP_v2.md) | Planned v2 work and design decisions |
| [CHANGELOG.md](CHANGELOG.md) | Versioned implementation history |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Development and verification workflow |
| [PROGRESS.md](PROGRESS.md) | Dated implementation log |
| [TODO.md](TODO.md) | Outstanding work items |
| [BRANCH_PROTECTION.md](BRANCH_PROTECTION.md) | Required branch protection settings |

Complete runnable Poly programs live in [`examples/`](examples/); executable behavior fixtures live in [`tests/`](tests/).

## Target Contract

- `--target rust` is the default and emits Rust through the AST, IR, optimizer, and Rust code generator.
- `--target c` emits and checks a C11 orchestration subset through `poly-c-codegen`.
- `--target asm` emits freestanding Linux x86-64 assembly through `poly-asm-codegen`.
- `--target js` emits an ES2020 JavaScript module through `poly-js-codegen`.
- `--target goose` emits and checks a Goose orchestration subset through `poly-goose-codegen` (the `goose` toolchain must be
on `PATH` for `--check`; emission works without it).
- `#rust`, `#c`, `#asm`, `#js`, and `#goose` blocks are top-level, opaque to Poly, and selected by target.
- `extern rust fn ...`, `extern c fn ...`, `extern asm fn ...`, `extern js fn ...`, and `extern goose fn ...` provide opt-in
Poly-side checks for opaque foreign calls.
- `#cpp` syntax is reserved and rejected explicitly because no C++ backend exists.
- `=` is equality; `:=` is assignment. `==` is rejected legacy syntax.
- Append output uses `put value to "file" -append` (Rust target; the C, asm, and JS backends reject file redirects).
- `put` always writes a trailing newline. `error`, `warn`, and `info` write prefixed messages to stderr.

## Historical References

Everything point-in-time — v1 guides, audit reports, blog drafts, old release notes, social copy, and superseded design
records — lives under [`docs/archive/`](docs/archive/README.md). Those files are not normative for v2 and may describe
older syntax. Use [POLY_MIGRATION_GUIDE_v1.8.md](docs/archive/POLY_MIGRATION_GUIDE_v1.8.md) only when migrating old
source, and then consult the v2 documents for the resulting syntax.

Also historical: `compiler/grammar/poly.bnf` (stale v1.5 draft; `POLY_GRAMMAR.md` is the maintained grammar).

Archived files should be updated only when a statement would actively mislead a current reader, or when a current document
needs to point at them. Examples marked `poly fragment` may retain version-specific syntax for historical explanation.

## Verification

Run the same checks used by the v2 CI workflow:

~~~bash
python3 scripts/check_markdown.py
python3 scripts/check_poly_examples.py --poly-bin compiler/target/release/poly  # every markdown file, as CI runs it
python3 scripts/check_generated_paths.py
python3 scripts/check_backends.py
cd compiler
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace -- --test-threads=1
cargo check --workspace --all-targets
~~~
