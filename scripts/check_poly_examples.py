#!/usr/bin/env python3
"""Audit fenced Poly examples in Markdown documentation.

Complete examples must parse independently. Documentation may also contain
intentionally abbreviated or version-specific snippets; those must be marked
with a ``fragment`` fence modifier, for example ``~~~poly fragment``. All
examples, including fragments, still undergo the canonical syntax policy check.
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
COMPILER = ROOT / "compiler"
BINARY = COMPILER / "target" / "debug" / "poly"

LEGACY_UNICODE = re.compile(r'(?<![A-Za-z])u["\']|-u\s+["\']')
LEGACY_TYPED_VAR = re.compile(
    r"^\s*var\s+[A-Za-z_]\w*\s*:(?!=)\s*.+?(?::=|(?<!:)=(?!=))"
)
LEGACY_VAR_EQUALS = re.compile(
    r"^\s*var\s+[A-Za-z_]\w*(?:\s+[A-Za-z_]\w*)?\s*(?<!:)=(?!=)"
)


@dataclass
class PolyExample:
    path: Path
    line: int
    source: str
    fragment: bool


def markdown_files(selected: list[Path] | None = None):
    if selected:
        for path in selected:
            resolved = path if path.is_absolute() else ROOT / path
            if resolved.is_file() and resolved.suffix == ".md":
                yield resolved
        return

    for path in sorted(ROOT.rglob("*.md")):
        if ".git" not in path.parts and "target" not in path.parts:
            yield path


def extract_examples(selected: list[Path] | None = None) -> list[PolyExample]:
    examples: list[PolyExample] = []
    for path in markdown_files(selected):
        lines = path.read_text(encoding="utf-8").splitlines()
        in_poly = False
        fragment = False
        body: list[str] = []
        start = 0
        for number, line in enumerate(lines, start=1):
            fence = line.lstrip()
            if fence.startswith("~~~"):
                if not in_poly:
                    labels = fence[3:].strip().split()
                    in_poly = bool(labels) and labels[0] == "poly"
                    fragment = in_poly and "fragment" in labels[1:]
                    body = []
                    start = number
                else:
                    examples.append(
                        PolyExample(
                            path,
                            start,
                            "\n".join(body) + "\n",
                            fragment,
                        )
                    )
                    in_poly = False
                    fragment = False
                continue
            if in_poly:
                body.append(line)
    return examples


def validate_policy(example: PolyExample) -> list[str]:
    errors: list[str] = []
    for offset, line in enumerate(example.source.splitlines(), start=example.line + 1):
        if LEGACY_UNICODE.search(line):
            errors.append(f"{example.path}:{offset}: retired Unicode literal syntax")
        if LEGACY_TYPED_VAR.search(line) or LEGACY_VAR_EQUALS.search(line):
            errors.append(f"{example.path}:{offset}: use `var name Type := value`")
        if re.search(r":=(?=\S)", line) and "::=" not in line:
            errors.append(f"{example.path}:{offset}: add a space after `:=`")
    return errors


def ensure_binary() -> None:
    # Let Cargo cheaply verify freshness instead of trusting a possibly stale binary.
    result = subprocess.run(
        ["cargo", "build", "-q", "-p", "poly-cli"],
        cwd=COMPILER,
        text=True,
    )
    if result.returncode:
        raise RuntimeError("unable to build the Poly CLI for documentation parsing")


def parse_examples(examples: list[PolyExample]) -> list[tuple[PolyExample, str]]:
    ensure_binary()
    failures: list[tuple[PolyExample, str]] = []
    with tempfile.TemporaryDirectory(prefix="poly-doc-examples-") as directory:
        directory_path = Path(directory)
        for index, example in enumerate(examples):
            source_path = directory_path / f"example-{index:04d}.poly"
            source_path.write_text(example.source, encoding="utf-8")
            # Complete examples must parse *and* type-check, and the generated
            # Rust must compile. Fragments are exempt from this step.
            target = "c" if re.search(r"^\s*#c\s*$", example.source, re.MULTILINE) else "rust"
            result = subprocess.run(
                [str(BINARY), "--target", target, "--check", str(source_path)],
                capture_output=True,
                text=True,
            )
            if result.returncode:
                message = (result.stderr or result.stdout).splitlines()
                failures.append((example, message[0] if message else "check failed"))
    return failures


def main() -> int:
    argument_parser = argparse.ArgumentParser(description=__doc__)
    argument_parser.add_argument(
        "paths",
        nargs="*",
        type=Path,
        help="optional Markdown files to audit; defaults to the whole repository",
    )
    arguments = argument_parser.parse_args()
    examples = extract_examples(arguments.paths or None)
    policy_errors = [error for example in examples for error in validate_policy(example)]
    if policy_errors:
        print("Poly documentation policy check failed:")
        print("\n".join(f"- {error}" for error in policy_errors))
        return 1

    complete_examples = [example for example in examples if not example.fragment]
    failures = parse_examples(complete_examples)

    print(
        f"Poly documentation audit: {len(examples)} blocks, "
        f"{len(complete_examples) - len(failures)} passed, "
        f"{sum(example.fragment for example in examples)} marked fragments, "
        f"{len(failures)} unmarked check failures."
    )
    for example, message in failures[:20]:
        print(f"- {example.path}:{example.line}: {message}")
    if len(failures) > 20:
        print(f"- ... {len(failures) - 20} more unmarked failures")

    return 1 if failures else 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except RuntimeError as error:
        print(f"Poly documentation audit failed: {error}", file=sys.stderr)
        sys.exit(1)
