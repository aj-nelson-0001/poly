# Poly 2.0 Roadmap

*From simple language to language-agnostic meta-language*

---

## Vision

Poly becomes a **thin, assembly-inspired syntax layer over any systems language.** It handles the simple, boilerplate-heavy parts of programming. For everything else — generics, async, closures, traits, pattern matching — users provide definitions in foreign language blocks (`#rust`, `#c`, `#cpp`, etc.).

**Key principle:** Foreign blocks provide target-language definitions at file scope. Poly provides the orchestration and generated entry point. The selected native compiler validates foreign code.

**One Poly source file → one complete source file in the target language → compiled by the target compiler.**

---

## Architecture

~~~
source.poly
    │
    ├── #rust blocks ──→ Declarations emitted at module scope
    │
    ├── Poly sections ──→ Lexer → Parser → AST → IR → Codegen → fn main() { ... }
    │
    └── output.rs
            │
            └──→ rustc (or cargo build)
~~~

---

## Phase 1: Core Passthrough (✅ Done)

### What's Implemented

- **Lexer**: `#rust`, `#c`, and `#cpp` blocks lexed as `ForeignBlock { language, content }` tokens
- **Parser**: foreign blocks are represented explicitly in the AST and rejected inside Poly blocks
- **IR**: Rust foreign blocks are lowered into the Rust IR; other targets are handled by target-specific backends
- **Codegen**: Verbatim emission of selected Rust blocks at module scope
- **Checker**: Foreign function names remain compatible as opaque calls; optional `extern rust fn`/`extern c fn` declarations provide Poly-side signature validation while native compilers validate the actual foreign definitions
- **Optimizer**: Foreign blocks are treated as opaque
- **CLI**: `--target rust` and `--target c` flags, plus `--emit-c` and C build/check paths
- **Validation**: Warning when `#rust` blocks contain executable code at top level

### Known Limitations

- Foreign function signatures remain opaque to Poly; the selected Rust/C compiler validates their argument and return types.
- `#cpp` is recognized and preserved in the syntax model, but no C++ code generator exists yet.
- The C backend currently covers the orchestration subset: scalar values, plain structs, functions, conditions, loops, arithmetic, and stdout/stderr output. Complex values should remain in foreign helper definitions.

### Verified Working

~~~poly
#rust
fn rust_function(x: i32) -> i32 {
    x * 2
}
#endrust

var greeting ustring := unicode "Hello from Poly!"
put greeting
var result i32 := rust_function(21)
put "Result: " + result
~~~

Transpiles to valid Rust: `#rust` block emits `rust_function` at module scope, Poly generates `fn main()` which calls it. Compiles and runs.

---

## Phase 1.5: Fix Checker for Foreign Blocks (✅ Done)

### What's Implemented

- **Function name extraction**: `register_foreign_functions()` scans selected foreign blocks for common Rust/C function declarations
- **`is_foreign` flag**: Added to `FunctionSignature` to mark foreign-defined functions
- **Arity bypass**: `check_arguments()` and the generic arity check both skip validation for `is_foreign` functions
- **Type bypass**: Foreign function calls return `Unknown` type (permissive)
- **Full build works**: `poly file.poly` now succeeds with `#rust` blocks; `#cpp` reports an explicit unsupported-backend error
- **Warning fix**: Brace-depth tracking prevents false positives on executable code inside function bodies

---

## Phase 2: Language Boundary

**Status:** The v2 preview intentionally keeps the broader Rust parser/checker surface while the Rust target remains the compatibility baseline. Future simplification must be driven by measured maintenance cost and a migration plan; it is not part of the current release contract.

### Goal

Strip Poly down to only what an assembly-like syntax layer needs. Remove features that should live in `#rust` blocks instead.

### Features to KEEP in Poly

| Feature | Syntax | Why |
|---------|--------|-----|
| Variable declarations | `var x i32 := 0` | Core assembly-like syntax |
| Constants | `const MAX := 100` | Simple declaration |
| I/O | `put`, `get`, `error`, `warn`, `info` | Poly's primary value add |
| File I/O | `put to`, `get from` | Shell-like, simple |
| Arithmetic | `add`, `sub`, `inc`, `dec` | Assembly-inspired |
| Control flow | `if`, `while`, `loop` | Simple loops and conditions |
| Functions | `fn name(params): Type ... end fn` | Basic signatures only |
| Structs | `struct Name ... end struct` | Simple field declarations |
| Primitives | `i32`, `u64`, `bool`, `ustring`, etc. | Core types |
| String interpolation | `"Hello, {name}!"` | Convenience |
| Bitwise ops | `&`, `\|`, `^`, `~`, `<<`, `>>` | Systems programming |

### Features to REMOVE from Poly (move to `#rust`)

| Feature | Why remove |
|---------|-----------|
| Closures (`\|x\| x + 1`) | Use Rust closures in `#rust` blocks |
| Higher-order methods (`.map`, `.filter`, `.reduce`) | Use Rust iterators in `#rust` blocks |
| Generic functions (`fn foo<T>(...)`) | Use Rust generics in `#rust` blocks |
| Generic structs (`struct Foo<T>`) | Use Rust generics in `#rust` blocks |
| Traits and impls | Use Rust traits in `#rust` blocks |
| Enums with data | Use Rust enums in `#rust` blocks |
| Match expressions | Use Rust match in `#rust` blocks |
| Async/await | Use Rust async in `#rust` blocks |
| Modules | Use Rust modules in `#rust` blocks |
| Ownership/borrowing syntax | Use Rust references in `#rust` blocks |
| Lifetimes | Use Rust lifetimes in `#rust` blocks |
| Closures as parameters | Use `impl Fn` in `#rust` blocks |
| Error handling (`Result`, `try`) | Use Rust `?` in `#rust` blocks |
| Type aliases | Use Rust `type` in `#rust` blocks |

### Estimated Impact

- **Parser**: Remove ~60% of expression/statement variants
- **AST**: Remove ~40% of node types
- **IR**: Remove ~50% of statement/expression variants
- **Codegen**: Remove ~70% of generation logic
- **Checker**: Remove ~80% of type inference/checking
- **Optimizer**: Can be significantly simplified or removed

---

## Phase 3: Polish & Tooling

### CLI Improvements

- `poly build file.poly` — optional future alias for the existing default compile workflow
- `poly --check file.poly` — validate the default Rust target
- `poly --emit-rust file.poly` — emit Rust to stdout
- `poly --watch file.poly` — watch and re-transpile on changes

### LSP

- Diagnostics for Poly syntax errors
- Completion for Poly keywords
- Hover information for Poly constructs
- **Passthrough**: No analysis of `#rust` blocks (let rust-analyzer handle them)

### VS Code Extension

- Syntax highlighting for Poly + `#rust` blocks
- Language configuration (brackets, indentation)
- Integration with rust-analyzer for `#rust` blocks

---

## Phase 4: Documentation Consolidation

**Status:** Maintained v2 references are now indexed in `POLY_DOCUMENTATION_INDEX.md`; older guides remain available as explicitly historical migration material.

### From 38 files to 5

| Keep | Purpose |
|------|---------|
| `README.md` | Quick start + overview |
| `POLY_SPEC.md` | Complete language specification |
| `CHANGELOG.md` | Version history |
| `CONTRIBUTING.md` | How to contribute |
| `examples/` | Self-documenting examples |

### Remove

All 20+ `POLY_*_GUIDE.md` files. The language is simple enough that the spec + examples are sufficient.

---

## Design Decisions

### Q: How do Rust-defined items become visible to Poly?

**A:** Functions and structs defined in `#rust` blocks are emitted at module scope (before `fn main()`). Poly code in `fn main()` can call them naturally.

### Q: What about `fn main()`?

**A:** Poly **always** generates `fn main()`. The `#rust` blocks provide declarations; Poly provides the entry point. Users cannot override `fn main()` — that would defeat the purpose.

### Q: Can I have multiple `#rust` blocks?

**A:** Yes. They're emitted in order at module scope.

### Q: What about future target languages (`#cpp`)?

**A:** `#cpp` syntax is reserved and currently rejected with an explicit diagnostic. A C++ backend needs a deliberate decision about direct C++ emission, compiler selection, and runtime/type mappings before implementation.

---

## Timeline

| Phase | Status | Effort |
|-------|--------|--------|
| Phase 1: Core Passthrough | ✅ Complete | ~2 hours |
| Phase 2: Language Boundary | ✅ Preview contract documented | Ongoing design work |
| Phase 2.5: C backend | ✅ Hardened Preview | ~1 day |
| Phase 3: Polish & Tooling | 🟡 In progress | Cross-platform CI and interface diagnostics are now implemented; packaging and UX remain |
| Phase 3.5: Preview Release Hardening | 📋 Current | Support matrix, release checklist, and clean-tree review |
| Phase 4: Documentation | ✅ Maintained v2 set indexed | Ongoing historical cleanup |

---

## Migration Guide

### For existing Poly users

1. **Simple code** (variables, I/O, loops, basic functions): No changes needed
2. **Complex code** (closures, generics, async, traits): Wrap in `#rust ... #endrust` blocks
3. **Type checker**: Will no longer analyze `#rust` blocks (use `cargo check` instead)

### Example migration

**Before (Poly 1.x):**
~~~poly fragment
fn process(items: Vec<i32>): Vec<i32>
    return items.filter(|x| x > 0).map(|x| x * 2)
end fn

var data := [1, -2, 3, 4]
var result := process(data)
put result
~~~

**After (Poly 2.0):**
~~~poly fragment
#rust
fn process(items: Vec<i32>) -> Vec<i32> {
    items.into_iter().filter(|x| *x > 0).map(|x| x * 2).collect()
}
#endrust

var data := [1, -2, 3, 4]
var result := process(data)
put result
~~~
