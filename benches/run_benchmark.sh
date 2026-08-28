#!/usr/bin/env bash
# Benchmark: Poly asm vs C vs Rust backends
# Measures transpile+compile time and execution time for a simple loop program.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
POLY_SRC="$SCRIPT_DIR/asm_benchmark.poly"
POLY_CLI="$(cd "$SCRIPT_DIR/.." && pwd)/compiler/target/debug/poly"
TMPDIR_BENCH="$(mktemp -d)"
trap 'rm -rf "$TMPDIR_BENCH"' EXIT

if [ ! -f "$POLY_SRC" ]; then
    echo "Error: $POLY_SRC not found" >&2
    exit 1
fi

# Build the CLI if needed
if [ ! -x "$POLY_CLI" ]; then
    echo "Building poly CLI..."
    cargo build --manifest-path "$(cd "$SCRIPT_DIR/.." && pwd)/compiler/Cargo.toml" --bin poly 2>/dev/null
fi

ITERATIONS=${ITERATIONS:-5}

run_bench() {
    local label="$1"
    shift
    # Warm up
    "$@" >/dev/null 2>&1 || true
    # Time N iterations
    local total_ms=0
    for _ in $(seq 1 "$ITERATIONS"); do
        local t
        t=$( { time "$@" >/dev/null 2>&1; } 2>&1 | grep real | sed 's/[^0-9.]//g' )
        # Convert seconds to milliseconds (approximate)
        local ms
        ms=$(echo "$t * 1000" | bc 2>/dev/null || echo "0")
        total_ms=$(echo "$total_ms + $ms" | bc 2>/dev/null || echo "$total_ms")
    done
    local avg
    avg=$(echo "scale=1; $total_ms / $ITERATIONS" | bc 2>/dev/null || echo "$total_ms")
    echo "$label: avg ${avg}ms over $ITERATIONS runs"
}

echo "=== Poly Backend Benchmark ==="
echo "Source: $POLY_SRC"
echo "Iterations: $ITERATIONS"
echo ""

# --- Rust backend ---
echo "--- Rust backend ---"
RUST_DIR="$TMPDIR_BENCH/rust_out"
T_START=$(date +%s%N)
"$POLY_CLI" --target rust --emit-rust "$POLY_SRC" > "$RUST_DIR.rs" 2>/dev/null
T_END=$(date +%s%N)
RUST_TRANSPILE_MS=$(( (T_END - T_START) / 1000000 ))
rustc --edition 2021 -O -o "$TMPDIR_BENCH/rust_bench" "$RUST_DIR.rs" 2>/dev/null
run_bench "  Execution" "$TMPDIR_BENCH/rust_bench"
echo "  Transpile: ${RUST_TRANSPILE_MS}ms"

# --- C backend ---
echo ""
echo "--- C backend ---"
T_START=$(date +%s%N)
"$POLY_CLI" --target c --emit-c "$POLY_SRC" > "$TMPDIR_BENCH/c_bench.c" 2>/dev/null
T_END=$(date +%s%N)
C_TRANSPILE_MS=$(( (T_END - T_START) / 1000000 ))
cc -O2 -o "$TMPDIR_BENCH/c_bench" "$TMPDIR_BENCH/c_bench.c" 2>/dev/null
run_bench "  Execution" "$TMPDIR_BENCH/c_bench"
echo "  Transpile: ${C_TRANSPILE_MS}ms"

# --- Asm backend ---
echo ""
echo "--- Asm backend ---"
T_START=$(date +%s%N)
"$POLY_CLI" --target asm --emit-asm "$POLY_SRC" > "$TMPDIR_BENCH/asm_bench.S" 2>/dev/null
T_END=$(date +%s%N)
ASM_TRANSPILE_MS=$(( (T_END - T_START) / 1000000 ))
as --64 -o "$TMPDIR_BENCH/asm_bench.o" "$TMPDIR_BENCH/asm_bench.S" 2>/dev/null
ld -o "$TMPDIR_BENCH/asm_bench" "$TMPDIR_BENCH/asm_bench.o" 2>/dev/null
run_bench "  Execution" "$TMPDIR_BENCH/asm_bench"
echo "  Transpile: ${ASM_TRANSPILE_MS}ms"

echo ""
echo "=== Done ==="
