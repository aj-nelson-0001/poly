# Poly Compiler Audit — Round 2

Follow-up to `AUDIT_REPORT.md` (round 1). This round added a fuzz-style
differential generator and re-probed areas the curated suites never
exercise. Findings continue the round-1 numbering.

## Tooling added: `scripts/fuzz_backends.py`

A seed-driven generator that produces novel Poly programs (arithmetic
helper functions over a fixed input lattice, if/else mutation statements,
nested calls in a call-DAG, two same-fn calls in one expression) and runs
each through all four targets, failing on any output divergence.

- **100 seeds green** after fixes (seeds 1–30 also pass).
- Generator deliberately bounds `*`, `/`, `mod`, and `shift left`
  operands so programs never overflow i32: overflow semantics are
  target-defined (finding 14 below) and would pollute the parity signal.

## Findings

### 11. ✅ FIXED — Parameters were immutable on the Rust target

`x := x + 1` inside `fn f(x: i32): i32` compiled on C/JS/asm (value
semantics — the assignment rebinds the local copy) but failed rustc with
E0384 on the Rust target. The checker accepted it; only rustc rejected.

**Fix:** the IR→Rust codegen scans each function body (including loop,
if, and match sub-bodies) for assignments to parameter names and emits
`mut` on exactly those parameters. Pinned by
`assigned_parameters_lower_to_mut_bindings`.

### 12. ✅ FIXED — asm shared one result slot per callee

Two calls to the same function in one expression evaluated wrongly on
asm: results were stored in a **name-keyed** `__call_{fn}` slot, so the
second call overwrote the first's value before the operator read it.

- `f(1) + f(2)` printed `40` on asm vs `30` on rust/c/js
- `add3(id(1), id(2), id(3))` printed `9` vs `6` (arguments clobbered)
- `fib(20)` printed `0` (recursion with computed args)

**Fix:** call results now use `frame.allocate_temp()` (fresh slot per
call) — the allocator whose doc comment described this exact hazard.
Pinned by fuzzer call-nesting plus the curated call-args probe.

### 13. ✅ FIXED — asm emitted unassemblable large immediates

`put 0 - 2147483648` emitted `movq $2147483648, -56(%rbp)`; GNU `as`
rejects immediates that don't fit a sign-extended 32-bit field
("operand type mismatch"). The value fits Poly's 64-bit asm registers —
the emission was simply wrong.

**Fix:** `emit_store_const` uses `movabsq` for register targets and a
`movabsq` + `pushq`/`popq` scratch sequence for stack slots when the
constant exceeds i32; loop-step induction arithmetic takes the same 64-bit
path. Pinned by `out_of_range_immediates_use_movabs`.

### 14. ✅ RESOLVED — Integer overflow semantics are now defined per-target

The language decision was made: default-width (`i32`) arithmetic is
two's-complement wrap on all four backends — `100000 * 100000` is
`1410065408` everywhere. Division and modulo by `-1` is guarded on both
widths (`INT_MIN / -1` is `INT_MIN`, mod is `0`; no SIGFPE on asm).
Declared-`i64` arithmetic stays exact 64-bit on rust/C/asm; JS is exact
to its documented 2^53 double limit. Big integer literals (outside i32)
are i64-class on every target.

The fuzzer now exercises overflow paths directly (previously it bounded
operands to stay in defined territory). The `poly --check` to `rustc`
emission gap is closed: the optimizer uses `checked_*` and refuses to fold
overflowing constants, so `--check` and `--emit-rust` agree.

### 15. ✅ verified clean — areas probed with no divergence

- **Division/mod**: `7 / 2`, negative-operand `div`/`mod` — all four
  targets agree (JS truncation is correctly normalized per-backend)
- **String + int concat** (`"v=" + 5`), bool printing (`put true`)
- **Shadowing** (`var x` twice in one scope parses and checks)
- **Double negation** (`- -5`), `not`, string comparison (`"a" < "b"`)
- **Recursion** (fib(20) = 6765 after fix 12), nested/derived call args

## Test status

- 504 workspace tests pass, 0 failed (2 new regression tests this round)
- All 6 curated differential suites remain byte-identical on 4 targets
- Fuzz suite: 100 seeds × 4 targets, all agreeing
