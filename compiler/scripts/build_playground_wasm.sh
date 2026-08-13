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
cargo build -p poly-wasm --release --target wasm32-unknown-unknown \
  --config 'profile.release.opt-level="z"' \
  --config 'profile.release.lto=true' \
  --config 'profile.release.codegen-units=1' \
  --config 'profile.release.strip=true' \
  --config 'profile.release.panic="abort"'

cp "target/wasm32-unknown-unknown/release/poly_wasm.wasm" ../playground/poly.wasm

echo "Built ../playground/poly.wasm ($(du -h ../playground/poly.wasm | cut -f1))"
