#!/usr/bin/env node
// Smoke test for the playground WASM artifact (playground/poly.wasm).
//
// Instantiates the module exactly as playground/index.html does and checks:
//   - keyword operator programs transpile to the expected Rust symbols
//   - retired symbol programs are rejected with migration diagnostics
//   - over-deep operator chains are rejected instead of overflowing the stack
//   - transpile_target emits C and JavaScript for the selected backend
//   - the module, the playground selector, and the release notes agree about
//     the asm target (compiled out here, documented as absent)
//
// Runs in CI (see the `wasm` job in .github/workflows/ci.yml) and locally:
//   node scripts/playground_wasm_smoke.mjs [path-to-poly.wasm]
//
// The optional path defaults to playground/poly.wasm. ci.yml's
// `wasm-asm-guard` job points it at a deliberately asm-enabled build and
// requires this script to fail, which proves the parity check below fires on a
// real runner rather than only under a hand edit.
import { readFileSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const wasmPath = process.argv[2]
  ? resolve(process.argv[2])
  : join(root, 'playground', 'poly.wasm');

const { instance } = await WebAssembly.instantiate(readFileSync(wasmPath), {});
const exp = instance.exports;
const encoder = new TextEncoder();
const decoder = new TextDecoder();

function transpile(src) {
  const bytes = encoder.encode(src);
  const ptr = exp.poly_alloc(bytes.length);
  new Uint8Array(exp.memory.buffer, ptr, bytes.length).set(bytes);
  const result = exp.transpile(ptr, bytes.length);
  exp.poly_free(ptr, bytes.length);
  const out = decoder.decode(
    new Uint8Array(exp.memory.buffer, exp.result_ptr(result), exp.result_len(result))
  );
  const isError = exp.result_is_error(result) !== 0;
  exp.free_result(result);
  return { out, isError };
}

function transpileTarget(src, target) {
  const bytes = encoder.encode(src);
  const ptr = exp.poly_alloc(bytes.length);
  new Uint8Array(exp.memory.buffer, ptr, bytes.length).set(bytes);
  const tgt = encoder.encode(target);
  const tgtPtr = exp.poly_alloc(tgt.length);
  new Uint8Array(exp.memory.buffer, tgtPtr, tgt.length).set(tgt);
  const result = exp.transpile_target(ptr, bytes.length, tgtPtr, tgt.length);
  exp.poly_free(ptr, bytes.length);
  exp.poly_free(tgtPtr, tgt.length);
  const out = decoder.decode(
    new Uint8Array(exp.memory.buffer, exp.result_ptr(result), exp.result_len(result))
  );
  const isError = exp.result_is_error(result) !== 0;
  exp.free_result(result);
  return { out, isError };
}

const good = [
  ['fn main()\n    put 5 mod 3\n    put 6 xor 3\nend fn\n', ['wrapping_rem', '^']],
  // `^` is the exponentiation operator (commit 9e3a752), not legacy xor:
  // integer power lowers to wrapping_pow, so it must be accepted.
  ['fn main()\n    put 6 ^ 3\nend fn\n', ['wrapping_pow']],
  ['fn main()\n    put true and false\n    put not false\nend fn\n', ['&&', '!']],
  ['fn main()\n    put 12 bitand 10\n    put 12 bitor 10\nend fn\n', ['&', '|']],
  // Shifts lower to wrapping_shl/shr (defined 32-bit wrap semantics), so
  // assert the method names rather than raw operator tokens.
  ['fn main()\n    put 1 shift left 2\n    put 8 shift right 2\nend fn\n', ['wrapping_shl', 'wrapping_shr']],
  ['fn main()\n    if 5 = 5,\n        put "eq"\n    end if\nend fn\n', ['==']],
];

const bad = [
  'fn main()\n    put 5 % 3\nend fn\n',   // -> mod
  'fn main()\n    put a && b\nend fn\n',  // -> and
  'fn main()\n    put !true\nend fn\n',   // -> not
  'fn main()\n    put a | b\nend fn\n',   // -> bitor
  'fn main()\n    put 5 == 3\nend fn\n',  // -> =
];

// Regression: a long left-associative chain (`1 + 1 + ... + 1`) has no nested
// delimiters, so it once parsed at any length and built a tree one level deep
// per operand. Every recursive walk over that tree -- the type checker, the
// backends, and Rust's own `Drop` -- then exhausted the WASM stack and killed
// the browser tab. The parser now refuses the chain while folding it, so the
// playground must report a diagnostic instead of trapping.
const longChain =
  `fn main()\n    var x i32 := ${Array(200_000).fill('1').join(' + ')}\n    put x\nend fn\n`;

let failures = 0;
for (const [src, markers] of good) {
  const { out, isError } = transpile(src);
  const missing = markers.filter((m) => !out.includes(m));
  if (isError || missing.length > 0) {
    failures += 1;
    console.error(`GOOD case rejected or missing ${JSON.stringify(missing)}:\n${out}`);
  }
}
for (const src of bad) {
  const { isError, out } = transpile(src);
  if (!isError) {
    failures += 1;
    console.error(`BAD case was accepted (legacy syntax not rejected):\n${src}\n${out}`);
  }
}

// The over-deep chain must come back as a diagnostic, not trap the module.
// A trap here means the stack overflow is back, so the check is the call
// returning at all.
{
  let trapped = null;
  let outcome = null;
  try {
    outcome = transpile(longChain);
  } catch (error) {
    trapped = error;
  }
  if (trapped) {
    failures += 1;
    console.error(`OVER-DEEP chain trapped the module: ${trapped.message}`);
  } else if (!outcome.isError) {
    failures += 1;
    console.error('OVER-DEEP chain was accepted (depth limit not enforced)');
  } else if (!outcome.out.includes('operator chain')) {
    failures += 1;
    console.error(`OVER-DEEP chain rejected with an unexpected diagnostic:\n${outcome.out}`);
  }
}

// A chain well inside the cap must still transpile, so the depth limit cannot
// silently regress into rejecting legitimate programs.
{
  const moderate = `fn main()\n    var x i32 := ${Array(200).fill('1').join(' + ')}\n    put x\nend fn\n`;
  const { out, isError } = transpile(moderate);
  // Match the declaration as the Rust backend actually emits it (`let mut x`).
  if (isError || !/let (mut )?x\b/.test(out)) {
    failures += 1;
    console.error(`MODERATE chain rejected (depth limit too aggressive):\n${out}`);
  }
}

if (failures > 0) {
  console.error(`playground wasm smoke test: ${failures} failure(s)`);
  process.exit(1);
}

// Target-aware pipeline checks (the playground target selector uses these).
const targetCases = [
  ['c', ['int main', 'printf']],
  ['js', ['function main', 'console.log']],
];
for (const [target, markers] of targetCases) {
  const { out, isError } = transpileTarget('fn main()\n    put 42\nend fn\n', target);
  const missing = markers.filter((m) => !out.includes(m));
  if (isError || missing.length > 0) {
    failures += 1;
    console.error(`TARGET ${target} rejected or missing ${JSON.stringify(missing)}:\n${out}`);
  }
}

// The module, the playground selector, and the release notes must agree about
// the asm target. preview.20 compiled that backend out of this module (~88 KB)
// because the page cannot run x86-64 assembly; if it comes back -- a dropped
// `default-features = false` or a flipped feature -- CI has to stop until the
// selector offers the target and the notes say the browser module carries it.
// Otherwise the download grows silently and the documentation keeps describing
// a module nobody rebuilds.
{
  const html = readFileSync(join(root, 'playground', 'index.html'), 'utf8');
  const uiTargets = [...html.matchAll(/<option value="([a-z]+)"/g)].map((m) => m[1]);
  // An unknown target's diagnostic lists the set this build can reach, so one
  // probe reports the compiled feature set without guessing at each backend.
  const probe = transpileTarget('fn main()\n    put 1\nend fn\n', 'not-a-target');
  const advertised = /Supported targets: ([a-z, ]+)/.exec(probe.out);
  const notes = readFileSync(join(root, 'POLY_V2_PREVIEW_RELEASE_NOTES.md'), 'utf8');
  const asmRow = notes.split('\n').find((line) => line.startsWith('| Asm '));
  const sorted = (targets) => [...targets].sort().join(', ');

  if (!advertised) {
    failures += 1;
    console.error(`TARGET probe did not advertise its supported targets:\n${probe.out}`);
  } else if (!asmRow) {
    failures += 1;
    console.error(
      'NOTES: POLY_V2_PREVIEW_RELEASE_NOTES.md has no `| Asm ...` Target Contract row, ' +
        "so the browser module's asm status is undocumented"
    );
  } else {
    const moduleTargets = advertised[1].split(',').map((t) => t.trim());
    const moduleHasAsm = moduleTargets.includes('asm');
    if (sorted(uiTargets) !== sorted(moduleTargets)) {
      failures += 1;
      console.error(
        `TARGET parity: the wasm module supports [${sorted(moduleTargets)}] but ` +
          `playground/index.html offers [${sorted(uiTargets)}]; update the selector ` +
          '(and, for asm, the release notes) or build the extra target back out'
      );
    }
    const notesSayAbsent = asmRow.includes('absent from the browser module');
    if (moduleHasAsm === notesSayAbsent) {
      failures += 1;
      console.error(
        moduleHasAsm
          ? 'NOTES: the wasm module now compiles asm, but the Target Contract row still says it ' +
              'is absent from the browser module; document the browser target (and its size budget)'
          : 'NOTES: the Target Contract row no longer says asm is absent from the browser module, ' +
              'but the wasm module does not compile it; correct the row'
      );
    }
  }
}

if (failures > 0) {
  console.error(`playground wasm smoke test: ${failures} failure(s)`);
  process.exit(1);
}
console.log(
  `playground wasm smoke test: ${good.length} good + ${bad.length} rejected + ` +
    `${targetCases.length} target cases + module/UI/notes parity pass`
);
