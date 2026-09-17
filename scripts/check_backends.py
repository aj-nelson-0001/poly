#!/usr/bin/env python3
"""Differential backend check: run one Poly program through every target and
fail when the outputs diverge.

This is the guard that caught the asm negative-integer printing bug, the asm
shared-temp-slot corruption, the C/JS missing match-expression support, the C
int32_t-default inference for struct-returning calls, and the asm
struct-call-argument segfault. Any backend behavior change must keep all
targets agreeing here.

Usage: check_backends.py [--poly-bin PATH]

Each `tests/diff_*.poly` program carries its expected output in a header
block: lines between the `EXPECTED:` marker and the end of the comment
region, one line of expected stdout per program output line.
"""
import argparse
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
TESTS_DIR = REPO_ROOT / "tests"


def expected_from_source(source: str) -> list[str]:
    """Extract the EXPECTED block from a differential test program."""
    lines = source.splitlines()
    try:
        marker = next(i for i, line in enumerate(lines) if line.startswith("# EXPECTED:"))
    except StopIteration:
        raise SystemExit(f"missing '# EXPECTED:' block in a diff_*.poly program")
    expected = []
    for line in lines[marker + 1 :]:
        if not line.startswith("# "):
            break
        text = line[2:]
        # Values are joined with " / ". Leading/trailing spaces in a value
        # are significant (they test whitespace preservation), so a value
        # that starts or ends with a space is written with two spaces next
        # to the separator: "... / diff /  ok / ..." means the output line
        # is " ok".
        expected.extend(part for part in text.split(" / ") if part.strip())
    return expected


def run_target(poly_bin: Path, source: Path, target: str, workdir: Path) -> list[str]:
    if target == "rust":
        project = workdir / "proj_rust"
        subprocess.run(
            [str(poly_bin), "--project", str(project), str(source)],
            check=True,
            capture_output=True,
            text=True,
        )
        subprocess.run(
            ["cargo", "build", "--release", "-q"],
            cwd=project,
            check=True,
            capture_output=True,
            text=True,
        )
        candidates = sorted(
            p
            for p in (project / "target/release").glob("*")
            if p.is_file()
            and not p.name.startswith(".")
            and p.suffix not in {".d", ".rlib"}
            and os.access(p, os.X_OK)
        )
        binary = candidates[0]
        result = subprocess.run([str(binary)], capture_output=True, text=True)
    elif target == "c":
        project = workdir / "proj_c"
        subprocess.run(
            [str(poly_bin), "--project", str(project), str(source), "--target", "c"],
            check=True,
            capture_output=True,
            text=True,
        )
        binary = workdir / "bin_c"
        subprocess.run(
            ["gcc", "-o", str(binary), str(project / "main.c")],
            check=True,
            capture_output=True,
            text=True,
        )
        result = subprocess.run([str(binary)], capture_output=True, text=True)
    elif target == "js":
        project = workdir / "proj_js"
        subprocess.run(
            [str(poly_bin), "--project", str(project), str(source), "--target", "js"],
            check=True,
            capture_output=True,
            text=True,
        )
        result = subprocess.run(
            ["node", str(project / "main.js")], capture_output=True, text=True
        )
    elif target == "asm":
        asm_path = workdir / "out.S"
        subprocess.run(
            [str(poly_bin), str(source), "--target", "asm", "-o", str(asm_path)],
            check=True,
            capture_output=True,
            text=True,
        )
        obj_path = workdir / "out.o"
        binary = workdir / "bin_asm"
        subprocess.run(["as", "-o", str(obj_path), str(asm_path)], check=True, capture_output=True, text=True)
        subprocess.run(["ld", "-o", str(binary), str(obj_path)], check=True, capture_output=True, text=True)
        result = subprocess.run([str(binary)], capture_output=True, text=True)
    else:
        raise SystemExit(f"unknown target: {target}")
    if result.returncode != 0:
        raise SystemExit(
            f"{source.name} on target {target} exited {result.returncode}:\n{result.stderr}"
        )
    return result.stdout.splitlines()


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--poly-bin", default=None)
    args = parser.parse_args()

    poly_bin = (
        Path(args.poly_bin)
        if args.poly_bin
        else REPO_ROOT / "compiler" / "target" / "release" / "poly"
    )
    if not poly_bin.exists():
        raise SystemExit(f"poly binary not found at {poly_bin} (build it first)")

    # asm emits Linux x86-64 syscalls; skip where it cannot assemble.
    targets = ["rust", "c", "js"]
    if shutil.which("as") and sys.platform == "linux":
        targets.append("asm")

    failures = 0
    for source in sorted(TESTS_DIR.glob("diff_*.poly")):
        expected = expected_from_source(source.read_text())
        with tempfile.TemporaryDirectory() as tmp:
            workdir = Path(tmp)
            for target in targets:
                got = run_target(poly_bin, source, target, workdir)
                if got != expected:
                    failures += 1
                    print(f"FAIL {source.name} [{target}]")
                    print(f"  expected: {expected}")
                    print(f"  got:      {got}")
                else:
                    print(f"ok   {source.name} [{target}] ({len(expected)} lines)")
    if failures:
        print(f"\n{failures} divergence(s) across the differential suite")
        sys.exit(1)
    print("\nAll targets agree on every differential program.")


if __name__ == "__main__":
    main()
