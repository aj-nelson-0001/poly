# Poly JavaScript Backend Design

**Status:** Proposal for review — not yet implemented. Design review updated
2026-09-09: the integration touchpoints below are verified against the
codebase, and the checker's strict foreign-call mode (`--strict`) applies to
this target automatically.

This document specifies a fourth Poly compilation target: JavaScript, runnable
both under Node.js and in the browser (the playground already ships a
`poly.wasm` build, so a JS target closes the loop: transpile in-browser,
execute in-browser). It follows the same target model as the C11 backend —
Poly lowers its orchestration subset to plain JavaScript, and complex
declarations stay in `#js` foreign blocks emitted verbatim.

## Goals

1. **Browser + Node parity.** One generated `.js` artifact that runs under
   Node (`node out.js`) and in the browser (module or plain script), with the
   same I/O semantics Poly programs already express.
2. **Same target model as C.** Poly orchestration statements → generated JS;
   `#js ... #endjs` blocks copied verbatim at module scope; `extern js fn`
   declarations are checker-only contracts (never emitted), exactly like
   `extern c fn`.
3. **Reuse the shared pipeline.** Lexer → parser → `select_target` → checker →
   codegen, so the JS backend gets type checking, error recovery, and target
   filtering for free.
4. **No runtime dependency.** Generated code uses only ECMAScript 2020
   features available in every evergreen browser and Node ≥ 14 — no npm
   packages, no bundler required.

## Non-goals

- **Not a WASM compiler.** The existing `poly-wasm` crate already embeds the
  whole transpiler in the browser; the JS target generates a JS *artifact*
  from Poly source. (A `--target wasm` that compiles through the IR to actual
  wasm bytecode is a separate, much larger effort.)
- **No DOM APIs.** Poly programs are orchestration + console I/O; browser
  output goes through `console.log`, not DOM manipulation.
- **No async translation.** Like the C backend, Poly `async` functions are
  rejected for this target (JS's own `async` in `#js` blocks is unaffected).

## Target Model

~~~poly fragment
#js
function doubleValue(x) {
    return x * 2;
}
#endjs

var answer i32 := doubleValue(21)
put answer
~~~

CLI surface (mirrors the C target):

~~~bash
poly --target js --check program.poly
poly --target js --emit-js program.poly
poly --target js program.poly          # node out.js
~~~

Foreign-block selection: only `#js` blocks are emitted for the JS target;
`#rust`/`#c`/`#asm` blocks are dropped. `#cpp` stays rejected globally.
`extern js fn ...` declarations are validated by the checker and never
emitted.

## Verified Integration Touchpoints

Each place the JS backend must touch, confirmed against the current code:

| Touchpoint | File | Current state | Required change |
|---|---|---|---|
| Foreign-block lexer | `poly-lexer/src/lexer.rs` (`foreign_block_language_at_current`) | Matches the list `["rust", "cpp", "c", "asm"]` by prefix | Add `"js"` to the list |
| `extern` target validation | `poly-parser/src/parser.rs` (`parse_extern_function_declaration`) | Accepts `rust`\|`c`\|`asm`, otherwise errors | Add `"js"` and extend the error message |
| Target filtering | `poly-transpiler/src/codegen.rs` (`select_target`) | Rejects `#cpp`, keeps blocks where `declaration.target == language` | No change — `js` flows through |
| Target dispatch | `poly-transpiler/src/codegen.rs` (`transpile_target`) | Maps `rust`/`c`/`asm` names | Add `"js"` → `transpile_js_checked` |
| Checker | `poly-transpiler/src/checker.rs` | Extern declarations for any target register via `register_extern_functions`; non-selected targets are filtered before checking | No change — see strict-mode note below |
| Workspace | `compiler/Cargo.toml` | Members list + `workspace.dependencies` | Add `crates/poly-js-codegen` |
| CLI target enum | `poly-cli/src/main.rs` (`Target::from_str`, `language_name`, extension) | `rust`/`c`/`asm`/`s`/`S` | Add `js`\|`mjs` → `Target::Js` |
| CLI validation | `poly-cli/src/main.rs` (`verify_*_compiles`) | `rustc`/`cc`/assembler checks, `POLY_CC` override | Add `verify_js_compiles` via `node --check` (`POLY_NODE` override; Node-absent fallback matches the C target) |
| CLI emission | `poly-cli/src/main.rs` (`--emit-*` arms, build path, `--project`) | `--emit-rust`/`--emit-c`/`--emit-asm` | Add `--emit-js` arm, JS build path (`node out.js` is a run step, so build = write + syntax check), and `generate_js_project` |
| CI | `.github/workflows/ci.yml` | C/asm fixture jobs | Add a `js` fixture check/execute step (Node is preinstalled on all runners) |

Checker strict mode note: the checker registers foreign function names with a
line heuristic that already handles JS `function name(...)` declarations (the
generic "contains `(`" branch strips `function `), so opaque `#js` calls
resolve the same way C calls do. Arrow-function constants and class methods do
**not** register — the first cut requires top-level `function` declarations in
`#js` blocks, documented like C's prototype expectations. The new
`--strict` mode (`check_program_strict_foreign_with_warnings`, added
2026-09-09) applies to this target without further work; the CLI should enable
it by default for `--target js` because the browser has no native compiler to
fall back on.

## Execution

- **Node:** `node program.js`. Generated file is CommonJS-free plain script
  (no `require`), so it also loads as a browser `<script>` or via
  `import` from a module shim.
- **Browser:** the playground compiles Poly → JS in-browser via the existing
  wasm build, then evaluates the artifact in a sandboxed context, capturing
  `console.*` for display.
- **Native validation (`--check`/build):** `node --check` performs a syntax
  check; if Node is absent, the CLI falls back to accepting generated output
  without validation (matching how the C target treats a missing compiler).
  `POLY_NODE` overrides the Node binary, like `POLY_CC`.

## Type Mapping

| Poly | JavaScript | Notes |
|---|---|---|
| `bool` | `boolean` | |
| `i8`…`i64`, `u8`…`u32` | `number` | Exact range below 2^53. |
| `u64` | `bigint` | Emitted as `123n` literals; arithmetic mixes guarded. |
| `f32`/`f64` | `number` | |
| `char` | `string` (length-1) | |
| `string`/`ustring` | `string` | UTF-16 native; no pointer semantics. |
| struct | plain object literal | `{ name: value, ... }`; field access via `.field`. |
| enum | frozen object of variants | String-backed; `match` compiles to `switch`. |

Integer division/modulo semantics: Poly's `/` and `%` on integers map to
`Math.trunc(a / b)` and `a % b` to keep truncation (JS `/` is float division).
`==`/`!=` compile to `===`/`!==`.

## Supported Poly Surface (parity with C target)

| Construct | JS target | Notes |
|---|---:|---|
| Scalar declarations, `let`, `const` | Yes | `let`/`const`. |
| Arithmetic/comparison/logical/bitwise | Yes | Integer `/`,`%` guarded as above. |
| Strings and concatenation | Yes | Native — no C-style flattening limits. |
| `put`, `error`, `warn`, `info` | Yes | `console.log/error/warn/info`. |
| `if`/`else`, `while`, loops, `break`/`continue` | Yes | |
| Inclusive numeric loops | Yes | `for (let i = a; i <= b; i++)`. |
| Multi-range loops | Yes | Nested loops, unlike C. |
| Collection loops | Yes | `for..of` over arrays. |
| Simple functions | Yes | Non-async, non-generic → `function`. |
| Structs | Plain only | Object literals; no methods/generics. |
| Enums, traits, impls, modules, aliases | No | Use a `#js` helper or Rust target. |
| Closures / higher-order ops | No | Rejected, as in C. |
| File redirects, stdin, typed input flags | No | Browser-hostile; call a `#js` helper (Node target could add later). |
| Async Poly functions | No | Rejected like C. |

## Output Shape

~~~js
/* Generated from Poly source code. */
function doubleValue(x) {          // from #js block, verbatim
    return x * 2;
}

function main() {                  // Poly orchestration, in order
  const answer = doubleValue(21);
  console.log(answer);
}
main();
~~~

Top-level orchestration statements run inside a `main()` invocation so `const`
re-declaration is safe when the artifact is imported twice.

## Implementation Plan

1. **`compiler/crates/poly-js-codegen`** — new crate mirroring
   `poly-c-codegen`'s structure (`JsGenerator`, `transpile(&Program)`),
   reusing its statement/expression walker shape. ~600–800 lines.
2. **Target plumbing** — add `"js"` to the lexer's foreign-language list and
   the parser's `extern` target validation (see the touchpoints table);
   `transpile_target` gains `"js"`; `poly-cli` gains `Target::Js`, `--emit-js`,
   and Node-based `--check`/build steps (`POLY_NODE` override).
3. **Checker** — no new rules: target-aware extern filtering already works for
   any target name, and `--strict` foreign-call checking applies automatically.
   Async/generic rejection mirrors the C backend's errors.
4. **Tests** — unit tests in the crate (mirroring `poly-asm-codegen`'s),
   snapshot tests added to `snapshot_tests.rs` (`*_js.snap`), plus a
   `tests/js_target_tests.poly` fixture with `#js` blocks and a CI step that
   executes it with Node.
5. **Docs** — new `POLY_JS_BLOCKS.md` (modeled on `POLY_C_BLOCKS.md`);
   `POLY_V2_SUPPORT_MATRIX.md` gains a JS column; README/CHANGELOG updated.
6. **Playground** — `poly-wasm`'s `transpile` export already runs the checked
   pipeline; expose the JS target there so the playground can compile and run
   Poly in-browser end-to-end.

### Acceptance Criteria

- `poly --target js --emit-js program.poly` prints valid ES2020 for every
  construct listed as supported above, matching the C table where applicable.
- `poly --target js --check program.poly` passes with Node present and still
  parses/checks with Node absent (warning, matching the C target's fallback).
- `tests/js_target_tests.poly` executes under Node with verified output in CI.
- All existing targets' fixtures and snapshots are unchanged (no regressions
  from the lexer/parser list extensions — `js` cannot collide with any current
  Poly keyword or existing marker).

## Risks / Open Questions

- **u64/bigint mixing** (`bigint` + `number` throws in JS): the first cut can
  reject mixed-width arithmetic and require a `#js` helper, matching the C
  backend's "keep it in a helper" philosophy.
- **`get`/stdin in Node:** could be supported via `readline` under a
  `--target js --node` build; deferred to keep browser parity clean.
- **Float formatting:** Poly `put 3.14` prints `3.14` in C/Rust; JS prints
  identically for most values, but large floats may differ — snapshot tests
  will pin the generated code (not runtime output), so this is a runtime
  documentation note, not a codegen problem.
