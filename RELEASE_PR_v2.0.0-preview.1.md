# Release PR: Poly 2.0.0-preview.1

## Release Target

**Poly `2.0.0-preview.1`** — Preview release candidate for the target-aware Rust/C compiler model.

**Branch:** `v1.7.3-audit-fixes` → `master`

---

## Release Evidence

### Compiler & Toolchain

- **Poly version:** 2.0.0-preview.1
- **Rust toolchain:** `1.93.0` (pinned via `rust-toolchain.toml`)
- **C compiler:** System default (`cc` / `gcc`)

### Verification Commands & Results

All commands executed from a clean checkout on the `v1.7.3-audit-fixes` branch:

| Check | Command | Result |
|-------|---------|--------|
| Format | `cargo fmt --all -- --check` | ✅ Pass |
| Lint | `cargo clippy --workspace --all-targets -- -D warnings` | ✅ Pass |
| Tests | `cargo test --workspace -- --test-threads=1` | ✅ 99 tests passed, 0 failed |
| Type check | `cargo check --workspace --all-targets` | ✅ Pass |
| Markdown | `python3 scripts/check_markdown.py` | ✅ 46 files, 1298 fence markers, no violations |
| Examples | `python3 scripts/check_poly_examples.py` (9 docs) | ✅ 45 blocks checked, 18 passed, 27 marked fragments |

### Test Summary

- **poly-lexer**: 33 tests ✅
- **poly-parser**: 26 tests ✅ (including 3 fuzz recovery)
- **poly-transpiler**: 24 tests ✅ (checker + codegen + source map)
- **poly-types**: 9 tests ✅
- **poly-wasm**: 3 tests ✅
- **poly-c-codegen**: 0 doc tests ✅
- **Fuzz recovery**: 3 tests ✅

### Runtime Fixtures

- **Rust examples**: All 22 checked and native Rust fixtures compiled ✅
- **C fixture**: Checked, built, and executed ✅
- **Mixed target fixture**: Verified ✅
- **C foreign signatures**: Explicit `extern c fn` declarations validated ✅
- **C unsupported diagnostics**: Verified for all rejected constructs ✅

---

## Feature Summary

### Rust Backend (Default)

Full v2 baseline — all Poly constructs transpile to valid Rust and compile through `rustc` or a generated Cargo project.

### C Backend

Intentionally narrow C11 orchestration subset:

| Supported | Not Supported (use `#c` helpers) |
|-----------|----------------------------------|
| Scalar declarations | File redirects / stdin |
| Plain structs | Vectors, tuples, arrays |
| Functions | Pattern matching |
| Conditions (`if`/`else`) | Closures |
| Loops (`while`, `loop`) | Async/await |
| Arithmetic, bitwise ops | Complex Poly types |
| `put` (stdout), `error`/`warn`/`info` (stderr) | String interpolation |

### Foreign Blocks

- `#rust ... #endrust` — Rust definitions emitted at module scope, opaque to Poly
- `#c ... #endc` — C declarations emitted at file scope, opaque to Poly
- `#cpp ... #endcpp` — Recognized, preserved, explicitly rejected (no backend)
- `extern rust fn ...` / `extern c fn ...` — Optional Poly-side arity/type checks

### CLI

~~~
poly <file.poly>                    # Generate + build (Rust default)
poly --target c <file.poly>         # Generate + build (C)
poly --target c --emit-c <file>     # Emit C to stdout
poly --check <file>                 # Validate without building
poly --emit-rust <file>             # Emit Rust to stdout
poly --intermediate-representation  # Show IR pipeline output
poly --source-map <file>            # Show Poly→Rust source map
poly --format <file>                # Format with rustfmt
poly --diff <file>                  # Show format diff
poly --watch <file>                 # Watch + re-transpile
poly --project <dir> <file>         # Generate Cargo/project dir
poly --repl                         # Interactive REPL
~~~

---

## Audit Risk Status

### Risk 1: Opaque foreign calls remain permissive

**Status:** Accepted for preview. Explicit `extern rust fn` / `extern c fn` declarations are available for stronger diagnostics. The native compiler (rustc/cc) remains the authoritative validator for foreign code.

**Mitigation path:** Post-preview, consider making `extern` declarations mandatory for all foreign calls or adding a `--strict` flag.

### Risk 2: C backend scope is intentionally narrow

**Status:** Documented in support matrix. All unsupported constructs produce clear error messages (e.g., "tuple index access is not supported by the C backend"). Users are directed to `#c` helpers.

### Risk 3: C++ is syntax-only

**Status:** `#cpp` blocks are recognized but explicitly rejected with an actionable diagnostic. No C++ target, codegen, or build path exists.

### Risk 4: v2 preview has compatibility-oriented parser paths

**Status:** Covered by tests. Legacy syntax (`==`, `u"..."`, `u'c'`, `-n` flag) is rejected with migration diagnostics. These paths are kept intentionally for v1→v2 migration UX.

### Risk 5: Old v1 documentation is historical

**Status:** All v1 docs are indexed in `POLY_DOCUMENTATION_INDEX.md` as explicitly non-normative. The v2 document set (spec, grammar, migration guide, support matrix, audit, roadmap) is the maintained source of truth.

### Risk 6: Cross-platform C coverage

**Status:** CI exercises Rust + C on `ubuntu-latest`, `macos-latest`, and `windows-latest`. `POLY_CC` allows explicit compiler selection. Platform-specific behavior is a residual risk to monitor when CI images update.

---

## Compatibility Notes

- `:=` is declaration/assignment; `=` is equality. Legacy `==` is rejected with migration diagnostic.
- Append output: `put value to "file" -append`
- `put` always writes a trailing newline.
- Foreign blocks and `extern` declarations must be top-level.
- C does not provide the Rust runtime surface (input, collections, tuples, pattern matching, closures, async, SQLite, network helpers).

---

## Commit Structure

The branch has been rebased from 36 interleaved commits into 3 focused commits:

~~~
8fc2f9f docs: v2 preview documentation, spec, migration guide, and release notes
c5f6d6d ci: cross-platform Rust/C matrix, WASM validation, and toolchain pinning
a70c4f5 feat: target-aware Rust/C pipeline, language features, and compiler improvements
~~~

- **feat** — All language features, compiler crates, examples, tests, and toolchain changes
- **ci** — CI workflows, WASM validation, and playground updates
- **docs** — All documentation, spec, migration guide, release notes, and metadata

The previous 36-commit history is preserved on the `backup-audit-fixes` branch for reference.

---

## Checklist

- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --workspace --all-targets -- -D warnings`
- [x] `cargo test --workspace -- --test-threads=1` (99/99 passed)
- [x] `cargo check --workspace --all-targets`
- [x] `python3 scripts/check_markdown.py` (46 files clean)
- [x] `python3 scripts/check_poly_examples.py` (9 docs, 45 blocks)
- [x] Rust examples checked, native fixtures compiled
- [x] C fixture checked, built, and executed
- [x] Mixed target fixture verified
- [x] Playground WASM artifact reproducible locally
- [x] Version metadata consistent (2.0.0-preview.1)
- [x] No credentials, machine-specific paths, or generated build dirs
- [x] Separate commits for implementation / tests / docs / CI — **done (3 focused commits)**
- [ ] Hosted cross-platform CI evidence (Linux, macOS, Windows) — **pending CI run**
- [ ] Clean-checkout review — **recommended before tagging**

---

## Known Preview Limitations

1. The C backend is not a general Poly-to-C translator — use `#c` helpers or Rust target
2. Foreign ABI, pointer, ownership, lifetime correctness delegated to native compiler
3. `#cpp` has no implementation
4. Playground WASM byte identity not expected across build environments
5. Hosted cross-platform CI evidence required before final tag

---

## Migration Notes

### From v1.8 to v2

1. **Simple code** (variables, I/O, loops, basic functions): No changes needed
2. **Complex code** (closures, generics, async, traits): Wrap in `#rust ... #endrust` blocks
3. **Equality:** Replace `==` with `=`
4. **Unicode strings:** Use `unicode "text"` instead of `u"text"`
5. **File append:** Use `put value to "file" -append` instead of `-append` flag on its own

### Example

~~~poly
# Before (v1.x)
fn process(items: Vec<i32>): Vec<i32>
    return items.filter(|x| x > 0).map(|x| x * 2)
end fn

# After (v2)
#rust
fn process(items: Vec<i32>) -> Vec<i32> {
    items.into_iter().filter(|x| *x > 0).map(|x| x * 2).collect()
}
#endrust
~~~

---

**This agent does not create or push tags.** A preview tag should be created only after all mandatory checks pass from a clean checkout and hosted CI evidence is recorded.
