#!/usr/bin/env bash
# Build the Poly playground's WebAssembly transpiler.
#
# The playground loads the real Rust transpiler compiled to wasm32; this
# script produces playground/poly.wasm from the poly-wasm crate. Requires
# the wasm32-unknown-unknown target:
#
#   rustup target add wasm32-unknown-unknown
#
set -euo pipefail

cd "$(dirname "$0")/.."

# Size optimizations are scoped to this build via --config so the CLI and LSP
# keep default release settings. panic = "abort" drops the unwinding machinery
# (hundreds of KB of dead weight in the browser); errors are returned as Result
# values by the transpiler, so panics are true bugs and abort is fine.
# Note: --config requires Cargo 1.63+.
cargo build --locked -p poly-wasm --release --target wasm32-unknown-unknown \
  --config 'profile.release.opt-level="z"' \
  --config 'profile.release.lto=true' \
  --config 'profile.release.codegen-units=1' \
  --config 'profile.release.strip=true' \
  --config 'profile.release.panic="abort"'

SRC="target/wasm32-unknown-unknown/release/poly_wasm.wasm"
OUT="../playground/poly.wasm"

# Optional second pass. wasm-opt (binaryen) typically shaves a further 10-20% off
# a Rust-produced module, which matters because the browser downloads the whole
# file. It is genuinely optional: the build above already sets opt-level="z" and
# strips, so the module is usable without it. Skip rather than fail when the
# tool is absent -- on Arch that means `pacman -S binaryen`.
#
# -Oz is the size-optimizing pipeline. --enable-bulk-memory and --enable-
# nontrapping-float-to-int are on because the Rust/LLVM backend already emits
# those post-MVP opcodes, so leaving them off would fail validation here.
if command -v wasm-opt >/dev/null 2>&1; then
  optimize() {
    wasm-opt -Oz --converge --enable-bulk-memory --enable-nontrapping-float-to-int \
      --enable-sign-ext --enable-mutable-globals \
      "$1" -o "$2"
  }

  # --converge runs the -Oz pipeline until it reaches a fixed point inside one
  # invocation, which is worth ~450 B over a single -Oz pass. A second
  # invocation still finds a handful of bytes, so repeat while the module keeps
  # shrinking (bounded, so an oscillation cannot hang the build).
  optimize "$SRC" "$OUT"
  passes=1
  while [ "$passes" -lt 4 ]; do
    before=$(wc -c < "$OUT")
    optimize "$OUT" "$OUT.next"
    if [ "$(wc -c < "$OUT.next")" -lt "$before" ]; then
      mv "$OUT.next" "$OUT"
      passes=$((passes + 1))
    else
      rm -f "$OUT.next"
      break
    fi
  done
  echo "wasm-opt: applied (-Oz --converge, $passes pass(es))"
else
  cp "$SRC" "$OUT"
  echo "wasm-opt: not found, using cargo output as-is"
fi

echo "Built ../playground/poly.wasm ($(du -h "$OUT" | cut -f1))"
