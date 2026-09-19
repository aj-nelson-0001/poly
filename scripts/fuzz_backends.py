#!/usr/bin/env python3
"""Fuzz backend parity: generate random (deterministic) Poly programs and run
them through every target, failing when the outputs diverge.

Unlike check_backends.py, which runs curated suites, this generator produces
novel programs so backend divergences surface before anyone hand-writes a
test for them. Programs are pure integer computations — no string arena,
no division that can trap, every `mod`/`div` right operand normalized to a
positive nonzero value — so a divergence means a real backend bug, not an
environment difference.

Usage: scripts/fuzz_backends.py [--seeds N] [--start N] [--poly-bin PATH]

Each seed is a complete program: a handful of arithmetic helper functions
plus a main that evaluates them over a fixed lattice of inputs and prints
the results. Determinism holds across targets because every operation is
32-bit integer arithmetic with the same Poly-level semantics.
"""
import argparse
import random
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(REPO_ROOT / "scripts"))
from check_backends import run_target  # noqa: E402

# Operator pool: all four backends share these exact semantics for i32.
ARITH = ["+", "-", "*", "mod", "/"]
CMP = ["<", ">", "<=", ">=", "="]
CONSTS = [0, 1, 2, 3, 5, 7, 13, 97, 1000, 10007, 65536, 0 - 1, 0 - 7, 0 - 100]

# A structured expression generator: depth-bounded, with `div`/`mod` right
# operands wrapped so they are always positive and nonzero (no traps, no
# target-specific SIGFPE divergence).


def gen_expr(rng: random.Random, vars: list[str], depth: int, calls: list | None = None) -> str:
    calls = calls if calls is not None else []
    if depth <= 0 or rng.random() < 0.3:
        if vars and rng.random() < 0.6:
            return rng.choice(vars)
        return str(rng.choice(CONSTS))
    # Calls to earlier-defined functions (DAG: they never call forward or
    # self, so termination holds even with nested call args). This is the
    # shape class that caught the asm shared call-result slot: two calls in
    # one expression must not alias.
    if calls and rng.random() < 0.15 and depth >= 2:
        name, arity = rng.choice(calls)
        args = ", ".join(gen_expr(rng, vars, depth - 1, calls) for _ in range(arity))
        return f"{name}({args})"
    kind = rng.random()
    if kind < 0.75:  # arithmetic
        op = rng.choice(ARITH)
        lhs = gen_expr(rng, vars, depth - 1)
        rhs = gen_expr(rng, vars, depth - 1)
        if op in ("/", "mod"):
            # ((rhs) mod 7) + 7) mod 7 + 1 keeps the divisor positive and nonzero.
            rhs = f"((( {rhs}) mod 7) + 7) mod 7 + 1"
        elif op == "*":
            # Bound both operands so products never overflow i32 — overflow
            # semantics are target-defined (see AUDIT_REPORT.md) and would
            # pollute the parity signal.
            lhs = f"((( {lhs}) mod 101) + 101) mod 101"
            rhs = f"((( {rhs}) mod 101) + 101) mod 101"
        return f"({lhs} {op} {rhs})"
    if kind < 0.9:  # unary minus via unsigned literal arithmetic
        return f"(0 - ({gen_expr(rng, vars, depth - 1)}))"
    # shift-family (parity guard: all four targets must agree)
    op = rng.choice(["shift left", "shift right"])
    operand = gen_expr(rng, vars, depth - 1)
    if op == "shift left":
        # Keep shifted values inside i32: |operand| < 10^6, shift <= 3.
        operand = f"((( {operand}) mod 1000001) + 1000001) mod 1000001"
    return f"({operand} {op} {rng.choice([0, 1, 2, 3])})"


def gen_fn(rng: random.Random, name: str, params: list[str], calls: list | None = None) -> str:
    calls = calls if calls is not None else []
    lines = [f"fn {name}({', '.join(f'{p}: i32' for p in params)}): i32"]
    vars = list(params)
    # a few derived locals, each consuming the ones before it
    for i in range(rng.randint(1, 3)):
        var = f"v{i}"
        expr = gen_expr(rng, vars, rng.randint(2, 4), calls)
        lines.append(f"    var {var} i32 := {expr}")
        vars.append(var)
    # 0-2 if/else statements that mutate the accumulated variables (plain
    # statement grammar — no inline if-expressions)
    for i in range(rng.randint(0, 2)):
        target = rng.choice(vars)
        cond = f"{gen_expr(rng, vars, 1)} < {gen_expr(rng, vars, 1)}"
        lines.append(f"    if {cond}")
        lines.append(f"        {target} := {gen_expr(rng, vars, 2)}")
        lines.append("    else,")
        lines.append(f"        {target} := {gen_expr(rng, vars, 2)}")
        lines.append("    end if")
    # an if/else over the accumulated variables
    cond = f"{gen_expr(rng, vars, 2, calls)} < {gen_expr(rng, vars, 2, calls)}"
    lines.append(f"    if {cond}")
    lines.append(f"        return {gen_expr(rng, vars, 3, calls)}")
    lines.append("    else,")
    lines.append(f"        return {gen_expr(rng, vars, 3, calls)}")
    lines.append("    end if")
    lines.append("end fn")
    return "\n".join(lines)


def gen_program_v2(seed: int) -> str:
    """Generate one complete program: arithmetic helper fns + call lattice."""
    rng = random.Random(seed)
    defined: list[tuple[str, int]] = []
    bodies: list[str] = []
    for i in range(rng.randint(2, 4)):
        arity = rng.randint(1, 3)
        params = [f"p{j}" for j in range(arity)]
        bodies.append(gen_fn(rng, f"f{i}", params, list(defined)))
        defined.append((f"f{i}", arity))
    main = ["fn main()"]
    inputs = [0, 1, 2, 3, 7, 13, 97, 0 - 5, 0 - 13, 1000]
    for name, arity in defined:
        for _ in range(rng.randint(1, 2)):
            args = ", ".join(str(rng.choice(inputs)) for _ in range(arity))
            main.append(f"    put {name}({args})")
    # Two calls in one expression: pins the fresh-temp-per-call lowering on
    # every target (the asm backend once shared one result slot per callee).
    if len(defined) >= 2:
        (n0, a0), (n1, a1) = defined[0], defined[1]
        for _ in range(rng.randint(1, 2)):
            args0 = ", ".join(str(rng.choice(inputs)) for _ in range(a0))
            args1 = ", ".join(str(rng.choice(inputs)) for _ in range(a1))
            main.append(f"    put {n0}({args0}) + {n1}({args1})")
    main.append("end fn")
    return "\n\n".join(bodies) + "\n\n" + "\n".join(main) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--seeds", type=int, default=25)
    parser.add_argument("--start", type=int, default=1)
    parser.add_argument("--poly-bin", default=None)
    args = parser.parse_args()

    poly_bin = (
        Path(args.poly_bin)
        if args.poly_bin
        else REPO_ROOT / "compiler" / "target" / "release" / "poly"
    )
    if not poly_bin.exists():
        raise SystemExit(f"poly binary not found at {poly_bin} (build it first)")

    targets = ["rust", "c", "js"]
    if shutil.which("as") and sys.platform == "linux":
        targets.append("asm")

    divergences = 0
    invalid = 0
    for seed in range(args.start, args.start + args.seeds):
        source = gen_program_v2(seed)
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / f"fuzz_{seed}.poly"
            path.write_text(source)
            outputs: dict[str, list[str] | str] = {}
            for target in targets:
                try:
                    got, _code, _err = run_target(poly_bin, path, target, Path(tmp))
                    outputs[target] = got
                except (subprocess.CalledProcessError, SystemExit) as exc:
                    outputs[target] = f"ERROR: {exc}"
                except subprocess.TimeoutExpired:
                    outputs[target] = "ERROR: timeout"
            reference = outputs.get("rust")
            if not isinstance(reference, list):
                print(f"seed {seed}: rust target failed:\n{reference}")
                invalid += 1
                continue
            mismatched = {
                t: o for t, o in outputs.items() if isinstance(o, list) and o != reference
            }
            errored = {t: o for t, o in outputs.items() if isinstance(o, str) and o != reference}
            if mismatched or errored:
                divergences += 1
                print(f"DIVERGENCE seed {seed}:")
                print(f"  rust: {reference}")
                for t, o in {**mismatched, **errored}.items():
                    print(f"  {t}: {o}")
                print(f"  program kept at /tmp/fuzz_divergence_{seed}.poly")
                Path(f"/tmp/fuzz_divergence_{seed}.poly").write_text(source)
            else:
                print(f"ok   seed {seed} ({len(reference)} lines, {len(targets)} targets)")
    if divergences or invalid:
        print(f"\n{divergences} divergence(s), {invalid} invalid program(s)")
        return 1
    print(f"\nAll {args.seeds} fuzz programs agree on every target.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
