# Release v1.6.0: New If Statement Syntax

## 🎉 Highlights

We're excited to announce **Poly v1.6.0** with a major syntax improvement: **new if statement syntax**!

### New If Syntax

**Before:** `if x > 0 then ... end if`  
**After:** `if x > 0, ... end if`

This change makes Poly code more concise and familiar to developers from other languages.

## 🚀 What's New

### Syntax Changes
- ✅ If statements now use commas instead of `then`
- ✅ Single-line support: `if x > 0, put x`
- ✅ Clean else-if chains
- ✅ Nested conditions work beautifully

### Examples

```poly
// Simple if
if x > 0,
    put x
end if

// If-else
if x > 0,
    put "positive"
else,
    put "non-positive"
end if

// If-else-if chain
if score >= 90,
    grade = "A"
else if score >= 80,
    grade = "B"
else if score >= 70,
    grade = "C"
else,
    grade = "F"
end if
```

### Transpilation

The new syntax transpiles to idiomatic Rust:

```rust
// Poly
if x > 0,
    put x
else if x < 0,
    put "negative"
else,
    put "zero"
end if

// Transpiles to:
if x > 0 {
    println!("{}", x);
} else if x < 0 {
    println!("{}", "negative");
} else {
    println!("{}", "zero");
}
```

## 📦 What's Included

### New Examples
- `if_else_chains.poly` - Complex if-else-if chains
- `nested_patterns.poly` - Nested conditions
- `sorting_algorithms.poly` - Sorting with classification
- `state_machine.poly` - State machine patterns

### Documentation
- Blog post explaining the syntax change
- Social media posts for announcements
- Updated language specification
- Updated quick reference card

### Transpiler Improvements
- Fixed else-if chain generation
- Added `gen_else_chain` method for recursive handling
- Improved error messages

## 🔄 Migration Guide

To update your existing Poly code:

1. Replace `then` with `,` (comma)
2. Keep `else if` and `else` as-is
3. Keep `end if` to close blocks

### Example Migration

**Before:**
```poly
if temperature > 100 then
    error "Too hot!"
else if temperature < 0 then
    error "Too cold!"
else
    put "Temperature is OK"
end if
```

**After:**
```poly
if temperature > 100,
    error "Too hot!"
else if temperature < 0,
    error "Too cold!"
else,
    put "Temperature is OK"
end if
```

## 📊 Test Results

All 86 core tests passing:
- 24 lexer tests ✅
- 22 parser tests ✅
- 40 transpiler tests ✅

## 🙏 Acknowledgments

Thanks to the community for feedback on the syntax design!

## 📝 Full Changelog

- Changed if statement syntax from `if cond then` to `if cond,`
- Added else-if chain support with proper transpilation
- Added `gen_else_chain` method for recursive else handling
- Added 5 new parser tests for if-else-if edge cases
- Added 5 new transpiler tests for if-else-if transpilation
- Fixed string literal handling in transpiler
- Updated all examples to use new syntax
- Updated all documentation
- Added blog post and social media content

---

**Download:** [Source code (zip)](https://github.com/aj-nelson-0001/poly/archive/refs/tags/v1.6.0.zip) | [Source code (tar.gz)](https://github.com/aj-nelson-0001/poly/archive/refs/tags/v1.6.0.tar.gz)
