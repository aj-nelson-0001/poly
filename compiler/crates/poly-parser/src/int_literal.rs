//! Radix-aware parsing of Poly integer literals.
//!
//! The lexer keeps an integer literal as its *exact source text*, so
//! `IntLiteral` can hold `"42"`, `"010"`, `"0xFF"`, `"0b1010"`, `"0o755"` or
//! `"-0x10"`. Every later stage that needs an actual number — constant
//! folding, i32/i64 width classification, loop-step checks, array lengths and
//! each backend's emitter — must convert that text with the rules below.
//!
//! The rules are intentionally *not* Rust's `str::parse`, which only accepts
//! plain decimal. Using it scattered across the compiler produced a cluster of
//! bugs: the C backend re-interpreted `010` as octal 8, the JS backend emitted
//! it verbatim (a strict-mode `SyntaxError`), the assembly backend rejected
//! `0xFF` outright, and `0x0` slipped past the type checker's "step must not
//! be zero" guard.
//!
//! # Grammar
//!
//! ```text
//! literal := ['-' | '+'] body
//! body    := radix-prefix? digits
//! prefix  := '0x' | '0X'  (base 16)
//!          | '0b' | '0B'  (base 2)
//!          | '0o' | '0O'  (base 8)
//! ```
//!
//! Decimal digits with leading zeros are **decimal** — `010` is ten, exactly
//! as the Rust and assembly targets have always treated it. There is no
//! digit-separator (`1_000`) syntax in Poly.

/// Radix prefixes recognised in a literal body, longest match wins because
/// each prefix is exactly two characters and they are mutually exclusive.
const RADIX_PREFIXES: [(&str, u32); 6] = [
    ("0x", 16),
    ("0X", 16),
    ("0b", 2),
    ("0B", 2),
    ("0o", 8),
    ("0O", 8),
];

/// Split an optional radix prefix from a literal body.
///
/// Returns `(radix, digits)`. Plain decimal has no prefix, so this always
/// succeeds — callers reject the literal by parsing, not by splitting.
fn split_radix(text: &str) -> (u32, &str) {
    for (prefix, radix) in RADIX_PREFIXES {
        if let Some(rest) = text.strip_prefix(prefix) {
            return (radix, rest);
        }
    }
    (10, text)
}

/// Parse a Poly integer literal into its numeric value.
///
/// Returns `None` when the text is not a complete, well-formed literal (for
/// example `"0x"`, `"0b2"`, `""`) or when the magnitude does not fit in
/// `i128`. Callers that must tolerate unparseable text should keep the
/// original source text rather than guessing a value.
pub fn parse_int_literal(text: &str) -> Option<i128> {
    let (negative, body) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text.strip_prefix('+').unwrap_or(text)),
    };
    let (radix, digits) = split_radix(body);
    if digits.is_empty() {
        return None;
    }
    let magnitude = u128::from_str_radix(digits, radix).ok()?;
    let magnitude = i128::try_from(magnitude).ok()?;
    Some(if negative { -magnitude } else { magnitude })
}

/// Parse a Poly integer literal that must fit in `i64`.
///
/// This is the width-classification helper used by the backends: `Ok(v)`
/// means the literal is in `i64` range, `None` means "not a literal we can
/// represent" *or* "out of `i64` range" — the distinction never matters,
/// because both are handled by the same "treat it as wide" branch.
pub fn parse_int_literal_i64(text: &str) -> Option<i64> {
    i64::try_from(parse_int_literal(text)?).ok()
}

/// Parse a Poly integer literal that must fit in `i32`.
pub fn parse_int_literal_i32(text: &str) -> Option<i32> {
    i32::try_from(parse_int_literal(text)?).ok()
}

/// Render a Poly integer literal as plain decimal text.
///
/// Returns `None` when the literal cannot be parsed, so callers can fall back
/// to the original source text instead of silently inventing a value.
pub fn normalize_int_literal(text: &str) -> Option<String> {
    Some(parse_int_literal(text)?.to_string())
}

/// Render a Poly integer literal as plain decimal text for emission, keeping
/// the original text when it cannot be parsed.
///
/// Backends use this so that every target writes the *same number*: C must
/// not read `010` back as octal, and JS must not see a strict-mode octal
/// literal. Radix prefixes are already understood by the Rust target but not
/// by C11, so they are normalised away too.
pub fn int_literal_text(text: &str) -> String {
    normalize_int_literal(text).unwrap_or_else(|| text.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_decimal() {
        assert_eq!(parse_int_literal("42"), Some(42));
        assert_eq!(parse_int_literal("0"), Some(0));
        assert_eq!(parse_int_literal("+7"), Some(7));
        assert_eq!(parse_int_literal("-7"), Some(-7));
    }

    #[test]
    fn leading_zeros_are_decimal_not_octal() {
        assert_eq!(parse_int_literal("010"), Some(10));
        assert_eq!(parse_int_literal("007"), Some(7));
        assert_eq!(parse_int_literal("08"), Some(8));
        assert_eq!(normalize_int_literal("010").as_deref(), Some("10"));
    }

    #[test]
    fn parses_radix_prefixes() {
        assert_eq!(parse_int_literal("0xFF"), Some(255));
        assert_eq!(parse_int_literal("0Xff"), Some(255));
        assert_eq!(parse_int_literal("0b1010"), Some(10));
        assert_eq!(parse_int_literal("0B1010"), Some(10));
        assert_eq!(parse_int_literal("0o755"), Some(493));
        assert_eq!(parse_int_literal("0O755"), Some(493));
        assert_eq!(parse_int_literal("-0x10"), Some(-16));
    }

    #[test]
    fn normalises_radix_literals_to_decimal() {
        assert_eq!(normalize_int_literal("0xFF").as_deref(), Some("255"));
        assert_eq!(normalize_int_literal("0b1010").as_deref(), Some("10"));
        assert_eq!(normalize_int_literal("0o755").as_deref(), Some("493"));
        assert_eq!(normalize_int_literal("-0x10").as_deref(), Some("-16"));
    }

    #[test]
    fn rejects_malformed_literals() {
        assert_eq!(parse_int_literal(""), None);
        assert_eq!(parse_int_literal("-"), None);
        assert_eq!(parse_int_literal("+"), None);
        assert_eq!(parse_int_literal("0x"), None);
        assert_eq!(parse_int_literal("0b2"), None);
        assert_eq!(parse_int_literal("0o8"), None);
        assert_eq!(parse_int_literal("0xG"), None);
        assert_eq!(normalize_int_literal("0x"), None);
    }

    #[test]
    fn unparseable_text_is_kept_verbatim() {
        assert_eq!(int_literal_text("0xG"), "0xG");
        assert_eq!(int_literal_text("0xFF"), "255");
    }

    #[test]
    fn width_helpers_agree_with_the_value() {
        assert_eq!(parse_int_literal_i32("0xFF"), Some(255));
        assert_eq!(parse_int_literal_i32("0x100000000"), None);
        assert_eq!(parse_int_literal_i64("0x100000000"), Some(4_294_967_296));
        assert_eq!(parse_int_literal_i64("-0x8000000000000000"), Some(i64::MIN));
        // Out-of-i64-range magnitudes are reported as "not representable",
        // which every caller already treats as "wide".
        assert_eq!(parse_int_literal_i64("9223372036854775808"), None);
        assert_eq!(parse_int_literal_i64("0xFFFFFFFFFFFFFFFF"), None);
    }
}
