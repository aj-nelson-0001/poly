#!/usr/bin/env python3
"""Prepare a Poly release: bump the version, regenerate, verify.

Automates the manual steps performed for 2.0.0-preview.4/.5:

1. Bump the workspace version in ``compiler/Cargo.toml`` and refresh
   ``compiler/Cargo.lock`` (workspace members only; dependencies untouched).
2. Regenerate the tetris generated project and restore its Cargo
   dependencies, then refresh its ``Cargo.lock`` **online** (an offline
   resolution downgrades transitive dependencies relative to the original
   lockfile) and verify ``src/main.rs`` matches ``--emit-rust`` output.
3. Run the full verification battery: workspace tests, ``cargo fmt --check``,
   ``cargo clippy -D warnings``, the markdown convention check, and the
   full-repository Poly documentation audit.

The script never commits, tags, or pushes. Review the diff, commit, and tag:

    git add -A
    git commit -m "chore(release): prepare <version>"
    git tag -a <version> -m "Poly <version>"
    git push origin <branch> <version>

Usage:

    python3 scripts/prepare_release.py 2.0.0-preview.6
    python3 scripts/prepare_release.py --skip-tests 2.0.0-preview.6
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
WORKSPACE_TOML = ROOT / "compiler" / "Cargo.toml"
WORKSPACE_LOCK = ROOT / "compiler" / "Cargo.lock"
TETRIS_DIR = ROOT / "tetris"
TETRIS_PROJECT = TETRIS_DIR / "rust_output" / "tetris"
TETRIS_SOURCE = TETRIS_DIR / "tetris.poly"
TETRIS_POLY_BIN = ROOT / "compiler" / "target" / "release" / "poly"
TETRIS_MAIN = TETRIS_PROJECT / "src" / "main.rs"
TETRIS_README = TETRIS_DIR / "README.md"
VERSION_RE = re.compile(r'^(version = ")([^"]+)(")$', re.MULTILINE)
# The tetris README documents the dependency pins; the generated project
# must agree with it.
TETRIS_README_DEPS = {"minifb": "0.27", "alsa": "0.9"}
# Docs whose first stated version must equal the release version; the value
# describes what to look for when the check fails.
DOC_BASELINE_FILES = {
    "POLY_DOCUMENTATION_INDEX.md": "baseline statement",
    "POLY_V2_PREVIEW_RELEASE_NOTES.md": "intro line",
}
DOC_BASELINE_RE = re.compile(r"(2\.0\.0-preview\.\d+)")


def run(cmd: list[str], cwd: Path | None = None, allow_failure: bool = False) -> str:
    print(f"  $ {' '.join(cmd)}")
    result = subprocess.run(cmd, cwd=cwd or ROOT, capture_output=True, text=True)
    if result.returncode and not allow_failure:
        tail = (result.stderr or result.stdout).strip().splitlines()
        detail = "\n".join(f"    {line}" for line in tail[-10:])
        raise SystemExit(f"command failed ({result.returncode}):\n{detail}")
    return result.stdout


def current_version() -> str:
    match = VERSION_RE.search(WORKSPACE_TOML.read_text(encoding="utf-8"))
    if not match:
        raise SystemExit(f"no version found in {WORKSPACE_TOML}")
    return match.group(2)


def bump_workspace_version(new_version: str) -> None:
    text = WORKSPACE_TOML.read_text(encoding="utf-8")
    updated = VERSION_RE.sub(rf'\g<1>{new_version}\g<3>', text, count=1)
    WORKSPACE_TOML.write_text(updated, encoding="utf-8")
    print(f"  compiler/Cargo.toml: version -> {new_version}")
    # Refresh the lockfile without touching dependency versions; cargo updates
    # only the workspace members' own entries when the manifest changes.
    run(["cargo", "update", "-w", "--offline"], cwd=ROOT / "compiler")


def tetris_dep_pins() -> dict[str, str]:
    """Dependency pins from the generated project's Cargo.toml."""
    pins: dict[str, str] = {}
    for line in (TETRIS_PROJECT / "Cargo.toml").read_text(encoding="utf-8").splitlines():
        dep = re.match(r'^(\w+)\s*=\s*"([^"]+)"', line.strip())
        if dep and dep.group(1) in TETRIS_README_DEPS:
            pins[dep.group(1)] = dep.group(2)
    return pins


def check_version_consistency(version: str) -> None:
    """Fail when the tetris project or doc baselines lag the new version.

    The docs must be edited by hand before running this script (release
    highlights and changelog entries cannot be generated mechanically), so a
    stale baseline means the human edit step was skipped.
    """
    project_version = VERSION_RE.search(
        (TETRIS_PROJECT / "Cargo.toml").read_text(encoding="utf-8")
    )
    if not project_version or project_version.group(2) != version:
        found = project_version.group(2) if project_version else "none"
        raise SystemExit(
            f"tetris generated project version is {found}, expected {version}"
        )
    problems: list[str] = []
    for name, pattern in DOC_BASELINE_FILES.items():
        text = (ROOT / name).read_text(encoding="utf-8")
        match = DOC_BASELINE_RE.search(text)
        if not match or match.group(1) != version:
            found = match.group(1) if match else "no version statement"
            problems.append(f"- {name}: {pattern} ({found} found, expected {version})")
    if problems:
        raise SystemExit(
            "stale release metadata; edit these by hand, then re-run:\n"
            + "\n".join(problems)
        )
    print(f"  doc baselines state {version}")


def regenerate_tetris_project() -> None:
    run(["cargo", "build", "--release", "-p", "poly-cli"], cwd=ROOT / "compiler")
    # --project refuses to touch an existing directory; regenerate from scratch.
    run(["rm", "-rf", str(TETRIS_PROJECT)])
    run([str(TETRIS_POLY_BIN), "--project", str(TETRIS_PROJECT), str(TETRIS_SOURCE)])
    # --project does not emit the Cargo dependencies the source relies on.
    toml_path = TETRIS_PROJECT / "Cargo.toml"
    toml = toml_path.read_text(encoding="utf-8")
    if "minifb" not in toml:
        toml = toml.replace(
            "[dependencies]",
            "[dependencies]\n" + "\n".join(
                f'{name} = "{ver}"' for name, ver in TETRIS_README_DEPS.items()
            ),
            1,
        )
        toml_path.write_text(toml, encoding="utf-8")
        print("  restored tetris Cargo dependencies")
    # Online resolution: an --offline run downgrades transitive deps relative
    # to the committed lockfile (observed with web-sys et al on 2026-09-17).
    lock_path = TETRIS_PROJECT / "Cargo.lock"
    if lock_path.exists():
        lock_path.unlink()
    run(
        ["cargo", "generate-lockfile", "--manifest-path", str(TETRIS_PROJECT / "Cargo.toml")],
        allow_failure=True,
    )
    # Generated main.rs must equal --emit-rust output minus its extra
    # trailing newline; a mismatch means regeneration drift.
    emitted = run([str(TETRIS_POLY_BIN), str(TETRIS_SOURCE), "--emit-rust"])
    emitted_lines = emitted.splitlines()
    tracked = TETRIS_MAIN.read_text(encoding="utf-8").splitlines()
    if emitted_lines[:-1] != tracked:
        raise SystemExit("regeneration drift: src/main.rs != --emit-rust output")
    print("  tetris regeneration verified (src/main.rs matches --emit-rust)")
    # Dependency pins must agree with the tetris README's documented pins.
    pins = tetris_dep_pins()
    if pins != TETRIS_README_DEPS:
        raise SystemExit(
            f"tetris dependency pins {pins} disagree with README pins "
            f"{TETRIS_README_DEPS}; update TETRIS_README_DEPS in this script "
            "or the README"
        )
    print(f"  dependency pins match README: {pins}")


def verify(skip_tests: bool) -> None:
    checks: list[tuple[str, list[str], Path]] = [
        ("cargo fmt --check", ["cargo", "fmt", "--all", "--", "--check"], ROOT / "compiler"),
        (
            "cargo clippy -D warnings",
            ["cargo", "clippy", "--workspace", "--all-targets", "--", "-D", "warnings"],
            ROOT / "compiler",
        ),
        (
            "markdown conventions",
            ["python3", "scripts/check_markdown.py"],
            ROOT,
        ),
        (
            "poly documentation audit",
            [
                "python3",
                "scripts/check_poly_examples.py",
                "--poly-bin",
                str(TETRIS_POLY_BIN),
            ],
            ROOT,
        ),
    ]
    if not skip_tests:
        checks.insert(
            0,
            (
                "cargo test --workspace",
                ["cargo", "test", "--workspace"],
                ROOT / "compiler",
            ),
        )
    for label, cmd, cwd in checks:
        print(f"  [{label}]")
        run(cmd, cwd=cwd)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("version", help="new version, e.g. 2.0.0-preview.6")
    parser.add_argument(
        "--skip-tests",
        action="store_true",
        help="skip the workspace test run (fmt/clippy/audits still run)",
    )
    arguments = parser.parse_args()

    old = current_version()
    print(f"Preparing release: {old} -> {arguments.version}")
    if old == arguments.version:
        print("Version unchanged; regenerating and re-verifying only.")

    print("1/3 Workspace version bump")
    bump_workspace_version(arguments.version)

    print("2/3 Tetris generated project")
    regenerate_tetris_project()

    print("3/3 Verification")
    check_version_consistency(arguments.version)
    verify(skip_tests=arguments.skip_tests)

    print(f"\nReady. Review the diff, then commit and tag {arguments.version}.")
    print("This script does not commit, tag, or push.")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except KeyboardInterrupt:
        sys.exit(130)
