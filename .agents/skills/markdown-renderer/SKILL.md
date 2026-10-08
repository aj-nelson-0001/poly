---
name: markdown-renderer
description: Render Markdown to HTML or plain text locally and diff renders before/after doc edits.
---

# Markdown Renderer

Render any Markdown file in this environment to HTML or plain text with no
network access, and use render diffs to prove that a Markdown edit did not
change how the document looks.

## Primary renderer: pandoc (GFM)

`pandoc` 3.11 is installed at `/usr/bin/pandoc`. Use the `gfm` reader — it
matches what GitHub renders: pipe tables, task lists, strikethrough, both
tilde and backtick fences, and hard line breaks.

~~~bash
# file -> HTML
pandoc -f gfm -t html docs/file.md -o /tmp/file.html

# file -> plain text (quick readability check)
pandoc -f gfm -t plain docs/file.md

# snippet on stdin (no temp file)
printf -- 'line one  \nline two\n\n| a | b |\n|---|---|\n| 1 | 2 |\n' \
  | pandoc -f gfm -t html

# self-contained HTML with styling for reading in a browser
pandoc -f gfm -t html5 --standalone --metadata title=preview \
  docs/file.md -o /tmp/file.html
~~~

Exit code is pandoc's: nonzero means the input could not be parsed.

## Fallback renderer: Python-Markdown

If pandoc is unavailable, the `markdown` module (3.11) is installed. Enable
the extensions that cover this repository's conventions (pipe tables,
fenced code blocks):

~~~bash
python3 - <<'EOF'
import markdown
src = open('docs/file.md', encoding='utf-8').read()
html = markdown.markdown(src, extensions=['tables', 'fenced_code', 'sane_lists'])
open('/tmp/file.html', 'w', encoding='utf-8').write(html)
print('rendered', len(html), 'chars')
EOF
~~~

Python-Markdown is not CommonMark; prefer pandoc when both exist.

## Verification recipe: render before vs. after an edit

Use this whenever a documentation change should be proven rendering-neutral
(for example wrapping a long line, moving a hard break, or splitting a
table row):

~~~bash
# BEFORE: render the committed version straight from git
git show HEAD:docs/file.md | pandoc -f gfm -t html > /tmp/before.html
# AFTER: render the working tree
pandoc -f gfm -t html docs/file.md > /tmp/after.html
diff /tmp/before.html /tmp/after.html && echo "RENDER IDENTICAL"
~~~

A diff means the rendered structure changed: inspect both sides, and if the
change is intentional, re-run the check on the final content.

## Rendering facts worth asserting

- **Hard breaks:** two trailing spaces and a trailing backslash both render
  as `<br />` under GFM, so converting one to the other is render-neutral.
  Test any doubt directly:
  `printf 'a  \nb\n' | pandoc -f gfm -t html` vs `printf 'a\\\nb\n' | pandoc -f gfm -t html`.
- **Fenced code:** the language after the opening fence becomes a class,
  so a `poly`-labeled fence renders as `<pre class="poly">`; a lost
  language label shows up as a plain `<pre>`.
- **Tables:** column integrity is visible in the HTML — count `<th>`/`<td>`
  per row after edits. Whitespace immediately inside cell pipes is trimmed
  by renderers, so moving it does not change output.
- **Soft wraps:** a newline inside a paragraph renders as a space; only two
  spaces or a backslash at line end force a break.

## Limits

- Rendering is HTML text only; there is no browser or screenshot tool
  here — diff the HTML rather than trying to capture pixels.
- Poly code inside fences is displayed, not executed. Validate example
  code with `python3 scripts/check_poly_examples.py` instead; fence
  conventions are checked by `python3 scripts/check_markdown.py`.
- pandoc's task-list markup differs slightly from GitHub's; structural
  comparisons (counts, order, `<br />`, `<pre>` classes) are reliable,
  byte-for-byte identity with github.com is not.
