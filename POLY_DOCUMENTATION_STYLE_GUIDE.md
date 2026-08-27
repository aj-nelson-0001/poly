# Poly Documentation Style Guide

This guide defines the conventions for Poly documentation, examples, and reference material.

## Markdown Code Blocks

Use fenced code blocks made from three tilde characters. Add a language label when one is available:

~~~poly
var greeting := unicode "Hello"
put greeting
~~~

Use `poly` for complete Poly examples, `poly fragment` for intentionally abbreviated or version-specific snippets, `rust` for generated Rust, and `bash` for shell commands. Keep the opening and closing fences on their own lines.

Use single backticks for short inline references such as `var`, `:=`, or `unicode "text"`. Do not use triple-backtick fences in documentation.

Mark a non-standalone Poly snippet explicitly:

~~~poly fragment
if condition
    ...
~~~

A complete Poly block must parse independently; a block labeled `poly fragment` is still checked for canonical syntax but is not required to parse on its own. Use `fragment` only for abbreviated, legacy, future, or context-dependent examples—not to hide a parser error in a complete example.

## Canonical Poly Syntax

Variable declarations use `:=`. The type is optional and appears without a colon:

~~~poly
var count i32 := 0
var greeting := unicode "Hello"
~~~

Use `:=` for declarations and assignment, and `=` for equality comparisons. The legacy `==` spelling is rejected:

~~~poly fragment
count := count + 1
if count = 1
    put greeting
end if
~~~

Prefer the current delimiter-free conditional form. A comma after the condition remains accepted for compatibility, but new examples should omit it.

## Unicode Text

Use the explicit `unicode` keyword for Unicode string and character expressions:

~~~poly
var message ustring := unicode "こんにちは"
var checkmark := unicode '✓'
put unicode "Ready"
var answer ustring := get unicode "回答: "
~~~

Do not use retired flag-prefixed or type-prefixed Unicode spellings in examples, tables, or prose that describes current syntax.

## Example Quality

- Keep examples focused on the feature being explained.
- Use realistic names and values instead of unexplained placeholders when showing a complete program.
- Mark intentionally abbreviated or unsupported snippets with the `fragment` fence modifier; not every teaching fragment is intended to compile by itself.
- Use files in `examples/` for complete runnable programs and files in `tests/` for executable language behavior tests.
- Keep generated Rust examples separate from Poly examples and label them `rust`.
- Preserve meaningful comments, but avoid comments that contradict the current syntax.

## Syntax Tables

- Put each syntax form in inline code spans.
- Keep the Poly form in the Poly column and generated Rust in the Rust column.
- Use the same canonical syntax as fenced examples.
- Escape a pipe inside a closure when it appears in a Markdown table, for example `get with validate \|x\| x > 0`.
- Keep table descriptions concise and behavior-focused.

## Validation

Before submitting documentation changes, run:

~~~bash
python3 scripts/check_markdown.py
python3 scripts/check_poly_examples.py
cd compiler && cargo test --workspace
~~~

The Markdown checker enforces tilde fences and rejects retired Unicode spellings. The Poly example checker extracts every `poly` and `poly fragment` fence, checks canonical declaration and Unicode syntax in both, parses every unmarked `poly` block, and fails on any unmarked parse failure.

## Review Checklist

- [ ] Code fences use tilde characters and have a language label where appropriate.
- [ ] Poly declarations use `var name Type := value` or `var name := value`.
- [ ] Unicode expressions use `unicode "text"` or `unicode 'c'`.
- [ ] Syntax tables match the fenced examples.
- [ ] Complete examples parse successfully.
- [ ] Fragmentary examples are clearly presented as fragments.
- [ ] Documentation checks and the compiler test suite pass.
