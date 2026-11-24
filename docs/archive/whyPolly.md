# Why Write Your Program in Poly?

Poly is a small, assembly-inspired language that transpiles to Rust, C, x86-64 assembly, JavaScript, or Goose from a
single source file. This page lists the reasons a project might choose it. Claims here follow the maintained
documentation ([POLY_SPEC_v2.md](../../POLY_SPEC_v2.md), [POLY_V2_SUPPORT_MATRIX.md](../../POLY_V2_SUPPORT_MATRIX.md), and
[README.md](../../README.md)); Poly is currently a 2.0 preview.

## One program, several targets

- `poly file.poly` emits **Rust** by default; `--target c`, `--target asm`, `--target js`, and `--target goose` switch
  the backend without touching your source.
- Behavior is deliberately consistent across targets: integer overflow wraps the same way everywhere, loop `step`
  semantics are identical on every backend, and an integer literal (`42`, `0xFF`, `0b1010`) means the same number in
  every backend.
- You can pick the delivery format per build — a Cargo project, a C11 binary, a freestanding Linux x86-64 program, an
  ES2020 module, or Goose source — from one codebase.

## Small, regular syntax

- Assignment is `:=`, equality is `=` (the legacy `==` spelling is rejected with a diagnostic), and blocks end with
  spelled-out terminators (`end if`, `end fn`) instead of braces.
- Mutation reads like the assembly it is inspired by: `add total, 5`, `inc count`, `dec count` alongside the explicit
  `total := total + 5`.
- The whole language contract fits in [POLY_SPEC_v2.md](../../POLY_SPEC_v2.md), with a compact grammar in
  [POLY_GRAMMAR.md](../../POLY_GRAMMAR.md) and a one-page [POLY_QUICK_REFERENCE.md](../../POLY_QUICK_REFERENCE.md).

## An escape hatch to the host language

- Foreign blocks (`#rust`, `#c`, `#asm`, `#js`, `#goose`) are copied verbatim into the output, so anything the target
  language can do stays reachable from Poly.
- Optional `extern rust fn ...` (and the other target spellings) declarations let Poly type-check foreign calls at
  arity and type level while the native compiler stays authoritative.
- On the Rust target, `dep name = "version"` pulls in Cargo crates — SQLite, networking, and async runtimes are
  available through the standard runtime helpers.

## Diagnostics instead of silent surprises

- Unsupported constructs are rejected with a diagnostic that names the limitation and suggests the foreign-helper
  escape hatch; nothing lowers to a placeholder silently.
- Arithmetic is strictly same-type: mixing integers and floats requires an explicit `as` cast, so narrowing and
  mixing bugs surface at check time.
- `poly --check` validates without running your program, `--strict` requires every foreign call to be declared, and
  `--watch`, `--format`, `--source-map`, `--emit-*`, and a REPL round out the tooling.

## Verified, not vibes

- Differential tests run the same program through the backends, and `doc_claim_guards` tests pin documented
  per-target claims so the docs cannot drift away from the implementation silently.
- The compiler has a security audit ([SECURITY_AUDIT_REPORT.md](SECURITY_AUDIT_REPORT.md)) covering untrusted-input
  handling, code generation, and CI hardening.

## Know before you commit

- The non-Rust targets are documented subsets — check the support matrix for your required constructs; the Rust
  target is the reference implementation with the full feature set (generics, traits, closures, async, vectors).
- Poly is a preview (`2.0.0-preview.x`): syntax can still move between previews, so pin your toolchain version.
- File output and typed input flags (`--default`, `--mask`, `--timeout`, ...) are Rust-target features; other
  targets reach equivalent behavior through a foreign helper.

If those trade-offs fit your project — one source file, several output formats, predictable cross-target semantics,
and a compiler that tells you when it cannot do what you asked — Poly is worth a try.
