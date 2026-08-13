# Poly Language for VS Code

Syntax highlighting and language server integration for the **Poly** programming language.

## Features

- **Syntax highlighting** for `.poly` files (TextMate grammar)
- **Live diagnostics** from the `poly-lsp` language server (lexer, parser, and type-checker errors)
- **Completion** triggered by `.` and `:` (methods, variants, keywords)
- **Hover** documentation
- **Document symbols** (outline / breadcrumbs)
- **`Poly: Restart Language Server`** command

## Requirements

The extension needs the `poly-lsp` binary. Build it from the Poly repository:

~~~bash
cd compiler
cargo build --release -p poly-lsp
# binary: compiler/target/release/poly-lsp
~~~

## Installation

### From source (local development)

1. Copy this `vscode/` directory to a convenient location (or open it as a folder).
2. Install the language client dependency:

~~~bash
cd vscode
npm install
~~~

3. In VS Code: **Run > Start Debugging** (F5) with the "Extension Development Host" launch, or package it:

~~~bash
npx @vscode/vsce package
code --install-extension poly-language-0.1.0.vsix
~~~

### Settings

| Setting | Default | Description |
|---------|---------|-------------|
| `poly.lsp.path` | `poly-lsp` | Path to the `poly-lsp` binary. Set an absolute path if it is not on `PATH` (e.g. `compiler/target/release/poly-lsp`). |
| `poly.lsp.trace` | `off` | LSP trace level (`off`, `messages`, `verbose`). |

If the server binary is not found, the extension shows a warning with a link to this setting.

## Usage

Open any `.poly` file. Diagnostics appear as you type; hover symbols for details and use the outline for navigation.

## Troubleshooting

- **"failed to start language server"** — set `poly.lsp.path` to the full path of the compiled binary and restart (`Poly: Restart Language Server`).
- **No highlighting** — the grammar is loaded on `.poly` files; if you renamed the extension, select *Poly* from the language picker in the bottom-right corner.
