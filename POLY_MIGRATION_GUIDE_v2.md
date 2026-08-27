# Poly 2.0 Preview Migration Guide

Poly 2.0 preview keeps the compact orchestration syntax and adds explicit target selection. Rust remains the default target; C is available as a documented C11 subset.

## Target Selection

Use the target flag when a source file contains definitions for more than one backend:

~~~poly
#rust
fn double_value(x: i32) -> i32 { x * 2 }
#endrust

#c
int double_value(int x) { return x * 2; }
#endc

var result i32 := double_value(21)
put result
~~~

~~~bash
poly --target rust program.poly
poly --target c program.poly
poly --target c --check program.poly
poly --target c --emit-c program.poly
~~~

Only the selected foreign block is emitted. A `#cpp` block is reserved and causes an explicit unsupported-backend error until a C++ backend is designed. Optional `extern rust fn ...` and `extern c fn ...` declarations let migrated programs opt into Poly-side foreign-call checks.

## Foreign Blocks

Move complex target-specific declarations out of Poly syntax and into a top-level foreign block. The contents are opaque to Poly and are validated by the native compiler.

~~~poly fragment
#rust
use std::collections::HashMap;

fn process(items: &[i32]) -> i32 {
    items.iter().sum()
}
#endrust

var values i32 := 3
put process(&[values, 0, 0])
~~~

Foreign block delimiters must be on their own lines. Blocks cannot appear inside Poly functions, loops, or other nested bodies. Foreign blocks should contain declarations and definitions at target-language scope; Poly always owns the generated entry point.

## Syntax Changes Since v1.8

The v1.8 syntax migration remains applicable:

- Use `loop i 0..10`, without the legacy `loop:` form.
- Use `name := value` for assignment.
- Use `put value to "file" -append` for append output.
- Use `get from "file"` for file input.

See [POLY_MIGRATION_GUIDE_v1.8.md](POLY_MIGRATION_GUIDE_v1.8.md) for the complete v1.7 to v1.8 syntax table.

## C Preview Scope

The C backend currently supports scalar variables, primitive types, plain structs, simple Poly functions, arithmetic, conditions, loops, and stdout/stderr output through `put`, `error`, `warn`, and `info`.

File redirects, stdin, vectors, tuples, pattern matching, closures, async, and complex Poly types are rejected with diagnostics. Put those operations in a `#c` helper until their C representation is specified.
