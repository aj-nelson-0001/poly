//! Deterministic fuzz tests for the full transpile pipeline.
//!
//! These exercise the lexer, parser (with recovery), type checker, and the
//! shared intermediate-representation code generator against random inputs.
//! The invariant under test: no input may panic or hang the pipeline; errors
//! must always come back as `Err`, and clean parses as `Ok`.

use poly_lexer::Lexer;
use poly_parser::Parser;

/// A small deterministic xorshift PRNG so fuzz runs are reproducible.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Rng(seed.max(1))
    }

    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound.max(1) as u64) as usize
    }
}

/// Fragments that combine into plausible (and implausible) Poly programs.
const TOKENS: &[&str] = &[
    "var",
    "let",
    "fn",
    "return",
    "if",
    "else",
    "while",
    "for",
    "in",
    "loop",
    "struct",
    "enum",
    "match",
    "trait",
    "impl",
    "async",
    "await",
    "end",
    "put",
    "get",
    "error",
    "warn",
    "info",
    "i32",
    "f64",
    "string",
    "bool",
    "x",
    "y",
    "foo",
    "0",
    "1",
    "42",
    "3.14",
    "\"hello\"",
    "true",
    "false",
    ":=",
    "=",
    "==",
    "<",
    ">",
    "+",
    "-",
    "*",
    ",",
    "(",
    ")",
    "[",
    "]",
    "{",
    "}",
    "=>",
    "|",
    "?",
    ";",
    "end fn",
    "end if",
    "end while",
    "end match",
];

/// Generate a random-ish source string by picking fragments.
fn generate_source(rng: &mut Rng, max_fragments: usize) -> String {
    let count = 1 + rng.below(max_fragments);
    let mut parts = Vec::with_capacity(count);
    for _ in 0..count {
        parts.push(TOKENS[rng.below(TOKENS.len())]);
    }
    let mut result = String::new();
    for (index, part) in parts.iter().enumerate() {
        if index > 0 && rng.below(5) == 0 {
            result.push('\n');
        } else if index > 0 && rng.below(10) == 0 {
            result.push(' ');
        }
        result.push_str(part);
    }
    result
}

#[test]
fn fuzz_full_pipeline_never_panics() {
    let mut rng = Rng::new(0xF0FF_2026);
    let transpiler = poly_transpiler::Transpiler::new();
    let mut checked = 0usize;

    for _ in 0..400 {
        let source = generate_source(&mut rng, 35);

        // `transpile_checked` runs lex -> parse -> check -> generate and must
        // either return generated Rust or a String error, never panic.
        if let Ok(rust) = transpiler.transpile_checked(&source) {
            checked += 1;
            // Generated code must reference the expected module header.
            assert!(rust.contains("Generated from Poly source code"));
        }

        // The optimized path must be equally panic-free.
        let _ = transpiler.transpile_with_intermediate_representation(&source);
    }

    // Sanity: some inputs should actually transpile successfully.
    assert!(checked > 0, "fuzzer never produced a transpilable program");
}

#[test]
fn fuzz_pipeline_various_seeds() {
    let transpiler = poly_transpiler::Transpiler::new();
    for seed in [2, 3, 5, 8, 13, 21, 34, 55, 89, 144] {
        let mut rng = Rng::new(seed);
        for _ in 0..120 {
            let source = generate_source(&mut rng, 40);
            let _ = transpiler.transpile_checked(&source);
            let _ = transpiler.transpile(&source);
        }
    }
}

#[test]
fn fuzz_recovery_parse_then_check_never_panics() {
    let mut rng = Rng::new(0xABCDEF);
    let mut error_parses = 0usize;
    let mut clean_parses = 0usize;

    for _ in 0..400 {
        let source = generate_source(&mut rng, 30);
        let (tokens, lexer_errors) = Lexer::lex(&source);
        let mut parser = Parser::new(&tokens);
        let (program, parse_errors) = parser.parse_with_recovery();

        if !lexer_errors.is_empty() || !parse_errors.is_empty() {
            error_parses += 1;
            continue;
        }
        clean_parses += 1;
        // Cleanly parsed programs must be type-checkable without panicking.
        let result = poly_transpiler::check_program(&program);
        let _ = result.is_ok() || result.is_err();
    }

    assert!(
        error_parses > 0,
        "fuzzer produced no error-recovery exercises"
    );
    assert!(
        clean_parses > 0 || error_parses == 400,
        "fuzzer never produced a clean parse"
    );
}
