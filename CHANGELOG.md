# Changelog

All notable changes to the Poly language compiler will be documented in this file.

## [1.5.0] - 2026-08-08

### Added

#### CLI Features
- Added `--format` flag to format output with rustfmt
- Added `--diff` flag to show diff between unformatted and formatted code
- Added `--watch` flag to watch file and re-transpile on changes
- Added `--check` flag to validate code and verify Rust compilation

#### Transpiler Improvements
- Added type mapping for Poly types to Rust types:
  - `ustring` / `string` → `String`
  - `uchar` → `char`
  - `byte` → `u8`
  - `bytes` → `Vec<u8>`
- Added file write operations (`put "content" > "file"`)
- Added file append operations (`put "content" >> "file"`)
- Added `split()` method support with `.collect::<Vec<_>>()`
- Added `open()` function support for file operations
- Added placeholder support for `eof()` and `get_line()` methods
- Fixed loop range variable naming for collection iteration
- Fixed match arm codegen to remove extra semicolons
- Fixed semicolons in `gen_statement_str` for statements in inline contexts

#### Examples
- Added `structs_enums.poly` example for struct and enum usage
- Added `closures.poly` example for closure syntax
- Added `pattern_matching.poly` example for match expressions
- Fixed `prime_numbers.poly` to use correct loop variable (`i` instead of `num`)

#### Documentation
- Updated README with new CLI options
- Created `.freebuff-prefs.md` for user preferences

### Fixed
- Fixed `break` statement generating `/* statement */` instead of `break;`
- Fixed `return` statements missing semicolons in generated Rust code
- Fixed `put` statements missing semicolons in inline contexts
- Fixed file write operations not being transpiled correctly
- Fixed string type mapping (`ustring` → `String`)
- Fixed loop variable naming for collection iteration

### Testing
- Added unit tests for file write operations
- Added unit tests for type mapping
- Added unit tests for split with collect
- Added comprehensive test suite for all example files
- All 84+ tests passing

## [1.4.0] - 2026-08-07

### Added
- Initial implementation of the Poly language compiler
- Lexer, parser, transpiler, and CLI
- Basic I/O operations (put, get)
- Error handling with Result types
- Loop ranges (SuperBASIC-inspired)
- Struct and enum support
- Match expressions
- Closures

### Fixed
- Various parser and lexer issues
- Error recovery improvements
- Integration tests

---

*This changelog follows the [Keep a Changelog](https://keepachangelog.com/) format.*
