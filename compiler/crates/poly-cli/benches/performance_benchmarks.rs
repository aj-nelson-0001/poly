//! Performance benchmarks for the Poly compiler.
//!
//! These benchmarks measure compilation speed and memory usage across
//! various program sizes and complexity levels.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use poly_lexer::Lexer;
use poly_parser::Parser;
use poly_transpiler::Transpiler;

// =============================================================================
// LEXER BENCHMARKS
// =============================================================================

fn bench_lexer_simple(c: &mut Criterion) {
    let source = "var x: i32 = 42\nput x";

    c.bench_function("lexer_simple", |b| {
        b.iter(|| {
            let (tokens, errors) = Lexer::lex(black_box(source));
            black_box((tokens, errors));
        })
    });
}

fn bench_lexer_complex(c: &mut Criterion) {
    let source = r#"
fn fibonacci(n: i32): i32
    if n <= 1
        return n
    end if
    return fibonacci(n - 1) + fibonacci(n - 2)
end fn

var result = fibonacci(10)
put result
"#;

    c.bench_function("lexer_complex", |b| {
        b.iter(|| {
            let (tokens, errors) = Lexer::lex(black_box(source));
            black_box((tokens, errors));
        })
    });
}

fn bench_lexer_large(c: &mut Criterion) {
    let mut source = String::new();
    for i in 0..1000 {
        source.push_str(&format!("var variable_{}: i32 = {}\n", i, i));
    }

    c.bench_function("lexer_large_1000_vars", |b| {
        b.iter(|| {
            let (tokens, errors) = Lexer::lex(black_box(&source));
            black_box((tokens, errors));
        })
    });
}

// =============================================================================
// PARSER BENCHMARKS
// =============================================================================

fn bench_parser_simple(c: &mut Criterion) {
    let source = "var x: i32 = 42\nput x";
    let (tokens, _) = Lexer::lex(source);

    c.bench_function("parser_simple", |b| {
        b.iter(|| {
            let mut parser = Parser::new(black_box(&tokens));
            let result = parser.parse();
            black_box(result);
        })
    });
}

fn bench_parser_complex(c: &mut Criterion) {
    let source = r#"
enum Direction
    North
    South
    East
    West
end enum

struct Point
    var x: f32
    var y: f32
end struct

fn distance(p1: Point, p2: Point): f32
    var dx = p2.x - p1.x
    var dy = p2.y - p1.y
    return (dx * dx + dy * dy)
end fn

fn main()
    var origin = Point { x: 0.0, y: 0.0 }
    var target = Point { x: 3.0, y: 4.0 }
    var dist = distance(origin, target)
    put dist
end fn
"#;
    let (tokens, _) = Lexer::lex(source);

    c.bench_function("parser_complex", |b| {
        b.iter(|| {
            let mut parser = Parser::new(black_box(&tokens));
            let result = parser.parse();
            black_box(result);
        })
    });
}

fn bench_parser_large(c: &mut Criterion) {
    let mut source = String::new();
    for i in 0..100 {
        source.push_str(&format!(
            "fn func_{}(x: i32): i32\n    return x + {}\nend fn\n",
            i, i
        ));
    }
    let (tokens, _) = Lexer::lex(&source);

    c.bench_function("parser_large_100_fns", |b| {
        b.iter(|| {
            let mut parser = Parser::new(black_box(&tokens));
            let result = parser.parse();
            black_box(result);
        })
    });
}

// =============================================================================
// TRANSPILER BENCHMARKS
// =============================================================================

fn bench_transpiler_simple(c: &mut Criterion) {
    let source = "var x: i32 = 42\nput x";

    c.bench_function("transpiler_simple", |b| {
        b.iter(|| {
            let t = Transpiler::new();
            let result = t.transpile(black_box(source));
            black_box(result);
        })
    });
}

fn bench_transpiler_complex(c: &mut Criterion) {
    let source = r#"
enum Shape
    Circle(f32)
    Rectangle(f32, f32)
    Triangle(f32, f32, f32)
end enum

fn area(shape: Shape): f32
    match shape
        Circle(r) => 3.14 * r * r
        Rectangle(w, h) => w * h
        Triangle(a, b, c) =>
            let s = (a + b + c) / 2.0
            return (s * (s - a) * (s - b) * (s - c))
    end match
end fn

fn main()
    var circle = Circle(5.0)
    var rect = Rectangle(4.0, 6.0)
    var tri = Triangle(3.0, 4.0, 5.0)
    
    put "Circle area: "
    put area(circle)
    put "\n"
    
    put "Rectangle area: "
    put area(rect)
    put "\n"
end fn
"#;

    c.bench_function("transpiler_complex", |b| {
        b.iter(|| {
            let t = Transpiler::new();
            let result = t.transpile(black_box(source));
            black_box(result);
        })
    });
}

fn bench_transpiler_large(c: &mut Criterion) {
    let mut source = String::new();
    for i in 0..50 {
        source.push_str(&format!(
            r#"enum Enum_{}
    Variant1
    Variant2(i32)
    Variant3 {{ x: i32, y: i32 }}
end enum

fn process_{}(x: i32): i32
    if x > 0
        return x * {}
    else
        return 0
    end if
end fn

"#,
            i, i, i
        ));
    }

    c.bench_function("transpiler_large_50_enums_fns", |b| {
        b.iter(|| {
            let t = Transpiler::new();
            let result = t.transpile(black_box(&source));
            black_box(result);
        })
    });
}

// =============================================================================
// MEMORY USAGE BENCHMARKS
// =============================================================================

fn bench_memory_usage(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_usage");

    // Small program
    let small = "var x: i32 = 42";
    group.bench_function("small_program", |b| {
        b.iter(|| {
            let t = Transpiler::new();
            let result = t.transpile(black_box(small));
            black_box(result);
        })
    });

    // Medium program
    let medium = r#"
fn fibonacci(n: i32): i32
    if n <= 1
        return n
    end if
    return fibonacci(n - 1) + fibonacci(n - 2)
end fn

fn factorial(n: i32): i32
    if n <= 1
        return 1
    end if
    return n * factorial(n - 1)
end fn

var fib10 = fibonacci(10)
var fact10 = factorial(10)
put fib10
put fact10
"#;
    group.bench_function("medium_program", |b| {
        b.iter(|| {
            let t = Transpiler::new();
            let result = t.transpile(black_box(medium));
            black_box(result);
        })
    });

    // Large program
    let mut large = String::new();
    for i in 0..100 {
        large.push_str(&format!(
            "fn func_{}(x: i32, y: i32): i32\n    return x + y + {}\nend fn\n",
            i, i
        ));
    }
    group.bench_function("large_program", |b| {
        b.iter(|| {
            let t = Transpiler::new();
            let result = t.transpile(black_box(&large));
            black_box(result);
        })
    });

    group.finish();
}

// =============================================================================
// THROUGHPUT BENCHMARKS
// =============================================================================

fn bench_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("throughput");

    // Measure bytes per second
    let small = "var x: i32 = 42";
    group.throughput(criterion::Throughput::Bytes(small.len() as u64));
    group.bench_function("small_throughput", |b| {
        b.iter(|| {
            let t = Transpiler::new();
            let result = t.transpile(black_box(small));
            black_box(result);
        })
    });

    let medium = "fn add(a: i32, b: i32): i32\n    return a + b\nend fn\nvar x = add(1, 2)";
    group.throughput(criterion::Throughput::Bytes(medium.len() as u64));
    group.bench_function("medium_throughput", |b| {
        b.iter(|| {
            let t = Transpiler::new();
            let result = t.transpile(black_box(medium));
            black_box(result);
        })
    });

    group.finish();
}

// =============================================================================
// REGRESSION TESTS
// =============================================================================

fn bench_regression_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("regression_patterns");

    // Nested loops
    let nested_loops = r#"
fn nested_loop()
    for i in 0..100
        for j in 0..100
            var x = i * j
        end for
    end for
end fn
"#;
    group.bench_function("nested_loops", |b| {
        b.iter(|| {
            let t = Transpiler::new();
            let result = t.transpile(black_box(nested_loops));
            black_box(result);
        })
    });

    // Complex pattern matching
    let pattern_matching = r#"
enum Expr
    Num(f32)
    Add(Expr, Expr)
    Mul(Expr, Expr)
end enum

fn eval(e: Expr): f32
    match e
        Num(n) => n
        Add(l, r) => eval(l) + eval(r)
        Mul(l, r) => eval(l) * eval(r)
    end match
end fn
"#;
    group.bench_function("complex_pattern_matching", |b| {
        b.iter(|| {
            let t = Transpiler::new();
            let result = t.transpile(black_box(pattern_matching));
            black_box(result);
        })
    });

    // Deep nesting
    let mut deep_nesting = String::from("fn deep()\n");
    for _ in 0..20 {
        deep_nesting.push_str("    if true\n");
    }
    deep_nesting.push_str("        put 42\n");
    for _ in 0..20 {
        deep_nesting.push_str("    end if\n");
    }
    deep_nesting.push_str("end fn\n");

    group.bench_function("deep_nesting", |b| {
        b.iter(|| {
            let t = Transpiler::new();
            let result = t.transpile(black_box(&deep_nesting));
            black_box(result);
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_lexer_simple,
    bench_lexer_complex,
    bench_lexer_large,
    bench_parser_simple,
    bench_parser_complex,
    bench_parser_large,
    bench_transpiler_simple,
    bench_transpiler_complex,
    bench_transpiler_large,
    bench_memory_usage,
    bench_throughput,
    bench_regression_patterns,
);

criterion_main!(benches);
