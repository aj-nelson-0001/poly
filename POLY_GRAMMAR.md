# Poly Language Grammar v2 Preview

**Status:** Current preview grammar for Poly 2.0.0-preview.2

This document describes the syntax accepted by the current lexer and parser. It is intentionally a compact grammar, not a promise that every parsed construct is supported by every target backend.

## Notation

- `::=` defines a production.
- `|` separates alternatives.
- `[ ... ]` is optional.
- `{ ... }` repeats zero or more times.
- Quoted text is a literal token.
- `identifier`, `integer`, and `string` are lexical categories.

## Lexical Structure

~~~
program       ::= { top_level_item } EOF

top_level_item ::= statement | foreign_block | extern_function_declaration

foreign_block ::= "#rust" foreign_text "#endrust"
                | "#c" foreign_text "#endc"
                | "#asm" foreign_text "#endasm"
                | "#cpp" foreign_text "#endcpp"

extern_function_declaration ::= "extern" foreign_target "fn" identifier
                                "(" [ parameters ] ")" [ ":" type ]
foreign_target ::= "rust" | "c" | "asm"

// Foreign markers are line-oriented. Leading whitespace is allowed, but the
// marker must occupy the line; foreign_text is opaque to the Poly lexer.
foreign_text  ::= { any_source_character }

comment       ::= "#" { any_character_except_newline }
                | "//" { any_character_except_newline }
                | "/*" { any_character } "*/"

identifier    ::= identifier_start { identifier_continue }
identifier_start ::= letter | "_"
identifier_continue ::= letter | digit | "_"

integer       ::= decimal | hexadecimal | binary | octal
float         ::= digit { digit } "." digit { digit } [ exponent ]
string        ::= '"' { string_character | escape } '"'
unicode_string ::= "unicode" string
unicode_char  ::= "unicode" "'" character "'"
boolean       ::= "true" | "false"
~~~

Outside foreign blocks, `#` starts a Poly comment. Inside a foreign block every character is copied as target-language text until a valid end marker is reached.

## Statements

~~~
statement ::= variable_declaration
            | let_declaration
            | const_declaration
            | assignment
            | function_declaration
            | struct_declaration
            | enum_declaration
            | trait_declaration
            | impl_declaration
            | module_declaration
            | use_declaration
            | type_declaration
            | if_statement
            | while_statement
            | loop_statement
            | return_statement
            | break_statement
            | continue_statement
            | put_statement
            | diagnostic_statement
            | expression_statement

variable_declaration ::= "var" identifier [ type ] ":=" [ expression ]
let_declaration      ::= "let" identifier [ ":" type ] ":=" expression
const_declaration    ::= "const" identifier ":=" expression
assignment           ::= assignment_target ":=" expression
assignment_target    ::= identifier { "." identifier | "." integer | "[" expression "]" }

function_declaration ::= [ "async" ] "fn" identifier [ generic_parameters ]
                         "(" [ parameters ] ")" [ ":" type ] function_body
function_body        ::= "{" { statement } "}"
                       | { statement } "end" "fn"
                       | "end" "fn"

struct_declaration   ::= "struct" identifier { struct_field } "end" "struct"
enum_declaration     ::= "enum" identifier { enum_variant } "end" "enum"
trait_declaration    ::= "trait" identifier { statement } "end" "trait"
impl_declaration     ::= "impl" { statement } "end" "impl"
module_declaration   ::= "module" identifier { statement } "end" "module"
use_declaration      ::= "use" use_path
type_declaration     ::= "type" identifier "=" type

return_statement     ::= "return" [ expression ]
break_statement      ::= "break" [ expression ]
continue_statement   ::= "continue"
~~~

Foreign blocks and explicit foreign function declarations are only valid as `top_level_item`s. The parser rejects them inside functions, loops, modules, or other nested Poly blocks. `extern rust fn ...` and `extern c fn ...` declarations are checker-only metadata and are not emitted.

## Types

~~~
type ::= primitive_type
       | identifier
       | "ptr" type
       | "Vec" "<" type ">"
       | "Option" "<" type ">"
       | "Result" "<" type "," type ">"
       | "Map" "<" type "," type ">"
       | "Set" "<" type ">"
       | "(" type { "," type } ")"
       | "[" type ";" expression "]"
       | function_type

primitive_type ::= "bool" | "char" | "uchar" | "string" | "ustring"
                 | "byte" | "bytes"
                 | "i8" | "u8" | "i16" | "u16"
                 | "i32" | "u32" | "i64" | "u64"
                 | "i128" | "u128" | "isize" | "usize"
                 | "f32" | "f64"

function_type ::= "|" [ parameters ] "|" type
~~~

The Rust backend supports the broader Poly type system. The C preview intentionally supports only primitive/scalar mappings, C-compatible plain structs, and simple pointers/references as documented in [POLY_C_BLOCKS.md](POLY_C_BLOCKS.md).

## Expressions

~~~
expression ::= literal
             | identifier
             | expression binary_operator expression
             | unary_operator expression
             | expression "(" [ arguments ] ")"
             | expression "." identifier
             | expression "." integer
             | expression "[" expression "]"
             | expression "as" type
             | "(" expression ")"
             | array_literal
             | tuple_literal
             | closure
             | if_expression
             | match_expression
             | range_expression
             | get_expression

literal ::= integer | float | string | unicode_string | unicode_char | boolean
array_literal ::= "[" [ arguments ] "]"
tuple_literal ::= "(" expression "," expression { "," expression } ")"
arguments ::= expression { "," expression }
closure ::= "|" [ parameters ] "|" expression

binary_operator ::= "+" | "-" | "*" | "/" | "mod"
                 | "=" | "!=" | "<" | ">" | "<=" | ">="
                 | "and" | "or" | "xor" | "bitand" | "bitor"
                 | "shift" ("left" | "right")
                 | "<<" | ">>"
unary_operator ::= "-" | "not" | "bitnot" | "*"
~~~

`=` is the equality operator. `:=` is assignment. Logical, bitwise, and
remainder operators are spelled as keywords: `and`, `or`, `xor`, `mod`,
`bitand`, `bitor`, `bitnot`, and `shift left` / `shift right`. The `<<` and
`>>` symbols remain valid alternative shift syntax. The retired symbol
spellings `&&`, `||`, `!`, `^`, `%`, `&`, `|`, and `~` are tokenized only to
provide migration diagnostics and are rejected by the parser.

## Control Flow

~~~
if_statement  ::= "if" expression { statement }
                 { "else" "if" expression { statement } }
                 [ "else" { statement } ] "end" "if"

while_statement ::= "while" expression { statement } "end" "while"

loop_statement ::= "loop" "end" "loop"
                 | "loop" identifier loop_source { statement } "end" "loop"
                 | "loop" "(" identifiers ")" "in" expression { statement } "end" "loop"

loop_source   ::= "in" expression | range_list
range_list    ::= range_part { "," range_part }
range_part    ::= expression ".." expression [ "step" expression ]
                 | expression

match_expression ::= "match" expression { pattern "," expression } "end" "match"
~~~

Poly loop ranges include both endpoints. A negative step selects descending iteration; a zero step is rejected by semantic checking.

## I/O

~~~
put_statement       ::= "put" expression [ "to" expression [ "-append" ] ]
diagnostic_statement ::= ( "error" | "warn" | "info" ) expression

get_expression      ::= "get" [ "unicode" string ] { get_flag }
                    | "get" "from" expression { get_flag }
get_flag            ::= "--timeout" expression
                      | "--default" expression
                      | "--mask" expression
                      | "--as" type
                      | "--until" expression
                      | "--bytes" expression
                      | "with" "validate" closure
                      | "with" "complete" expression
                      | "with" "encoding" expression
~~~

`put` always writes a trailing newline. `error`, `warn`, and `info` write to stderr with `[ERROR]`, `[WARN]`, and `[INFO]` prefixes. File output is implemented by the Rust backend and currently rejected by the C backend.

## Target Selection

~~~bash
poly --target rust program.poly
poly --target c program.poly
poly --target c --check program.poly
poly --target c --emit-c program.poly
poly --target asm --emit-asm program.poly
poly --target asm program.poly
~~~

`#rust` blocks are selected for Rust, `#c` blocks for C, and `#asm` blocks for
the Linux x86-64 assembly target. A source containing `#cpp` is rejected
explicitly because no C++ backend exists yet.
