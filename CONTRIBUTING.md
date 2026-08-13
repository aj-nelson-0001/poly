# Contributing to Poly

Thank you for your interest in contributing to Poly! This document provides guidelines and information for contributors.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [How to Contribute](#how-to-contribute)
- [Development Setup](#development-setup)
- [Making Changes](#making-changes)
- [Testing](#testing)
- [Pull Request Process](#pull-request-process)
- [Style Guidelines](#style-guidelines)
- [Reporting Issues](#reporting-issues)

## Code of Conduct

Please be respectful and inclusive in all interactions. We are committed to providing a welcoming and constructive environment for everyone.

## How to Contribute

### Ways to Contribute

- **Code**: Implement new features, fix bugs, improve tests
- **Documentation**: Improve docs, add examples, fix typos
- **Testing**: Write tests, report bugs, verify fixes
- **Design**: Suggest syntax improvements, UI/UX enhancements
- **Community**: Help others, answer questions, provide feedback

### Getting Started

1. Fork the repository
2. Clone your fork: `git clone https://github.com/your-username/poly.git`
3. Create a branch: `git checkout -b feature/your-feature`
4. Make your changes
5. Test your changes
6. Commit your changes
7. Push to your fork
8. Create a Pull Request

## Development Setup

### Prerequisites

- Rust (latest stable)
- Git
- A code editor (VS Code recommended)

### Building the Project

~~~bash
# Clone the repository
git clone https://github.com/aj-nelson-0001/poly.git
cd poly

# Build the compiler
cd compiler
cargo build

# Run tests
cargo test --workspace
~~~

### Project Structure

~~~
poly/
├── compiler/
│   ├── crates/
│   │   ├── poly-lexer/      # Tokenizer
│   │   ├── poly-parser/     # AST builder
│   │   ├── poly-transpiler/ # Rust code generator
│   │   └── poly-cli/        # Command-line interface
│   └── Cargo.toml
├── examples/                # Example Poly programs
├── tests/                   # Integration tests
└── docs/                    # Documentation
~~~

## Making Changes

### Branch Naming

Use descriptive branch names:
- `feature/add-pattern-matching`
- `fix/resolve-parser-error`
- `docs/update-tutorial`
- `test/add-edge-case-tests`

### Commit Messages

Write clear, concise commit messages:
- Use imperative mood ("Add feature" not "Added feature")
- Keep the first line under 72 characters
- Reference issues when applicable

Example:
~~~
Add pattern matching support

- Implement match expression parsing
- Add transpilation to Rust match
- Add comprehensive tests

Closes #123
~~~

### Code Style

- Follow Rust style guidelines
- Use `cargo fmt` to format code
- Use `cargo clippy` to check for warnings
- Add comments for complex logic
- Keep functions focused and small

## Testing

### Running Tests

~~~bash
# Run all tests
cargo test --workspace

# Run specific crate tests
cargo test -p poly-parser
cargo test -p poly-transpiler

# Run with output
cargo test -- --nocapture
~~~

### Writing Tests

- Add tests for new features
- Test edge cases
- Test error conditions
- Use descriptive test names

Example:
~~~rust
#[test]
fn test_parse_if_else_if() {
    let source = "if x > 0, put x else if x < 0, put \"neg\" end if";
    let result = parse_source(source);
    assert!(result.is_ok());
}
~~~

### Test Coverage

Aim for high test coverage, especially for:
- Parser edge cases
- Transpiler correctness
- Error handling
- Type checking

## Pull Request Process

### Before Submitting

1. ✅ Code compiles without errors
2. ✅ All tests pass
3. ✅ Code is formatted with `cargo fmt`
4. ✅ No clippy warnings
5. ✅ Documentation is updated (if applicable)
6. ✅ Examples work correctly

### PR Template

~~~markdown
## Description

Brief description of changes

## Type of Change

- [ ] Bug fix
- [ ] New feature
- [ ] Breaking change
- [ ] Documentation update
- [ ] Test improvement

## Testing

- [ ] Tests pass locally
- [ ] Added tests for new functionality
- [ ] Tested with examples

## Checklist

- [ ] Code follows style guidelines
- [ ] Self-review completed
- [ ] Documentation updated
- [ ] No breaking changes (or documented)
~~~

### Review Process

1. Submit your PR
2. Wait for CI to pass
3. Address review feedback
4. Get approval from maintainers
5. Merge when approved

## Style Guidelines

### Rust Code

- Use meaningful variable names
- Add doc comments for public items
- Handle errors appropriately
- Avoid unwrap() in production code
- Use early returns for clarity

### Poly Code

- Use the new if syntax: `if condition, ... end if`
- Add comments for complex logic
- Use meaningful function names
- Keep functions focused

### Documentation

- Use clear, concise language
- Include code examples
- Keep formatting consistent
- Use tilde fences for code blocks: write three tilde characters followed by an optional language label, such as `poly`, and close with three tilde characters
- Leave single backticks available for inline code references
- Use the current Poly declaration syntax: `var name Type := value` or `var name := value`
- Update table of contents
- Run `python3 scripts/check_markdown.py` and `python3 scripts/check_poly_examples.py` before submitting documentation changes

## Reporting Issues

### Bug Reports

Include:
- Poly version
- Operating system
- Steps to reproduce
- Expected behavior
- Actual behavior
- Error messages (if any)

### Feature Requests

Include:
- Description of the feature
- Use case
- Proposed syntax (if applicable)
- Benefits

### Security Issues

For security issues, please email: security@poly-lang.dev

## Questions?

- Open a discussion: https://github.com/aj-nelson-0001/poly/discussions
- Join our Discord: [link]
- Check the docs: [docs link]

---

Thank you for contributing to Poly! 🎉
