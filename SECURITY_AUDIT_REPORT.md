# Poly Compiler — Deep Security Audit

**Date:** 2026-10-03 · **Branch:** v2.0-dev · **Scope:** `compiler/crates/*` (12 crates),
`playground/`, `.github/workflows/`, `scripts/`
**Toolchain:** workspace compiles clean, `cargo fmt` clean, clippy `-D warnings` clean,
539 workspace tests pass, 0 failed

> **STATUS:** Findings 1–4 are **fixed in this branch** and pinned by new regression
> tests. Findings 5 and 6 are documented design properties, now stated explicitly in
> [POLY_SECURITY_GUIDE.md](POLY_SECURITY_GUIDE.md#0-running-the-compiler-on-untrusted-files)
> and [README.md](README.md). Finding 7 is a residual fidelity limitation, tracked
> rather than half-fixed. See "Residual risk" at the end.

Findings are ranked by severity. Every finding marked **confirmed** was reproduced
empirically against `compiler/target/release/poly`, and the reproductions are
reproduced verbatim below so anyone can re-run them.

Prior audits ([AUDIT_REPORT.md](AUDIT_REPORT.md),
[AUDIT_ROUND2_REPORT.md](AUDIT_ROUND2_REPORT.md),
[PROJECT_AUDIT_REPORT.md](PROJECT_AUDIT_REPORT.md),
[POLY_V2_AUDIT.md](POLY_V2_AUDIT.md)) are **correctness** audits — silent-wrong-behaviour
and codegen-divergence findings. None of the items below were previously tracked.

---

## 🔴 Critical

### 1. ✅ FIXED — `dep` version string injected arbitrary TOML into the generated `Cargo.toml` → arbitrary code execution

**Where:** `poly-cli/src/main.rs` `cargo_manifest()`

~~~rust
for (name, version) in declared_dependencies {
    dependencies.push_str(&format!("{name} = \"{version}\"\n"));
}
~~~

`version` came verbatim from `poly-parser/src/parser.rs`
`parse_dependency_declaration()`, and the lexer
(`poly-lexer/src/lexer.rs` string-escape handling) happily decodes `\"` and `\n`.
Nothing escaped or validated the value, so a `dep` line could close its own TOML
string and open arbitrary manifest tables.

**Why that is code execution:** `main.rs` `verify_rust_compiles_with_dependencies()`
writes that manifest into a temporary Cargo project and runs `cargo check` on it, so
the injected content reaches Cargo's dependency resolution. `cargo` executes build
scripts for path and build dependencies. The same manifest is written by `--project`,
which is then `cargo run`.

The lexer supplies the attacker exactly the two escapes needed: `\"` to terminate the
TOML string and `\n` to start a new line.

**Reproduction (confirmed end-to-end).** A one-line `.poly` file:

~~~poly fragment







dep serde = "1.0\"\nevilbuild = { path = \"/tmp/polysec/evilbuild\" }\n#"







fn main()







    put "hi"







end fn







~~~

with `/tmp/polysec/evilbuild/Cargo.toml` declaring `name = "evilbuild"` and a
`build.rs` that writes a marker file. `poly --project out rce.poly` produces:

~~~toml
[dependencies]
serde = "1.0"
evilbuild = { path = "/tmp/polysec/evilbuild" }
#"
~~~

The remainder of the file is a TOML comment, so the manifest is valid. Then:

~~~console







$ poly --check --strict rce.poly







OK: 2 statements parsed, rust code compiles







$ cat /tmp/polysec/PWNED_BUILD_SCRIPT







arbitrary code executed at build time







~~~

Note the output: **`--check --strict` reported OK while executing the attacker's
build script.** Two details make the exploit reliable and are worth recording for
anyone re-testing it: the injected dependency key must match the target crate's
`[package] name` (Cargo resolves the key to the package name, and a mismatch fails
resolution), and a plain `[dependencies]` path entry is the reliable vector —
`cargo check` does not build `[build-dependencies]` for a root package that has no
`build.rs` of its own.

**Fix.** Two layers.

1. **Parser (primary gate).** `poly-parser` now exposes
   `is_valid_dependency_name` / `is_valid_dependency_version`, and
   `parse_dependency_declaration` rejects anything outside those allowlists with a
   spanned diagnostic. The version allowlist is limited to the characters Cargo
   version requirements actually use — alphanumerics plus
   `. * + - _ , <space> ^ ~ > < =` — which excludes `"`, `\`, `[`, `]`, `{`, `}`,
   `#`, every control character, and every non-ASCII character. `dep` has no other
   use for a version string, so nothing legitimate is lost. The name allowlist is
   `[A-Za-z0-9_-]`, bounded at 64 characters (Cargo's own limit).
2. **Sink (defence in depth).** `cargo_manifest` re-checks each pair and skips
   anything invalid, so the manifest writer is never the only thing standing between
   a `.poly` file and an injected `[build-dependencies]` table.

Rejecting (rather than sanitizing) is deliberate: a sanitized version would be a
different string from the one the author wrote, and a silently rewritten dependency
is worse than a clear diagnostic.

**Pinned by:** `dependency_version_cannot_escape_the_generated_toml_string`,
`dependency_name_cannot_escape_the_generated_toml_string`,
`dependency_version_allowlist_matches_cargo_version_requirements`,
`ordinary_dependency_declarations_still_parse`,
`cargo_manifest_never_emits_an_injectable_dependency_line`.

---

## 🟠 High

### 2. ✅ DOCUMENTED — running `poly` on an untrusted file is code execution, and nothing said so

`poly file.poly` transpiles **and compiles and runs** the result. `--check` does not
run the program but does hand the generated code to `rustc` / `cargo check` / `cc` /
`as` / `node`. Three independent paths reach arbitrary code execution, all by
design:

1. **Foreign blocks.** `#rust`, `#c`, `#asm` and `#js` bodies are copied **verbatim**
   into the generated target source. Confirmed: a `#rust` block calling
   `std::process::Command::new("sh")` appeared unmodified in the generated
   `src/main.rs` and executed when the binary was run.
2. **Foreign calls.** With `--strict`, a call to a foreign function requires an
   explicit `extern <target> fn` declaration. Without it the default is permissive
   and the name is resolved by the target compiler.
3. **`dep` declarations.** These make the generated project resolve a crate through
   Cargo, which runs that crate's build scripts. (Finding 1 closed the injection
   path; a legitimate dependency is still build-time code.)

This is expected for a systems language and is not a bug. It was, however,
undocumented, which is the actual defect: a user had no way to know that
`--check` was not a safe way to inspect a file.

**Fix.** A new section `0. Running the compiler on untrusted files` at the top of
[POLY_SECURITY_GUIDE.md](POLY_SECURITY_GUIDE.md) — the existing guide was entirely
about writing secure *Poly programs* and said nothing about running the *compiler* on
hostile input. It gives a per-command table of what each mode does, the three
execution paths above, and the rules (never run `poly` on a file you did not write;
prefer `--tokens` / `--ast` / `--emit-*` for inspection, while noting those are not a
security boundary; use `--strict`; containerize on shared hosts). README.md now
carries a short callout next to the foreign-blocks section and a pointer to it from
the documentation table. The same section also lists the compiler-side guarantees
(findings 1, 3, 6 and the identifier/nesting caps), so the boundary between "the
compiler is hardened" and "the compiler still executes your file" is explicit.

### 3. ✅ FIXED — foreign-source RCE reached CI silently, and every action was pinned to a floating tag

`ci.yml` runs `--check --strict` over `examples/`, `tests/` and the differential
fixtures, and `tetris` additionally builds and runs a generated project. Since
`--check` executes build scripts and `--target c`/asm/JS fixtures are compiled and
run, **a malicious pull request that adds a `.poly` fixture executes code on the
runner** — while reporting a green build. Compounding it, every action was pinned to
a mutable tag (`actions/checkout@v4`, `dtolnay/rust-toolchain@1.98.0`,
`actions/cache@v4`, `msys2/setup-msys2@v2`, `actions/upload-artifact@v4`,
`actions/download-artifact@v4`, `softprops/action-gh-release@v2`), so a compromised
or re-pointed upstream tag would land in a workflow that also runs untrusted code.

**Fix.**

- **Least privilege.** `permissions: {}` at the top of `ci.yml` and
  `differential.yml`, where no job needs a token. In `release.yml`, `permissions: {}`
  at the top with `contents: write` granted only to `create-release`. Artifact
  upload/download uses the Actions runtime token rather than `GITHUB_TOKEN`, so it is
  unaffected. A hostile job now has no token to steal and nothing to exfiltrate with.
- **Immutable pins.** All 28 action references across the three workflows are now
  full commit SHAs with the version in a trailing comment, e.g.
  `actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # v4.4.0`. The SHAs are
  the commits the floating tags resolved to, so the pin preserves current behaviour
  exactly while removing the tag's mutability.

Each pinning carries a version comment, and
[BRANCH_PROTECTION.md](BRANCH_PROTECTION.md) is the place to record the bump policy.

---

## 🟡 Medium

### 4. ✅ FIXED — `\0` and the remaining control characters were not escaped in generated C / asm / JS

`c_escape()` (poly-c-codegen), `asm_escape()` (poly-asm-codegen) and `js_escape()`
(poly-js-codegen) each handled only `\\ " \n \r \t`, and did so with a chain of
`str::replace` calls. A NUL in a Poly string therefore reached the generated source as
a raw byte.

**Reproduction (confirmed).**

~~~poly
fn main()
    put "a\0b"
end fn
~~~

emitted `printf("a<NUL>b\n")`. `cc -std=c11` only *warns*
(`warning: null character(s) preserved in literal`) and exits 0, so the build looks
clean while the binary prints `a` — **silent string truncation**. The same raw NUL
appeared in the JS output.

**Fix.** All three escapers rewritten as a single character-wise pass. NUL, every
other C0 control, and DEL (`0x7f`) are emitted as escapes; C and asm use fixed-width
three-digit octal and JS uses fixed-width `\xNN`, so the following character can
never be absorbed as extra digits. Characters above `0x7f` pass through unchanged —
the emitted source is UTF-8 and matches the bytes the runtime string already holds.

This also fixed the JS target's actual behaviour: `put "a\0b"` on `--target js` now
round-trips the NUL (`node` receives `61 00 62`, verified), where the raw-byte form
was relying on Node's tolerance of a control character in source.

**Pinned by:** `c_escape_covers_nul_and_every_control_character`,
`js_escape_covers_nul_and_every_control_character`,
`asm_escape_covers_nul_and_every_control_character` — each covering NUL, `\u{1b}`,
DEL, a trailing hex/octal-looking digit, the five classic escapes, and non-ASCII
pass-through.

### 5. ✅ FIXED — predictable temp directories and files in the shared temp dir (`CWE-377`)

Every scratch path handed to an external toolchain was `<pid>-<nanos>` written
straight into `std::env::temp_dir()`:

- `verify_rust_compiles_with_dependencies()` — the temporary Cargo project
  (`poly_check_deps_<pid>-<nanos>`), created with `create_dir_all`, which **succeeds
  on a pre-existing directory**
- `verify_async_rust_compiles()` — the same, for `poly_check_async_*`
- `verify_rust_compiles()`, `verify_c_compiles()`, `verify_asm_compiles()`,
  `verify_js_compiles()`, `compile_rust_binary()` — loose `poly_check_*` source and
  object files

`nanos` is guessable by a local attacker who knows roughly when the process started,
and `create_dir_all` does not fail if the path already exists. Cargo then resolves,
builds, and **executes build scripts and proc macros** from whatever manifest it finds
in that directory. I demonstrated the race with a watcher loop: it won the window and
replaced the project's `src/` with a symlink to an attacker-chosen directory, after
which Cargo operated on the attacker's paths.

**Fix.** `create_private_temp_dir(prefix)` in `poly-cli/src/main.rs`:

- the name embeds a hash of `RandomState`, whose seed the OS supplies per process, so
  it is not predictable or pre-creatable;
- it uses `create_dir` (not `create_dir_all`), which fails on an existing path, and
  retries across a bounded range;
- on Unix it creates the directory `0700`, so another user can enumerate the name in
  `/tmp` but cannot enter the directory, replace a file, or read the generated source.

All eight scratch paths now go through it and clean up with `remove_dir_all` instead
of `remove_file`, so nothing is left behind. No new dependency was added — the
workspace's minimal-dependency posture is preserved. The residual (very low) window is
enumeration-plus-race within the microseconds between `create_dir` and the first
write, which the `0700` mode closes for any attacker who is not the same user.

**Pinned by:** `private_temp_dirs_are_private_unique_and_unguessable`, which asserts
distinct names, temp-dir containment, `0700` mode, and cleanup.

---

## 🟢 Low

### 6. ✅ CHECKED, documented — hand-rolled `struct termios` FFI is glibc-shaped only

`poly-cli/src/repl.rs` declares `RawTermios` by hand for the REPL's raw mode. The
Linux variant (`c_line: u8`, `c_cc: [u8; 32]`) is **correct for glibc** — the field
offsets were checked against `struct termios` (16 / 17..49 / 52 / 56) and match. The
macOS variant is likewise plausible.

It is wrong on **musl**, whose `struct termios` has `c_cc[19]` and no `c_line`. This
is not memory-unsafe (our struct is larger than musl's, so `tcgetattr` writes less
than we allocated), but the offsets shift, `ISIG`/`ICRNL`/`IXON` are never applied,
and the REPL's raw mode misbehaves on Alpine. `Default` is `std::mem::zeroed()`, which
is sound for this all-integer struct.

Left as-is and recorded rather than changed: the fix is to use `libc::termios` or to
gate on `target_env = "gnu"`, and neither is a security issue. Noted here so it is
not rediscovered as a bug report from an Alpine user.

---

## ✅ Verified clean (checked, no issue)

| Area | Result |
|---|---|
| **Identifier injection into C/JS/asm** | Impossible. `poly-lexer` starts
identifiers only on `[a-zA-Z_]` and continues only on alphanumerics/underscore,

so no variable name can break out into generated target syntax. Confirmed

empirically: `var a$b i32 := 1` produces no C output. | |

| **Expression nesting DoS** | Capped at 32 (`MAX_EXPRESSION_DEPTH`, enforced

in `parse_unary`). 200,000 nested parens produce a clean `Maximum nested

expression depth (32) exceeded` error, no crash. **Superseded:** this cap

alone was later found insufficient — see "Flat operator chains" below. | |

| **Statement nesting DoS** | Capped at 128 (`MAX_STATEMENT_DEPTH`, checked

in `parse_block`). 100,000 nested `loop`/`end loop` produce a clean parse

error, no stack overflow. Note that the CI comments describing `parse_block`

as unbounded recursion are stale relative to this guard. | |

| **Playground XSS** | No `innerHTML`, `eval`, `new Function`, or

`document.write` anywhere; all compiler output goes through `textContent`.

WASM is instantiated from `fetch()` bytes with **zero host imports**, and the

Rust `unsafe` blocks null- and length-check before every `from_raw_parts`. | |

| **LSP** | The hand-rolled JSON parser in `poly-lsp/src/json.rs` is

bounds-checked (`peek` uses `.get()`), rejects trailing characters, and

escapes every control character below `0x20` in output strings. The server

performs **no file I/O at all**. **Superseded:** nesting depth and frame size

are now bounded too — see below. | |

| **Python scripts** | All subprocess calls use `subprocess.run([...])`

argument lists — no `shell=True`, no `os.system`, no `eval`. | |

| **Dependency posture** | `cargo audit` reports **0 vulnerabilities** across 82 crates. CI has an `audit` job. |

| **Subprocess use** | No shell invocation anywhere in the compiler. `POLY_CC`

and `POLY_NODE` name a single executable and are environment-controlled, not

reachable from source. | |

| **Output-path safety** | `--project` validates the `.poly` extension, uses

a `.poly-generated` marker file to avoid clobbering a non-Poly project, and

sanitizes the package/directory name. | |

---

## Residual risk

### 7. `put` of a string containing NUL still truncates on the C target

Fixed-width octal in `c_escape` makes the *emitted source* valid, but the C target
builds `put` as a single `printf("<format>\n", …)` call, and a C string literal is
NUL-terminated — so the call still stops at the NUL. `%.*s` does **not** help here
(precision is a maximum, not a byte count); an `fwrite`-per-piece emission path with an
explicit length would be required, which changes the shape of the `put` lowering
across all four targets and would churn the codegen snapshots.

The JS and asm targets carry an explicit byte length and are correct.

Recorded in a comment at the `printf_parts` string-literal arm so it is not
rediscovered as a regression, and deliberately **not** half-fixed with a `%.*s` that
would not have worked.

### 8. `--emit-*` is inspection, not a boundary

`--tokens`, `--ast` and `--emit-*` run the front end but hand nothing to an
external toolchain, which makes them the right choice for looking at a file you did
not write. They are not a security boundary: they exercise the lexer and parser on
attacker-controlled input (which findings 4 and the nesting caps do harden), and
their *output* is untrusted. The Security Guide says this explicitly so nobody
mistakes them for a sandbox.

---

## Test status

- 539 workspace tests pass, 0 failed (9 new regression tests added by this audit)
- `cargo fmt --all -- --check` clean
- `cargo clippy --workspace --all-targets -- -D warnings` clean
- `python3 scripts/check_markdown.py` — 55 files, clean
- `python3 scripts/check_poly_examples.py` — 587 blocks, 101 passed, 0 failures
- `python3 scripts/check_generated_paths.py` — clean
- `cargo audit` — 0 vulnerabilities
- Both attack reproductions re-run against the fixed binary: the `dep` injection is
  rejected at parse time with a diagnostic, and the JS target now round-trips NUL

## Follow-ups not taken

- **`RawTermios` on musl** (finding 6): a portability bug, not a security one; left
  recorded.
- **A `--no-build` / `--deny-foreign-blocks` mode** (finding 2): the compiler already
  has `--strict` for foreign *calls*, and foreign *blocks* are the feature. A flag
  that refuses them would be new language surface, so it is left as a design decision
  rather than a security patch.
- **`printf_parts` → ordered `fwrite` pieces** (finding 7): described above.

## Found after this audit was written

This report is a snapshot dated 2026-10-03 (539 tests). Three further
availability issues were found and fixed later; each was verified by disabling
the guard and reproducing the original crash.

- **Flat operator chains overflowed the stack.** `MAX_EXPRESSION_DEPTH` bounds
  *syntactic* nesting, so `1 + 1 + … + 1` has no delimiters for it to count —
  yet the precedence ladder folds it into a tree one level deep per operand.
  Three consumers then recurse over that tree, including Rust's own `Drop` for
  `Box<Expression>`, which is why rejecting the finished tree did not help: it
  still had to be dropped. The parser now refuses to build the tree, capping a
  chain at 512 operands per statement, with an iterative depth validator and a
  type-checker backstop behind it. A 200k-operand file exits with a diagnostic
  instead of aborting.
- **Deeply nested JSON aborted the LSP.** `parse_value` recursed without a
  limit. Now capped at 128 levels, checked in a wrapper so the counter unwinds
  on every early return; the boundary holds at 128 accepted / 129 rejected.
- **`Content-Length` was trusted outright**, so one header could demand an
  arbitrary allocation before a byte was read (`Content-Length:
  100000000000` aborted with an allocation failure). Frames over 8 MiB are now
  refused before allocating. That figure is measured rather than guessed: a real
  session against the largest `.poly` in the tree peaks at a 21 KB frame.

Full detail, including the negative tests, is in
[SECURITY_FIX_HANDOFF.md](SECURITY_FIX_HANDOFF.md); the limits themselves are
specified in [POLY_SPEC_v2.md](POLY_SPEC_v2.md#depth-limits).
