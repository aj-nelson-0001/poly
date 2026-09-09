#!/usr/bin/env node
// Smoke test for the playground WASM artifact (playground/poly.wasm).
//
// Instantiates the module exactly as playground/index.html does and checks:
//   - keyword operator programs transpile to the expected Rust symbols
//   - retired symbol programs are rejected with migration diagnostics
//   - transpile_target emits C and JavaScript for the selected backend
//
// Runs in CI (see the `wasm` job in .github/workflows/ci.yml) and locally:
//   node scripts/playground_wasm_smoke.mjs
import { readFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const wasmPath = join(root, 'playground', 'poly.wasm');

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
  ['fn main()\n    put 5 mod 3\n    put 6 xor 3\nend fn\n', ['%', '^']],
  ['fn main()\n    put true and false\n    put not false\nend fn\n', ['&&', '!']],
  ['fn main()\n    put 12 bitand 10\n    put 12 bitor 10\nend fn\n', ['&', '|']],
  ['fn main()\n    put 1 shift left 2\n    put 8 shift right 2\nend fn\n', ['<<', '>>']],
  ['fn main()\n    if 5 = 5,\n        put "eq"\n    end if\nend fn\n', ['==']],
];

const bad = [
  'fn main()\n    put 5 % 3\nend fn\n',   // -> mod
  'fn main()\n    put a && b\nend fn\n',  // -> and
  'fn main()\n    put 6 ^ 3\nend fn\n',   // -> xor
  'fn main()\n    put !true\nend fn\n',   // -> not
  'fn main()\n    put a | b\nend fn\n',   // -> bitor
  'fn main()\n    put 5 == 3\nend fn\n',  // -> =
];

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

if (failures > 0) {
  console.error(`playground wasm smoke test: ${failures} failure(s)`);
  process.exit(1);
}
console.log(
  `playground wasm smoke test: ${good.length} good + ${bad.length} rejected + ${targetCases.length} target cases pass`
);
