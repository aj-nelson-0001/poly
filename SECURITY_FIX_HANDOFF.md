# Security Fixes — Handoff (resolved)

> **Status: all four fixes are implemented, verified, and committed.**
> MEDIUM-1 is no longer uncompiled: the `ast_dump.rs` refactor builds, passes
> clippy and the full suite, and its four known defects are fixed. Every guard
> was negative-tested by disabling it and confirming the abort returns.
>
> Verification run from a clean tree (`cargo clean`, 17.1 GiB removed):
> build 0, clippy `-D warnings` 0, **571 tests passing**, `cargo audit` clean,
> and every project script check green. Output is byte-identical to the
> pre-fix baseline across all 180 file/backend combinations.

## Files changed

| File | Change |
|---|---|
| `compiler/crates/poly-parser/src/depth_limit.rs` | **NEW** — iterative AST depth validator |
| `compiler/crates/poly-parser/src/ast_dump.rs` | **NEW** — linear AST printer for `--ast` |
| `compiler/crates/poly-parser/src/parser.rs` | operand-chain cap in precedence ladder; 19 fold sites |
| `compiler/crates/poly-parser/src/lib.rs` | module + re-export wiring |
| `compiler/crates/poly-transpiler/src/checker/type_checker.rs` | `MAX_CHECK_EXPRESSION_DEPTH` guard |
| `compiler/crates/poly-lsp/src/json.rs` | `MAX_JSON_DEPTH` guard |
| `compiler/crates/poly-lsp/src/main.rs` | `MAX_MESSAGE_BYTES` frame cap; `read_message` generic over `BufRead` |
| `compiler/crates/poly-cli/src/main.rs` | `--ast` streams via the new printer; bounded `rustfmt` subprocess |
| `compiler/scripts/build_playground_wasm.sh` | apply `wasm-opt -Oz` when available |
| `scripts/playground_wasm_smoke.mjs` | 2 CI regression cases for the depth fix |
| `playground/poly.wasm` | rebuilt (609,080 B) |

---

## HIGH-1 — stack overflow on deep expressions ✅

Root cause was **two** faults, not one:

1. `MAX_EXPRESSION_DEPTH = 32` bounds *syntactic* nesting only. A chain like
   `1 + 1 + … + 1` has no nested delimiters, so it parses at any length while the
   precedence ladder folds it into a tree one level deep per operand.
2. Three consumers then recurse over that tree: the type checker's
   `check_expression`, each backend, and **Rust's own `Drop` for `Box<Expression>`**.

**Rejecting the finished tree is not a fix** — you still have to drop it. The first
attempt did exactly that and the regression test still overflowed; that is how the
second cause was found.

Three layers:
- **Bound construction** (`parser.rs`): `MAX_OPERAND_CHAIN = 512`; all 19 mechanical
  fold sites route through a new `fold_binary` helper. The deep tree is never built.
- **Defense in depth** (`depth_limit.rs`): `ast_depth()` walks with an explicit heap
  worklist — a *recursive* validator would reintroduce the very overflow it prevents.
- **Backstop** (`type_checker.rs`): `MAX_CHECK_EXPRESSION_DEPTH = 512`, decrement
  paired in a `check_expression` wrapper so it is structurally guaranteed on every
  early return.

`MAX_AST_DEPTH` is derived as `MAX_OPERAND_CHAIN + 64` so the two caps cannot drift
and disagree — an earlier version did, rejecting exactly 512 operands.

Verified: PoC (5k/200k/1M operands) `exit 134 → 1`; boundary 512 passes / 513
rejected; real program still compiles and runs.

## HIGH-2 — LSP JSON stack overflow ✅

`poly-lsp/src/json.rs`: `MAX_JSON_DEPTH = 128` counter, paired in a `parse_value`
wrapper around `parse_value_inner`.

Verified: 200k-nested JSON returns a `-32700` diagnostic instead of aborting;
boundary 128 accepted / 129 rejected (the 2-level JSON-RPC envelope consumes 2).

## HIGH-3 — unbounded `Content-Length` allocation ✅

`poly-lsp/src/main.rs`: `MAX_MESSAGE_BYTES = 8 MiB`, checked **before** the
`vec![0u8; length]` allocation. Also generalized `read_message` to `BufRead` so the
framing and its limit are unit-testable.

The 8 MiB figure is **measured, not guessed**: a real session (initialize, didOpen,
documentSymbol, workspace/symbol) against the largest `.poly` in the tree peaks at a
21 KB frame, leaving roughly 400× headroom. It is deliberately generous — the job is
to refuse an absurd header, not to second-guess a large genuine document.

## MEDIUM-1 — `--ast` quadratic blowup ✅

Root cause was never really "cubic in the printer". Rust's alternate `Debug`
(`{:#?}`) indents every line by node depth, so a left-nested chain emitted
**quadratic** output:

~~~







n= 500  {:#?} ->  1,507,503 B,   1.5 s  |  {:?} ->  5,003 B,  61 µs







n=1000  {:#?} ->  6,015,003 B,  13.5 s  |  {:?} -> 10,003 B, 170 µs







n=2000  {:#?} -> 24,030,003 B, 137.5 s  |  {:?} -> 20,003 B, 399 µs







~~~

Fix: new `ast_dump.rs` — iterative printer, explicit heap worklist, indentation capped
at `MAX_INDENT_LEVELS = 32`. `--ast` streams to stdout instead of materializing a
String, and treats `BrokenPipe` (`| head`) as normal.

**Measured on the release binary:** 4113 ms → **9 ms** (≈450×), 3.1 MB → **141 KB**
(≈22× smaller), output now linear.

The four defects found in review (lifetime invariance on `&mut Vec<Step>`, four
`.expect()` calls under `expect_used = "deny"`, two stale `.iter().rev()` printing
tuples backwards, and a dropped `StructLiteral` name) are all fixed and now covered by
tests. The linearity test was negative-tested: restoring `{:#?}` makes it fail.

## Also fixed: rustfmt hang (found while measuring MEDIUM-1)

`--emit-rust` blocked indefinitely on long chains. The transpiler was never the
problem — gdb showed the process parked in `wait_with_output()` on a `rustfmt` child.
rustfmt backtracks pathologically on the fully-parenthesised code this backend emits
(rust-lang/rustfmt#5128; reproducible in a 3-line file at paren depth 88, and only in
a `let` binding — the same expression inside `println!` is fine).

Since formatting is documented as best-effort, `format_with_rustfmt` now enforces a
10 s deadline and falls back to unformatted output. n=88 went from >300 s to bounded,
and the result still compiles and runs correctly. The same function also leaked the
child on a stdin write failure and never drained its pipes; both are fixed.

## Verification

~~~sh







cd compiler







cargo build --release --workspace







cargo clippy --release --workspace --all-targets -- -D warnings







cargo test  --release --workspace -- --test-threads=1







cargo fmt --all -- --check







cargo audit







cd .. && python3 scripts/check_backends.py --poly-bin compiler/target/release/poly







node scripts/playground_wasm_smoke.mjs







~~~

Last full run from a clean tree: build 0 · clippy `-D warnings` 0 · **571 tests
passing, 0 failing** · fmt 0 · `cargo audit` no vulnerabilities · all script checks
green · wasm smoke `5 good + 6 rejected + 2 target`.

Behavioural equivalence was checked against the pre-fix baseline across all 180
file/backend combinations (stdout, stderr, and exit code compared separately):
**identical, 0 differences**. The 79 combinations that report errors are long-standing
backend coverage gaps (e.g. closures on the C backend), unchanged from baseline.

## Known limitations

- `MAX_MESSAGE_BYTES = 8 MiB` is generous on the evidence (400× headroom); a future
  change should revisit it if a real workflow ever exceeds it.
- All stack-overflow fixes bound recursion rather than relying on headroom, so they
  hold across platforms; exact abort thresholds would shift.
- The rustfmt timeout is a flat 10 s. Normal formatting finishes well under a second
  (worst observed real case: 116 ms), so the margin is generous rather than tuned.
- Windows and macOS CI jobs were not exercised; verification here was Linux-only.
