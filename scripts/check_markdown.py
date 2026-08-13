#!/usr/bin/env python3
"""Check repository Markdown fence conventions.

Poly documentation uses tilde fences so code blocks are visually distinct from
inline Markdown code. This check intentionally leaves single-backtick inline
code alone, but rejects triple-backtick fences anywhere in Markdown files.
"""

from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]


def markdown_files():
    for path in sorted(ROOT.rglob("*.md")):
        if ".git" not in path.parts and "target" not in path.parts:
            yield path


def main() -> int:
    errors: list[str] = []
    checked = 0
    fence_markers = 0

    for path in markdown_files():
        checked += 1
        text = path.read_text(encoding="utf-8")
        relative_path = path.relative_to(ROOT)

        if "```" in text:
            errors.append(f"{relative_path}: contains triple-backtick Markdown")
        if "~~~poly\\n" in text:
            errors.append(f"{relative_path}: contains an escaped Poly fence; use real lines")

        count = text.count("~~~")
        fence_markers += count
        inside_fence = False
        for line_number, line in enumerate(text.splitlines(), start=1):
            if "~~~" not in line:
                continue
            if not line.lstrip().startswith("~~~"):
                errors.append(
                    f"{relative_path}:{line_number}: tilde marker is not a fence line"
                )
                continue
            inside_fence = not inside_fence

        if count % 2:
            errors.append(
                f"{relative_path}: has an odd number of tilde fence markers ({count})"
            )
        if inside_fence:
            errors.append(f"{relative_path}: has an unclosed tilde fence")

    if errors:
        print("Markdown convention check failed:")
        for error in errors:
            print(f"- {error}")
        return 1

    print(
        f"Markdown convention check passed: {checked} files, "
        f"{fence_markers} tilde fence markers, no triple-backtick fences."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
