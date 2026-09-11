# Poly Documentation Index

Poly 2.0.0-preview.3 is the current implementation baseline. The files below are the maintained sources of truth for syntax, targets, tooling, and migration.

## Current Documentation

| Document | Purpose |
|---|---|
| [README.md](README.md) | Project overview, quick start, features, CLI, and links |
| [POLY_SPEC_v2.md](POLY_SPEC_v2.md) | Current v2 syntax and language contract |
| [POLY_GRAMMAR.md](POLY_GRAMMAR.md) | Compact v2 grammar accepted by the parser |
| [POLY_MIGRATION_GUIDE_v2.md](POLY_MIGRATION_GUIDE_v2.md) | Migration from v1.8 to the v2 preview and target selection |
| [POLY_C_BLOCKS.md](POLY_C_BLOCKS.md) | C11 backend scope and `#c` block behavior |
| [POLY_JS_BLOCKS.md](POLY_JS_BLOCKS.md) | JavaScript backend scope and `#js` block behavior |
| [POLY_API_REFERENCE.md](POLY_API_REFERENCE.md) | Current I/O, builtin, and runtime API reference |
| [POLY_QUICK_REFERENCE.md](POLY_QUICK_REFERENCE.md) | Current syntax and I/O quick reference |
| [POLY_DOCUMENTATION_STYLE_GUIDE.md](POLY_DOCUMENTATION_STYLE_GUIDE.md) | Example, fence, and terminology rules |
| [POLY_V2_AUDIT.md](POLY_V2_AUDIT.md) | Verified v2 status and remaining risks |
| [POLY_V2_SUPPORT_MATRIX.md](POLY_V2_SUPPORT_MATRIX.md) | Frozen Rust/C feature and foreign-interface contract |
| [POLY_PREVIEW_RELEASE_CHECKLIST.md](POLY_PREVIEW_RELEASE_CHECKLIST.md) | Preview release readiness and hygiene checklist |
| [POLY_V2_PREVIEW_RELEASE_NOTES.md](POLY_V2_PREVIEW_RELEASE_NOTES.md) | Preview highlights, compatibility notes, and verification evidence |
| [POLY_ROADMAP_v2.md](POLY_ROADMAP_v2.md) | Planned v2 work and design decisions |
| [CHANGELOG.md](CHANGELOG.md) | Versioned implementation history |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Development and verification workflow |

Complete runnable Poly programs live in [`examples/`](examples/); executable behavior fixtures live in [`tests/`](tests/).

## Target Contract

- `--target rust` is the default and emits Rust through the AST, IR, optimizer, and Rust code generator.
- `--target c` emits and checks a C11 orchestration subset through `poly-c-codegen`.
- `--target asm` emits freestanding Linux x86-64 assembly through `poly-asm-codegen`.
- `#rust`, `#c`, and `#asm` blocks are top-level, opaque to Poly, and selected by target.
- `extern rust fn ...`, `extern c fn ...`, and `extern asm fn ...` provide opt-in Poly-side checks for opaque foreign calls.
- `#cpp` syntax is reserved and rejected explicitly because no C++ backend exists.
- `=` is equality; `:=` is assignment. `==` is rejected legacy syntax.
- Append output uses `put value to "file" -append`.
- `put` always writes a trailing newline. `error`, `warn`, and `info` write prefixed messages to stderr.

## Historical References

The v1 guides, blog posts, release notes, and expanded drafts remain in the repository as historical material. They may describe syntax or features from older releases and are not normative for v2. Use [POLY_MIGRATION_GUIDE_v1.8.md](POLY_MIGRATION_GUIDE_v1.8.md) only when migrating old source, and then consult the v2 documents for the resulting syntax.

Historical documents include:

- `POLY_LANGUAGE_SPECIFICATION_EXPANDED.md`
- `THE_POLY_PROGRAMMING_LANGUAGE.md`
- `POLY_COMPREHENSIVE_GUIDE.md`
- `POLY_TUTORIAL.md`
- `POLY_BEST_PRACTICES.md`
- `POLY_PERFORMANCE_GUIDE.md`
- `POLY_SECURITY_GUIDE.md`
- `POLY_DEPLOYMENT_GUIDE.md`
- `POLY_INTEGRATION_GUIDE.md`
- `POLY_COMPATIBILITY_GUIDE.md`
- `POLY_VERSIONING_GUIDE.md`
- `POLY_FUTURE_MIGRATION_GUIDE.md`
- `BLOG_POST_*.md` and `RELEASE_NOTES_*.md`

These files should be updated when a statement would mislead a current reader. Examples marked `poly fragment` are allowed to retain version-specific or context-dependent syntax for historical explanation.

## Verification

Run the same checks used by the v2 CI workflow:

~~~bash
python3 scripts/check_markdown.py
python3 scripts/check_poly_examples.py README.md POLY_SPEC_v2.md POLY_ROADMAP_v2.md POLY_C_BLOCKS.md POLY_JS_BLOCKS.md POLY_V2_AUDIT.md POLY_MIGRATION_GUIDE_v2.md POLY_V2_SUPPORT_MATRIX.md POLY_PREVIEW_RELEASE_CHECKLIST.md POLY_V2_PREVIEW_RELEASE_NOTES.md
cd compiler
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace -- --test-threads=1
cargo check --workspace --all-targets
~~~
