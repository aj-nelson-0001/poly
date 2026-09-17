#!/usr/bin/env python3
"""Flag machine-specific paths in checked-in generated files.

Poly commits generated artifacts (the tetris Cargo project, playground
wasm) so consumers can build them without running the compiler first.
Those artifacts must be byte-reproducible across machines and checkouts:
any embedded user-home or CI-runner path silently breaks reproduction, as
happened with the original `.poly-generated` marker (`/home/andy/...` on
one machine, `/home/runner/...` on CI). This lint walks tracked generated
files and fails on those signatures.
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

# Generated files known to be committed to the repository. Extend this list
# when a new generated artifact becomes tracked.
GENERATED_PATTERNS = [
    "tetris/rust_output/**",
]

# Machine-specific path signatures: user home directories (Linux, macOS,
# Windows) and root. Deliberately narrow — ordinary absolute paths like
# `/usr/lib/...` are legitimate toolchain content, and Rust division
# (`width / COLS`) must not read as a path.
MACHINE_PATH = re.compile(
    r"/home/[\w.-]+/|/Users/[\w.-]+/|/root/|[A-Za-z]:\\Users\\"
)

SKIP_SUFFIXES = {".wasm", ".ico", ".png", ".jpg", ".bin"}


def tracked_generated_files() -> list[Path]:
    listed = subprocess.run(
        ["git", "ls-files", *GENERATED_PATTERNS],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    return sorted(
        Path(line)
        for line in listed.splitlines()
        if line and Path(line).suffix not in SKIP_SUFFIXES
    )


def violations_in(path: Path) -> list[str]:
    try:
        text = path.read_text(encoding="utf-8", errors="replace")
    except OSError as error:
        return [f"{path}: unreadable ({error})"]
    found = []
    for line_number, line in enumerate(text.splitlines(), start=1):
        for match in MACHINE_PATH.finditer(line):
            found.append(f"{path}:{line_number}: {match.group(0)}")
    return found


def main() -> int:
    argument_parser = argparse.ArgumentParser(description=__doc__)
    argument_parser.add_argument(
        "paths",
        nargs="*",
        type=Path,
        help="optional generated files to lint; defaults to all tracked generated files",
    )
    arguments = argument_parser.parse_args()

    files = arguments.paths or tracked_generated_files()
    if not files:
        print("Generated-path check: no tracked generated files matched.")
        return 0

    violations = [finding for path in files for finding in violations_in(path)]
    if violations:
        print("Generated-path check failed: machine-specific paths found:")
        print("\n".join(f"- {finding}" for finding in violations[:20]))
        if len(violations) > 20:
            print(f"- ... {len(violations) - 20} more")
        print("Regenerate the artifact from source instead of committing")
        print("machine-local paths; see POLY_SPEC_v2.md (Generated-project marker).")
        return 1

    print(f"Generated-path check passed: {len(files)} files, no machine-specific paths.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
