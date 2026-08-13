# Poly Language Update: New If Statement Syntax

**Date:** August 9, 2026  
**Author:** Poly Team

## Introduction

We're excited to announce a significant syntax improvement in the Poly language: the new if statement syntax! Starting with this release, if statements now use a comma instead of the `then` keyword, making the syntax cleaner and more concise.

## Before and After

### Old Syntax
~~~poly fragment
if x > 0 then
    put x
else if x < 0 then
    put "negative"
else
    put "zero"
end if
~~~

### New Syntax
~~~poly
if x > 0,
    put x
else if x < 0,
    put "negative"
else,
    put "zero"
end if
~~~

## Key Changes

1. **Comma replaces `then`**: The `then` keyword is no longer needed. Simply use a comma after the condition.

2. **Single-line support**: For simple conditions, you can write:
   ~~~poly fragment
   if x > 0, put x
   ~~~

3. **Else-if chains work seamlessly**:
   ~~~poly
   if score >= 90,
       set grade to "A"
   else if score >= 80,
       set grade to "B"
   else if score >= 70,
       set grade to "C"
   else,
       set grade to "F"
   end if
   ~~~

4. **Nested conditions**:
   ~~~poly
   if x > 0,
       if y > 0,
           put "both positive"
       end if
   end if
   ~~~

## Why the Change?

The comma syntax offers several advantages:

- **More concise**: Removes unnecessary keyword
- **Familiar**: Similar to other languages like Swift and Rust
- **Cleaner**: Reduces visual clutter in code
- **Consistent**: Aligns with modern language design trends

## Transpilation

The new syntax transpiles to idiomatic Rust:

~~~rust
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
~~~

## Examples

### Grade Calculator
~~~poly
fn calculate_grade(score: i32): string
    if score >= 90,
        return "A+"
    else if score >= 80,
        return "A"
    else if score >= 70,
        return "B"
    else if score >= 60,
        return "C"
    else,
        return "F"
    end if
end fn
~~~

### Temperature Classifier
~~~poly
fn classify_temperature(temp: f64): string
    if temp > 40.0,
        return "Extremely Hot"
    else if temp > 30.0,
        return "Hot"
    else if temp > 20.0,
        return "Warm"
    else if temp > 10.0,
        return "Cool"
    else,
        return "Cold"
    end if
end fn
~~~

### Nested Conditions
~~~poly
fn categorize_number(n: i32): string
    if n > 0,
        if n % 2 = 0,
            return "Positive Even"
        else,
            return "Positive Odd"
        end if
    else if n < 0,
        if n % 2 = 0,
            return "Negative Even"
        else,
            return "Negative Odd"
        end if
    else,
        return "Zero"
    end if
end fn
~~~

## Migration Guide

To update your existing Poly code:

1. Replace `then` with `,` (comma)
2. Keep `else if` and `else` as-is
3. Keep `end if` to close blocks

### Example Migration

**Before:**
~~~poly fragment
if temperature > 100 then
    error "Too hot!"
else if temperature < 0 then
    error "Too cold!"
else
    put "Temperature is OK"
end if
~~~

**After:**
~~~poly
if temperature > 100,
    error "Too hot!"
else if temperature < 0,
    error "Too cold!"
else,
    put "Temperature is OK"
end if
~~~

## What's Next?

We're continuing to improve the Poly language with:

- Pattern matching enhancements
- More expression types
- Better error messages
- Performance optimizations

## Feedback

We'd love to hear your thoughts on the new syntax! Join our community:

- GitHub: [poly-lang/poly](https://github.com/poly-lang/poly)
- Discord: [Poly Community](https://discord.gg/poly-lang)
- Twitter: [@PolyLang](https://twitter.com/PolyLang)

---

*Happy coding with Poly!*
