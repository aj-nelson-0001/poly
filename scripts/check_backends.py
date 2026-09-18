#!/usr/bin/env python3
"""Differential backend check: run one Poly program through every target and
fail when the outputs diverge.

This is the guard that caught the asm negative-integer printing bug, the asm
shared-temp-slot corruption, the C/JS missing match-expression support, the C
int32_t-default inference for struct-returning calls, the asm
struct-call-argument segfault, and the asm put-concat pointer-leaf bug. Any
backend behavior change must keep all targets agreeing here.

Usage: check_backends.py [--poly-bin PATH] [--stress]

Each `tests/diff_*.poly` program carries its expected output in a header
block: lines between the `EXPECTED:` marker and the end of the comment
region, one line of expected stdout per program output line.

Target-asymmetric programs (e.g. exercising one target's resource limit)
declare per-target failure expectations with directive lines:

    # EXPECTED-FAIL asm: exit=42, stderr=poly: string arena exhausted

On a named target the program must then FAIL: exit with the given code
(`exit=`) and print the given text to stderr (`stderr=`, substring match).
Multiple directives for different targets are allowed; the EXPECTED block
applies to every target without a directive. Directive values are split on
", " — do not put that sequence inside a stderr expectation.

`--stress` runs the heavier `tests/stress_*.poly` programs instead of the
every-push set: same EXPECTED format, larger iteration counts (the asm
bump arenas are 64 KiB vectors + 1 MiB strings, so ~2000 string results
and ~7000 vector slots fit), and a longer per-run timeout. Slow because
asm lacks optimization; CI runs it only in the nightly workflow.
"""
import argparse
import os
import re
import shutil
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
TESTS_DIR = REPO_ROOT / "tests"

_FAIL_DIRECTIVE = re.compile(r"^# EXPECTED-FAIL ([a-z]+): (.+)$")


@dataclass
class FailExpectation:
    """Per-target requirement that the program must fail at runtime."""

    exit_code: int | None = None
    stderr_contains: str | None = None

    def matches(self, returncode: int, stderr: str) -> str | None:
        """Return a mismatch description, or None when satisfied."""
        if self.exit_code is not None and returncode != self.exit_code:
            return f"expected exit {self.exit_code}, got {returncode}"
        if self.stderr_contains and self.stderr_contains not in stderr:
            return (
                f"stderr should contain {self.stderr_contains!r}, "
                f"got: {stderr[:200]!r}"
            )
        return None


def expected_from_source(source: str) -> tuple[list[str], dict[str, FailExpectation]]:
    """Extract the EXPECTED block and EXPECTED-FAIL directives from a program."""
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
    fail_expectations: dict[str, FailExpectation] = {}
    # Directives live in the comment header; they may appear before or after
    # the EXPECTED block, so scan every comment line (the loop above stops
    # at the first non-comment line after the marker).
    for line in lines:
        if not line.startswith("#"):
            continue
        match = _FAIL_DIRECTIVE.match(line)
        if not match:
            continue
        target, spec = match.groups()
        expectation = FailExpectation()
        for part in spec.split(", "):
            key, _, value = part.partition("=")
            if key == "exit":
                expectation.exit_code = int(value)
            elif key == "stderr":
                expectation.stderr_contains = value
            else:
                raise SystemExit(
                    f"unknown EXPECTED-FAIL key {key!r} (supported: exit, stderr)"
                )
        fail_expectations[target] = expectation
    return expected, fail_expectations


def run_target(
    poly_bin: Path,
    source: Path,
    target: str,
    workdir: Path,
    timeout: int = 120,
    expect_failure: bool = False,
) -> tuple[list[str], int, str]:
    """Run one target; return (stdout lines, exit code, stderr).

    With ``expect_failure`` a nonzero runtime exit is a result, not an error
    (build/compile failures still raise). Otherwise a nonzero exit raises,
    as before.
    """
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
        result = subprocess.run(
            [str(binary)], capture_output=True, text=True, timeout=timeout
        )
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
        result = subprocess.run(
            [str(binary)], capture_output=True, text=True, timeout=timeout
        )
    elif target == "js":
        project = workdir / "proj_js"
        subprocess.run(
            [str(poly_bin), "--project", str(project), str(source), "--target", "js"],
            check=True,
            capture_output=True,
            text=True,
        )
        result = subprocess.run(
            ["node", str(project / "main.js")],
            capture_output=True,
            text=True,
            timeout=timeout,
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
    if result.returncode != 0 and not expect_failure:
        raise SystemExit(
            f"{source.name} on target {target} exited {result.returncode}:\n{result.stderr}"
        )
    return result.stdout.splitlines(), result.returncode, result.stderr


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--poly-bin", default=None)
    parser.add_argument(
        "--stress",
        action="store_true",
        help="run tests/stress_*.poly (heavy, nightly) instead of tests/diff_*.poly",
    )
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
    pattern = "stress_*.poly" if args.stress else "diff_*.poly"
    timeout = 300 if args.stress else 120
    sources = sorted(TESTS_DIR.glob(pattern))
    if not sources:
        raise SystemExit(f"no {pattern} programs found in {TESTS_DIR}")
    for source in sources:
        expected, fail_expectations = expected_from_source(source.read_text())
        with tempfile.TemporaryDirectory() as tmp:
            workdir = Path(tmp)
            for target in targets:
                fail_expectation = fail_expectations.get(target)
                got, code, stderr = run_target(
                    poly_bin,
                    source,
                    target,
                    workdir,
                    timeout,
                    expect_failure=fail_expectation is not None,
                )
                if fail_expectation is not None:
                    if code == 0:
                        failures += 1
                        print(
                            f"FAIL {source.name} [{target}]: "
                            "expected a runtime failure, but the program succeeded"
                        )
                    else:
                        mismatch = fail_expectation.matches(code, stderr)
                        if mismatch:
                            failures += 1
                            print(f"FAIL {source.name} [{target}]: {mismatch}")
                        else:
                            print(f"ok   {source.name} [{target}] (expected failure)")
                    continue
                if got != expected:
                    failures += 1
                    print(f"FAIL {source.name} [{target}]")
                    print(f"  expected: {expected}")
                    print(f"  got:      {got}")
                else:
                    print(f"ok   {source.name} [{target}] ({len(expected)} lines)")
    if failures:
        print(
            f"\n{failures} divergence(s) across the "
            f"{'stress' if args.stress else 'differential'} suite"
        )
        sys.exit(1)
    print(
        f"\nAll targets agree on every "
        f"{'stress' if args.stress else 'differential'} program."
    )


if __name__ == "__main__":
    main()
