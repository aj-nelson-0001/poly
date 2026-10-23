//! Documentation-claim guards: pin the per-backend behavior claims that the
//! maintained docs (README, POLY_SPEC_v2, POLY_GRAMMAR, POLY_QUICK_REFERENCE,
//! POLY_API_REFERENCE, POLY_CHEATSHEET, POLY_JS_BLOCKS,
//! POLY_V2_SUPPORT_MATRIX, POLY_MIGRATION_GUIDE_v2) make about the five
//! targets.
//!
//! Every case was verified end-to-end during the 2.0.0-preview.12
//! documentation audit (probes compiled through every backend, several built
//! and executed). The expectation enum, target list, and runner live in the
//! shared `common` module alongside `capability_parity.rs`, so the two
//! suites cannot disagree about what "supported on target X" means. If one
//! of these tests fails, either the backend changed or the docs did —
//! update the backend, the affected docs, and the pinned expectation in the
//! same commit so the documentation stays truthful.

mod common;

use common::{Case, Expect};

const CASES: &[Case] = &[
    // --- corrected claim: interpolation is NOT rust-only -------------------
    Case::accepted(
        "string interpolation works on all targets",
        "fn main()\n    var n i32 := 42\n    put \"n is {n} and done\"\nend fn\n",
        Expect::All,
    ),
    // --- corrected claim: C backend supports plain stdin `get` -------------
    Case::with_output(
        "c get with prompt",
        "fn main()\n    var name ustring := get unicode \"Name: \"\n    put name\nend fn\n",
        Expect::RustC,
        ("c", "__poly_get_line(\"Name: \")"),
    ),
    Case::with_output(
        "c get without prompt",
        // Regression for the 0-args-to-1-arg-helper bug: the generated C
        // must call the helper with an explicit empty string literal.
        "fn main()\n    var line := get\n    put line\nend fn\n",
        Expect::RustC,
        ("c", "__poly_get_line(\"\")"),
    ),
    // --- corrected claim: JS backend supports structs ----------------------
    Case::with_output(
        "js struct declaration and literal",
        concat!(
            "struct Point\n    x: i32\n    y: i32\nend struct\n\nfn main()\n    var p := Point { x: 3, y:",
            " 4 }\n    put p.x + p.y\nend fn\n",
        ),
        Expect::All,
        ("js", "Point"),
    ),
    // --- corrected claim: JS backend supports tuples -----------------------
    Case::accepted(
        "js tuple literal and index access",
        "fn main()\n    var t := (10, 20)\n    put t.0\n    put t.1\nend fn\n",
        Expect::RustCJs,
    ),
    // --- corrected claim: match expressions and typed tuples are portable --
    Case::accepted(
        "match expression on all targets",
        "fn main()\n    var m := match 1\n        0, 100\n        _, 200\n    end match\n    put m\nend fn\n",
        Expect::All,
    ),
    Case::accepted(
        "typed tuple on all targets",
        "fn main()\n    var pair (i32, i32) := (1, 2)\n    put pair.0\nend fn\n",
        Expect::RustCJs,
    ),
    // --- corrected claim: `..=` inclusive range loops on all targets -------
    Case::accepted(
        "inclusive range loop",
        "fn main()\n    loop j 0..=4\n        put j\n    end loop\nend fn\n",
        Expect::All,
    ),
    // --- corrected claim: a literal zero step is a compile-time error ------
    // (docs previously said it "yields zero iterations"; only runtime zero
    // steps do — see the next case).
    Case::accepted(
        "literal zero step is rejected",
        "fn main()\n    loop i 0..10 step 0\n        put i\n    end loop\nend fn\n",
        Expect::AllReject,
    ),
    Case::accepted(
        "runtime zero step compiles everywhere",
        "fn main()\n    var s i32 := 0\n    loop i 0..10 step s\n        put i\n    end loop\n    put \"after\"\nend fn\n",
        Expect::All,
    ),
    // --- rows pinned from POLY_V2_SUPPORT_MATRIX.md (probed 2026-09-20) ----
    // Enum declarations: C/asm support unit variants (payload-carrying are
    // rejected); JS rejects enums entirely.
    Case::accepted(
        "unit enum declaration and qualified match",
        concat!(
            "enum Color\n    Red\n    Green\nend enum\n\nfn main()\n    var c := Color::Green\n    match ",
            "c\n        Color::Red, put 1\n        Color::Green, put 2\n        _, put 0\n    end match\n",
            "end fn\n",
        ),
        Expect::RustCAsm,
    ),
    // Closures: C compiles non-capturing only; capturing are rejected with
    // guidance; asm and JS reject all closures.
    Case::accepted(
        "non-capturing closure",
        "fn main()\n    var twice := |x: i32| x * 2\n    put twice(21)\nend fn\n",
        Expect::RustC,
    ),
    Case::accepted(
        "capturing closure is rejected outside rust",
        "fn main()\n    var n i32 := 10\n    var addn := |x: i32| x + n\n    put addn(5)\nend fn\n",
        Expect::RustOnly,
    ),
    // Vectors: C has none; JS supports literals/len but no mutators; asm
    // supports literals and mutators via its growable runtime.
    Case::accepted(
        "vector literal and len",
        "fn main()\n    var v Vec<i32> := [1, 2, 3]\n    put v.len()\nend fn\n",
        Expect::RustAsmJs,
    ),
    Case::accepted(
        "vector push",
        "fn main()\n    var v Vec<i32> := [1, 2]\n    v.push(3)\n    put v.len()\nend fn\n",
        Expect::RustAsm,
    ),
    // --- comment styles (POLY_QUICK_REFERENCE.md, POLY_TUTORIAL.md) -------
    // All three comment styles are stripped by the lexer before any backend
    // sees the token stream, so every target must accept them identically —
    // and the statements around each comment must survive compilation.
    Case::accepted(
        "hash line comments on every target",
        "# leading comment\nfn main()\n    var n i32 := 1 # trailing comment\n    # standalone comment\n    put n\nend fn\n",
        Expect::All,
    ),
    Case::accepted(
        "c++-style line comments on every target",
        concat!(
            "// leading comment\nfn main()\n    var n i32 := 2 // trailing comment\n",
            "    // standalone comment\n    put n\nend fn\n"
        ),
        Expect::All,
    ),
    Case::accepted(
        "block comments on every target",
        "/* leading\n   multi-line */\nfn main()\n    /* inline */ var n i32 := 3\n    put n\nend fn\n",
        Expect::All,
    ),
    // Async: a Rust-target feature (Tokio). C, asm, and JS reject async
    // functions with guidance pointing at foreign blocks (POLY_TUTORIAL.md
    // Part 7, POLY_V2_SUPPORT_MATRIX.md).
    Case::accepted(
        "async fn with await and spawn on rust",
        concat!(
            "async fn compute(x: i32): i32\n    delay(10).await\n    return x * x\nend fn\n\nasync fn mai",
            "n()\n    var a := compute(3).await\n    spawn compute(5)\nend fn\n",
        ),
        Expect::RustOnly,
    ),
    // Command-word symmetry: enum variant names that are reserved command words
    // must also be rejected in pattern position (the arm binding form) so they
    // don't shadow the command form. This is the transpiler-side mirror of the
    // parser test `parser::reserved_add_sub_reject_match_variant_names`.
    Case::accepted(
        "reserved-add-sub enum variant rejected in pattern position",
        concat!(
            "enum E\n    add\n    sub\nend enum\n\n",
            "fn main()\n    match 1\n        E::add, put \"ok\"\n    end match\nend fn\n",
        ),
        Expect::AllReject,
    ),
    // --- named-field enum payload matching (regression: E0164) ------------
    // Variants declared with named fields compile as Rust struct variants,
    // but matches used to render tuple patterns, failing rustc with E0164
    // ("expected tuple struct or tuple variant, found struct variant").
    // Both unqualified and qualified arms must emit `{ field: binding }`.
    Case::with_output(
        "named-field payload match, unqualified arm",
        concat!(
            "enum Badge\n    Level(n: i32)\n    Stars(count: i32)\nend enum\n\nfn main()\n    var b := Ba",
            "dge::Level(3)\n    match b\n        Level(n), put \"level \" + n.to_string()\n        Stars(",
            "count), put \"stars \" + count.to_string()\n        _, put \"none\"\n    end match\nend fn\n",
        ),
        Expect::RustAsm,
        ("rust", "Badge::Level { n: n }"),
    )
    // Goose rejects the `enum` declaration itself, so the vector-capable
    // `RustAsm` default is overridden for these two rows.
    .without_goose(),
    Case::with_output(
        "named-field payload match, qualified arm",
        concat!(
            "enum Badge\n    Level(n: i32)\n    None\nend enum\n\nfn main()\n    var b := Badge::Level(3)",
            "\n    match b\n        Badge::Level(n), put \"level \" + n.to_string()\n        _, put \"non",
            "e\"\n    end match\nend fn\n",
        ),
        Expect::RustAsm,
        ("rust", "Badge::Level { n: n }"),
    )
    .without_goose(),
    // File I/O: redirects and file input are Rust-target features; C, asm,
    // and JS reject them with guidance.
    Case::accepted(
        "file output redirect",
        "fn main()\n    put \"x\" to \"out.txt\"\nend fn\n",
        Expect::RustOnly,
    ),
    Case::accepted(
        "file prepend redirect",
        "fn main()\n    put \"x\" to \"out.txt\" -prepend\nend fn\n",
        Expect::RustOnly,
    ),
    Case::accepted(
        "file input via get from",
        "fn main()\n    var d bytes := get from \"data.bin\" --bytes 8\n    put d.len()\nend fn\n",
        Expect::RustOnly,
    ),
];

#[test]
fn doc_pinned_claims_hold_on_every_target() -> Result<(), Box<dyn std::error::Error>> {
    common::run_matrix(CASES);
    Ok(())
}

#[test]
fn zero_step_rejection_names_the_limitation() -> Result<(), Box<dyn std::error::Error>> {
    // The literal-zero-step refusal must be a clear compile-time error on
    // every target, matching POLY_GRAMMAR.md / POLY_SPEC_v2.md wording.
    let transpiler = poly_transpiler::Transpiler::new();
    let source = "fn main()\n    loop i 0..10 step 0\n        put i\n    end loop\nend fn\n";
    for target in common::TARGETS {
        let err = match transpiler.transpile_target(source, target) {
            Err(err) => err,
            Ok(_) => panic!("literal zero step unexpectedly compiled on `{target}`"),
        };
        assert!(
            err.to_lowercase().contains("zero"),
            "target `{target}`: zero-step error should mention \"zero\", got: {err}"
        );
    }
    Ok(())
}

#[test]
fn reserved_add_sub_reject_value_position_variant_names() -> Result<(), Box<dyn std::error::Error>> {
    // Enum variant names that are reserved command words must be rejected in
    // pattern position by the parser's `reject_reserved_command_name` guard
    // (via `expect_binding_identifier`) before they can be used as that arm's
    // binding. See `parser::reserved_add_sub_reject_match_variant_names`.
    for target in common::TARGETS {
        let source = concat!(
            "enum E\n    add\n    sub\nend enum\n\n",
            "fn main()\n    match 1\n        E::add, put \"ok\"\n    end match\nend fn\n",
        );
        let res =
            poly_transpiler::Transpiler::new().transpile_target(source, &target);
        assert!(
            res.is_err(),
            "target `{target}`: reserved-command enum variant in pattern should be rejected"
        );
        let err = res.unwrap_err();
        assert!(
            err.to_string().contains("reserved command word"),
            "target `{target}`: mismatch: {err:?}"
        );
    }
    Ok(())
}

#[test]
fn async_rejection_names_the_foreign_block_escape_hatch() -> Result<(), Box<dyn std::error::Error>>
{
    // C, asm, JS, and goose must refuse async functions with guidance
    // pointing at a foreign block (POLY_TUTORIAL.md Part 7); rust must
    // accept them.
    let transpiler = poly_transpiler::Transpiler::new();
    let source = concat!(
        "async fn compute(x: i32): i32\n    delay(10).await\n    return x * x\nend fn\n\nfn main()\n ",
        "   var a := compute(3).await\nend fn\n",
    );
    assert!(
        transpiler.transpile_target(source, "rust").is_ok(),
        "async fn should compile on the rust target"
    );
    for (target, block) in [
        ("c", "#c"),
        ("asm", "#asm"),
        ("js", "#js"),
        ("goose", "#goose"),
    ] {
        let err = match transpiler.transpile_target(source, target) {
            Err(err) => err,
            Ok(_) => panic!("async fn unexpectedly compiled on `{target}`"),
        };
        let message = err.to_lowercase();
        assert!(
            message.contains("async") && message.contains(block),
            "target `{target}`: async rejection should mention async and {block}, got: {err}"
        );
    }
    Ok(())
}

#[test]
fn mod_is_the_truncated_remainder_on_every_target() -> Result<(), Box<dyn std::error::Error>> {
    // POLY_V2_SUPPORT_MATRIX.md and AUDIT_ROUND2_REPORT.md pin `mod` to the
    // truncated (sign-of-dividend) remainder on every backend, so
    // `-17 mod 5` must be `-2` everywhere. Goose's native `%` is *Euclidean*
    // (reference §Arithmetic: the result lies in `[0, |b|)` and is never
    // negative), which would yield 3, so the helper delegates to `std.rem`
    // — the documented truncated partner of `/`, verified against the real
    // compiler. The helper must never grow a sign-adjustment clause or
    // fall back to `%`: both produce a floored, non-negative remainder
    // that diverges from rust/c/asm/js for negative operands (3 vs -2).
    let source = "fn main()\n    var a i32 := -17\n    var r i32 := a mod 5\n    put r\nend fn\n";
    let transpiler = poly_transpiler::Transpiler::new();

    // The other four targets lower `mod` to their native remainder
    // operator, which truncates in Rust, C, x86 `idiv`, and JS alike.
    for target in ["rust", "c", "js"] {
        let out = transpiler.transpile_target(source, target)?;
        assert!(
            !out.contains("__poly_mod"),
            "target `{target}`: mod should lower to the native operator, not a helper:\n{out}"
        );
    }
    let asm = transpiler.transpile_target(source, "asm")?;
    assert!(
        asm.contains("idivl") && !asm.contains("__poly_mod"),
        "asm: mod should lower to idiv (truncated remainder in edx):\n{asm}"
    );

    // Goose emits the helper; it must delegate to `std.rem` through the
    // file-level `import std;`, with untyped parameters so the real goose
    // compiler specializes it per operand width and signedness (a typed
    // `a: i32` helper rejected every non-i32 operand with "narrowing and
    // sign changes require an explicit `as`").
    let goose = transpiler.transpile_target(source, "goose")?;
    assert!(
        goose.contains("fn __poly_mod_trunc(a, b) {")
            && goose.contains("return rem(a, b);")
            && goose.contains("import std;"),
        "goose: mod helper should delegate to std.rem (native % is Euclidean, not truncated), got:\n{goose}"
    );
    assert!(
        !goose.contains("r + b") && !goose.contains("a % b"),
        "goose: mod helper must neither adjust the remainder's sign nor use native % (both diverge from the other four targets):\n{goose}"
    );
    Ok(())
}

#[test]
fn runtime_step_sign_dispatch_matches_the_other_targets() -> Result<(), Box<dyn std::error::Error>> {
    // POLY_SPEC_v2 "Step semantics (identical on every target — rust, c,
    // asm, js, goose)": a non-literal step keeps its sign at runtime, so the
    // loop must dispatch on that sign — ascending compares against the upper
    // bound, descending against the lower bound (c and js emit the same
    // dispatch and pin it with `runtime_step_uses_sign_dispatch`). The Goose
    // backend used to reuse the statically-chosen ascending comparison in
    // both branches, so `step s` with `s = -1` ran zero iterations, and a
    // runtime-negative step over ascending bounds never terminated.
    let source = "fn main()\n    var s i32 := -1\n    var acc i32 := 0\n    loop i 10..1 step s\n        acc := acc + i\n    end loop\n    put acc\nend fn\n";
    let transpiler = poly_transpiler::Transpiler::new();
    let goose = transpiler.transpile_target(source, "goose")?;
    assert!(
        goose.contains(
            "while (__poly_step > 0 && (i <= __poly_end)) || (__poly_step < 0 && (i >= __poly_end))"
        ),
        "goose: a runtime step must dispatch on its sign like c/js (descending compares `>=`), got:\n{goose}"
    );
    // The expansion must also declare typed variables: Goose infers an
    // untyped `var i = 10` as `i64`, so `acc = acc + i` with `acc: i32`
    // fails the real `goose check` ("narrowing and sign changes require an
    // explicit `as`"). Typed against the real goose v2026.3 compiler:
    // `poly --check --target goose` runs `goose check` on this output.
    assert!(
        goose.contains("var i: i32 = 10;")
            && goose.contains("var __poly_end: i32 = 1;")
            && goose.contains("var __poly_step: i64 = s;")
            && goose.contains("i += __poly_step as i32;"),
        "goose: step-loop variables must be typed i32 and the step widened to i64 (the real goose compiler rejects the untyped form), got:\n{goose}"
    );
    Ok(())
}
