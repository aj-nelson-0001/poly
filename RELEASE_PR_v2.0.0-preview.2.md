# Release PR: Poly 2.0.0-preview.2

## Release Target

**Poly `2.0.0-preview.2`** — Second preview of the target-aware Rust/C/Asm compiler model.

**Branch:** `v1.7.3-audit-fixes` → `master`

**Evidence refreshed:** 2026-09-09 (local clean-tree verification of the
version-sync pass; the assembly target and C backend expansion are included
under "Feature Summary").

---

## Release Evidence

### Compiler & Toolchain

- **Poly version:** 2.0.0-preview.2 (workspace version, `poly --version`, REPL banner, and playground footer agree)
- **Rust toolchain:** `1.98.0` (pinned via `rust-toolchain.toml`)
- **C compiler:** System default `cc` (GCC 16.2.1)
- **Assembler:** System `cc` for the Linux x86-64 asm fixture (`-nostartfiles`)

### Verification Commands & Results

All commands executed locally on the `v1.7.3-audit-fixes` branch at the version-sync pass (release-metadata changes staged):

| Check | Command | Result |
|-------|---------|--------|
| Format | `cargo fmt --all -- --check` | ✅ Pass |
| Lint | `cargo clippy --workspace --all-targets -- -D warnings` | ✅ Pass |
| Tests | `cargo test --workspace -- --test-threads=1` | ✅ 406 tests passed, 0 failed |
| Type check | `cargo check --workspace --all-targets` | ✅ Pass |
| Dependency audit | `cargo audit` | ✅ 0 vulnerabilities (81 dependencies) |
| Markdown | `python3 scripts/check_markdown.py` | ✅ 49 files, 1,310 tilde fence markers, no violations |
| Doc examples (maintained set) | `python3 scripts/check_poly_examples.py` (9 v2 docs) | ✅ 44 blocks: 18 passed, 26 marked fragments, 0 unmarked failures |
| Doc examples (full tree) | `python3 scripts/check_poly_examples.py` (all 49 docs) | ✅ 564 blocks: 229 passed, 335 marked fragments, 0 unmarked failures |
| Rust examples | `poly --target rust --check examples/*.poly` | ✅ All 22 pass |
| C fixture | `--check`, build, execute | ✅ Output `30` / `3` verified |
| Asm fixture | `--check`, `-o` build, assemble, execute | ✅ Output `42` / `10` / `30` / `60` verified |
| Mixed target fixture | `--target rust` / `--target c` checks | ✅ Pass |
| Playground examples | All 9 embedded examples pass `poly --check` | ✅ Pass |
| Playground wasm | Rebuilt from preview.2 compiler (408K) + `scripts/playground_wasm_smoke.mjs` | ✅ 5 good + 6 rejected cases pass |

### Test Summary

- **poly-cli**: 110 tests ✅ (12 unit, 53 error-handling, 7 error-recovery, 38 integration)
- **poly-transpiler**: 92 tests ✅ (86 unit, 3 fuzz recovery, 3 snapshot)
- **poly-parser**: 60 tests ✅ (56 unit, 4 fuzz recovery)
- **poly-intermediate-representation**: 45 tests ✅
- **poly-lexer**: 38 tests ✅
- **poly-asm-codegen**: 22 tests ✅
- **poly-lsp**: 18 tests ✅
- **poly-c-codegen**: 9 tests ✅
- **poly-types**: 9 tests ✅
- **poly-wasm**: 3 tests ✅

---

## Feature Summary

### Rust Backend (Default)

Full v2 baseline — all Poly constructs transpile to valid Rust and compile through `rustc` or a generated Cargo project. Preview.2 adds language-wide `pop()` (type-checks as the element type, lowers to `pop().unwrap_or_default()`).

### C Backend

C11 orchestration subset, widened in preview.2:

| Supported | Not Supported (use `#c` helpers) |
|-----------|----------------------------------|
| Scalar declarations | File redirects / file input |
| Plain structs + struct literals (C99 compound literals) | Vectors, maps, sets |
| Tuples (anonymous structs, `.N` access, nesting) | Typed input flags |
| Enum declarations + simple variants in expressions and match | Capturing closures |
| `match` (literal, wildcard, range, identifier arms) | Async/await |
| Non-capturing closures | Payload-carrying enum variants |
| Plain `get` (stdin line, optional prompt) | Guards / structured match patterns |
| Functions, conditions, loops, arithmetic, bitwise ops | Generic functions / structs |
| `put` (stdout), `error`/`warn`/`info` (stderr) | Complex Poly types |

### Asm Backend (New in Preview.2)

Linux x86-64 freestanding subset via `--target asm`:

- `extern asm fn ...` declarations and `#asm` foreign blocks
- `_start` entry point that calls `fn main` when declared
- Static descriptor-backed vectors with index access and `for x in <vector>`; `push`/`pop`/`len` switch to a growable in-place runtime (static bump arena, doubling growth, exhaustion exits 42)
- Pointer dereference, `&T` locals, and `for x in <string>` iteration
- Integer and char output; general string values are rejected with guidance

### Foreign Blocks

- `#rust ... #endrust` — Rust definitions emitted at module scope, opaque to Poly
- `#c ... #endc` — C declarations emitted at file scope, opaque to Poly
- `#asm ... #endasm` — Assembly definitions emitted at file scope, opaque to Poly
- `#cpp ... #endcpp` — Recognized, preserved, explicitly rejected (no backend)
- `extern rust fn ...` / `extern c fn ...` / `extern asm fn ...` — Optional Poly-side arity/type checks

### CLI

~~~
poly <file.poly>                    # Generate + build (Rust default)
poly --target c <file.poly>         # Generate + build (C)
poly --target asm <file.poly>       # Generate + build (Linux x86-64 asm)
poly --target c --emit-c <file>     # Emit C to stdout
poly --target asm --emit-asm <file> # Emit assembly to stdout
poly --check <file>                 # Validate without building
poly --emit-rust <file>             # Emit Rust to stdout
poly --intermediate-representation  # Show IR pipeline output
poly --source-map <file>            # Show Poly→Rust source map (statement-precise)
poly --format <file>                # Format with rustfmt
poly --diff <file>                  # Show format diff
poly --watch <file>                 # Watch + re-transpile
poly --project <dir> <file>         # Generate Cargo/project dir
poly --repl                         # Interactive REPL
~~~

---

## Audit Risk Status

### Risk 1: Opaque foreign calls remain permissive

**Status:** Accepted for preview. Explicit `extern rust fn` / `extern c fn` / `extern asm fn` declarations are available for stronger diagnostics. The native compiler (rustc/cc/assembler) remains the authoritative validator for foreign code.

**Mitigation path:** Post-preview, consider making `extern` declarations mandatory for all foreign calls or adding a `--strict` flag.

### Risk 2: C backend scope is intentionally narrow

**Status:** Widened in preview.2 (tuples, match, struct literals, enum variants, plain `get`, non-capturing closures). Remaining gaps — file redirects, file input, vectors, capturing closures, async — produce clear diagnostics directing users to `#c` helpers or the Rust target.

### Risk 3: C++ is syntax-only

**Status:** `#cpp` blocks are recognized but explicitly rejected with an actionable diagnostic. No C++ target, codegen, or build path exists.

### Risk 4: The asm target is a narrow Linux-only subset

**Status:** New in preview.2. It emits freestanding x86-64 assembly for Linux syscalls, runs under CI on Linux only, and rejects unsupported constructs (general string values, maps/sets, non-vector methods) with guidance instead of miscompiling. Grow-heap exhaustion exits with status 42 rather than corrupting memory.

### Risk 5: v2 preview has compatibility-oriented parser paths

**Status:** Covered by tests. Legacy syntax (`==`, `&&`, `||`, `!`, `^`, `%`, `&`, `|`, `~`, compound assignments, `add`/`sub`/`inc`/`dec`) is rejected with migration diagnostics. These paths are kept intentionally for v1→v2 migration UX.

### Risk 6: Old v1 documentation is historical

**Status:** All v1 docs are indexed in `POLY_DOCUMENTATION_INDEX.md` as explicitly non-normative. The v2 document set (spec, grammar, migration guide, support matrix, audit, roadmap) is the maintained source of truth and was re-synced to the preview.2 implementation in this pass.

### Risk 7: Cross-platform C coverage

**Status:** CI exercises Rust + C on `ubuntu-latest`, `macos-latest`, and `windows-latest`. `POLY_CC` allows explicit compiler selection. Platform-specific behavior is a residual risk to monitor when CI images update.

---

## Compatibility Notes

- `:=` is declaration/assignment; `=` is equality. Legacy `==` is rejected with migration diagnostic.
- Mutation uses explicit re-assignment (`x := x + 1`); compound assignments (`+=`) and `add`/`inc`-style statements are retired with migration hints.
- Logical/bitwise/remainder operators are keywords (`and`, `or`, `not`, `xor`, `mod`, `bitand`, `bitor`, `bitnot`, `shift left/right`); the symbol forms are rejected with migration diagnostics (`<<`/`>>` remain valid).
- Append output: `put value to "file" -append`
- `put` always writes a trailing newline.
- Foreign blocks and `extern` declarations must be top-level.
- C does not provide the full Rust runtime surface (see feature table); the asm target is narrower still.

---

## Branch State

This release consolidates the follow-up passes that landed after
`2.0.0-preview.1` (assembly target, C backend expansion, `pop()`, source
maps, LSP UTF-16 symbols, `-o` fix, CI wasm/asm smoke jobs) plus the
version-sync pass that documents them. The branch is ahead of
`origin/v1.7.3-audit-fixes` and ready to merge to `master` after hosted CI
evidence is recorded.

---

## Checklist

- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --workspace --all-targets -- -D warnings`
- [x] `cargo test --workspace` (406/406 passed)
- [x] `cargo check --workspace --all-targets`
- [x] `cargo audit` (0 vulnerabilities)
- [x] `python3 scripts/check_markdown.py` (49 files clean)
- [x] `python3 scripts/check_poly_examples.py` (564 blocks, 0 unmarked failures)
- [x] All 9 playground examples pass `poly --check`; `poly.wasm` rebuilt and smoke-tested
- [x] Rust examples checked (22/22), mixed target fixture verified
- [x] C fixture checked, built, and executed (output verified)
- [x] Asm fixture checked, built, and executed (output verified)
- [x] Version metadata consistent (2.0.0-preview.2 across Cargo, CLI, REPL, playground)
- [x] No credentials, machine-specific paths, or generated build dirs
- [ ] Hosted cross-platform CI evidence (Linux, macOS, Windows) — **pending CI run**
- [ ] Clean-checkout review — **recommended before tagging**

---

## Known Preview Limitations

1. The C backend is not a general Poly-to-C translator — use `#c` helpers or Rust target
2. The asm target is Linux x86-64 only and covers a narrow orchestration subset
3. Foreign ABI, pointer, ownership, lifetime correctness delegated to native compiler
4. `#cpp` has no implementation
5. Playground WASM byte identity not expected across build environments
6. Hosted cross-platform CI evidence required before final tag

---

## Migration Notes

### From v1.8 to v2

1. **Simple code** (variables, I/O, loops, basic functions): No changes needed
2. **Complex code** (closures, generics, async, traits): Wrap in `#rust ... #endrust` blocks
3. **Equality:** Replace `==` with `=`
4. **Operators:** Logical/bitwise/remainder operators are keywords — `and`, `or`, `not`, `xor`, `mod`, `bitand`, `bitor`, `bitnot`, `shift left/right`; the symbol forms (`&&`, `||`, `!`, `^`, `%`, `&`, `|`, `~`) are rejected with migration diagnostics (`<<`/`>>` remain valid)
5. **Mutation:** Replace `x += n` with `x := x + n`; replace `add x` / `inc x` with `x := x + 1`
6. **Unicode strings:** Use `unicode "text"` instead of `u"text"`
7. **File append:** Use `put value to "file" -append`

### Example

~~~poly fragment
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
