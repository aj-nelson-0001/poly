# Poly Project Audit Report

**Date:** September 6, 2026 (clippy and documentation items resolved 2026-09-07)
**Branch:** v1.7.3-audit-fixes
**Status:** Build clean, 383 tests pass, 0 failures, clippy strict-CI clean

---

## 1. Build & Tests

- **Build**: 0 errors
- **Tests**: **383 passed**, 0 failed (20 test suites)
- **Example programs**: All compile and pass `--check`
- **Snapshot tests**: All pass (byte-for-byte output verification)

---

## 2. Clippy / Compiler Warnings (0 remaining)

All 8 warnings that would have failed the strict CI command
(`cargo clippy --workspace --all-targets -- -D warnings`) defined in the
support matrix were fixed on 2026-09-07. The strict command now passes.

### poly-asm-codegen (5 warnings — fixed)

1. **`manual_div_ceil`** — `(x + 7) / 8` → `x.div_ceil(8)` (line 66) ✅
2. **`manual_div_ceil`** — `(f.size + 7) / 8` → `f.size.div_ceil(8)` (line 340) ✅
3. **`manual_div_ceil`** — `(f.size + 7) / 8` → `f.size.div_ceil(8)` (line 882) ✅
4. **`collapsible_match`** — nested `if let` collapsed into outer pattern (line 593) ✅
5. **`collapsible_match`** — nested `if let` collapsed into outer pattern (line 1112) ✅

### poly-transpiler test `snapshot_tests.rs` (3 warnings — fixed)

6. **Unused import** — removed `poly_lexer::Lexer` (line 16) ✅
7. **Unused import** — removed `poly_parser::Parser` (line 17) ✅
8. **`needless_borrows_for_generic_args`** — `&snapshot_dir()` → `snapshot_dir()` (line 75) ✅

---

## 3. Codegen Optimizations Applied

All changes are internal to `poly-intermediate-representation/src/codegen.rs`. The Poly language and generated output are unchanged.

### 3a. Output Buffer Pre-allocation
**Before:** `String::new()` — starts empty, reallocates as it grows
**After:** `String::with_capacity(4096)` — pre-allocates 4KB for typical programs

### 3b. Indent Writing
**Before:** `indent_str()` called `String::repeat(self.indent)` on **every line**, allocating a new `String`
**After:** Writes indent characters directly via `push_str("    ")` in a loop — zero allocation

### 3c. `writeln_fmt` — Eliminates Intermediate `format!` Strings
**Before:** `self.writeln(&format!("let mut {}: {} = {};", name, ty, val))` — `format!` creates a temporary `String`, `writeln` copies it to output
**After:** `self.writeln_fmt(format_args!(...))` — writes indent + formatted args directly to the output buffer in one pass, no intermediate `String`

Applied to all hot paths:
- `VarDecl` / `LetDecl` statements (every variable declaration)
- `Assignment` / `Mutation` statements (every assignment)
- `Return` / `Put` / `Error` / `Warn` / `Info` statements
- `If` / `While` / `Match` control flow
- `gen_put` (every `put` statement)
- `gen_function` / `gen_struct` / `gen_enum` declarations

### 3d. `gen_loop_range` Optimization
**Before:** Built result string with `push_str(&format!(...))` — double allocation
**After:** Uses `writeln!` directly on the result `String` — single allocation
Also pre-allocates `String::with_capacity(128)` for loop bodies

### 3e. Boolean Literal Optimization (`gen_expr`)
**Before:** `value.to_string()` — allocates for every `true`/`false`
**After:** Returns `&str` constants — zero allocation for the common case

### 3f. Identifier Lookup Optimization (`gen_expr`)
**Before:** `self.enum_variants.get(name).cloned().unwrap_or_else(|| name.clone())`
**After:** `match` with explicit branches — avoids unnecessary `.cloned()` in the common (non-enum) case

### Impact
- **N fewer `String::repeat()` allocations** per program (indent)
- **N fewer `format!` intermediate strings** per program (writeln_fmt)
- **1 fewer heap allocation** per output line (indent + content written directly)
- **Pre-allocated buffer** avoids ~log₂(output_size) reallocations

---

## 4. Missing / Incomplete Features

### 4a. Backends Not Implemented

| Feature | Status |
|---|---|
| **JS backend** | Design doc only (`POLY_JS_DESIGN.md`). No `#js` blocks, no codegen, no CLI support. Implementation on hold per review decision. |
| **C++ backend** | `#cpp` syntax reserved and explicitly rejected with diagnostic. No implementation. |

### 4b. C Backend Gaps

The C backend covers the orchestration subset but many features require `#c` helpers:

| Missing Feature | Notes |
|---|---|
| File redirects (`put to`, `get from`) | Must use `#c` helper |
| `get` / stdin / typed input | Must use `#c` helper |
| Multi-range loops | Rejected by C backend |
| Collection loops | Limited to shallow C-array form; vectors unsupported |
| Closures / higher-order operations | Not supported |
| Tuples | Not supported |
| `Option` / `Result` / pattern matching | Not supported |
| Async / generic functions | Not supported |
| Value-position string concatenation | Needs `#c` helper |
| Struct methods / generics | Rejected |
| Enums, traits, impls, modules, aliases | Not supported |

### 4c. ASM Backend Gaps

| Missing Feature | Notes |
|---|---|
| File redirects | Must use `#asm` helper |
| For-in loops | Not implemented |
| Dereferencing | Not implemented |
| Async / generic Poly functions | Not supported |
| Limited type support | Only integer, bool, string, struct, enum |

### 4d. Other Known Limitations

- **`^` is bitwise XOR**, not exponentiation — float powers need a library call or `sqrt`-style helper
- **`Type::method()` call syntax** has confusing error messages (hits associated-function arity path)
- **Generic type checking is permissive** — type variables often resolve to `Unknown`; bounds not validated semantically
- **Source-map line mappings are best-effort** heuristics, despite precise statement spans being available in the AST
- **LSP hover/completion** — UTF-16 issue was fixed in v1.7.3 but non-ASCII edge cases remain
- **Custom JSON parser** lacks full surrogate-pair escape handling
- **CI doesn't**: rebuild/verify the WASM bundle, run browser tests, package the VS Code extension, or execute benchmark targets
- **Playground wasm** could shrink further with `wasm-opt` if installed

---

## 5. Documentation Bugs Found

### `with validate` is not implemented (fixed 2026-09-07)
`POLY_TUTORIAL.md` (line 117) documented `with validate` as a working feature:

~~~poly
var age i32 := get with validate |x| x >= 1 && x <= 150
~~~

However, this is a **parser-only stub**. The syntax is parsed and stored in the AST, but the checker and codegen ignore it entirely. The validation closure is never executed at runtime. The v2 spec and API reference both state this is "not part of the maintained runnable v2 API."

**Resolved:** the tutorial's three `with validate` presentations now show the
working loop-based validation pattern (verified end-to-end with `--check` and a
runtime run) and carry a note pointing to the v2 spec. The tutorial is already
marked historical/non-normative.

---

## 6. Code Quality Notes

### No `todo!()` calls in production code
The single `todo!` found in `codegen.rs` line 931 is a string literal in a list of known Rust macros — not an actual `todo!()` invocation.

### `unreachable!()` calls are appropriate
All 14 `unreachable!()` calls are defensive assertions in code paths that should never execute.

### Stubs resolved
The earlier stubs (`db_execute`, `--mask`, `--until`, `http_get`, `tcp_connect`) have been implemented. Test assertions in `checker.rs` confirm the old stub warnings no longer fire.

---

## 7. Documentation Status

- **Maintained v2 docs**: 15 files indexed in `POLY_DOCUMENTATION_INDEX.md`
- **Historical v1 docs**: ~20+ files retained for migration context, marked as non-normative
- **No TODO/FIXME/HACK comments** found in any Markdown documentation files

---

## 8. Summary

| Category | Status |
|---|---|
| Build | ✅ Clean |
| Tests | ✅ 383/383 pass |
| Clippy | ✅ 0 warnings (strict CI passes) |
| Codegen optimizations | ✅ Applied (5 changes, all verified) |
| JS backend | 🔴 Not implemented (design only) |
| C++ backend | 🔴 Not implemented (reserved) |
| C backend | 🟡 Core works; many features need `#c` helpers |
| ASM backend | 🟡 Core works; several features missing |
| Generic type checking | 🟡 Permissive, not fully validated |
| LSP | 🟡 Works; UTF-16 fixed, edge cases remain |
| WASM bundle | ✅ Built and tested; not verified in CI |
| VS Code extension | ✅ Built; not packaged in CI |
| Documentation | ✅ Tutorial `with validate` examples corrected |
