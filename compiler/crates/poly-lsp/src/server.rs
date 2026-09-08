//! The Poly language server.
//!
//! Implements a pragmatic subset of the Language Server Protocol: full-text
//! document synchronization, diagnostics from every compiler phase, keyword
//! and symbol completion, hover information, and document symbols.  The
//! server is deliberately dependency-free: JSON-RPC framing and JSON values
//! are handled by the in-tree `json` module.

use std::collections::HashMap;

use poly_lexer::Lexer;
use poly_parser::Parser;

use crate::json::Json;

/// Poly keywords surfaced by the completion provider.
const COMPLETION_KEYWORDS: &[&str] = &[
    "var", "let", "const", "fn", "return", "if", "else", "end", "while", "for", "in", "loop",
    "struct", "enum", "match", "trait", "impl", "async", "await", "unsafe", "pub", "module", "use",
    "type", "as", "try", "break", "continue", "put", "get", "error", "warn", "info",
];

/// The running language server.
#[derive(Default)]
pub struct Server {
    /// Documents currently open in the editor, keyed by URI.
    documents: HashMap<String, String>,
    /// Whether the client has sent `initialize` and `initialized`.
    initialized: bool,
    /// Whether `shutdown` has been requested (the next `exit` stops the loop).
    shutdown_requested: bool,
}

/// The result of processing one inbound JSON-RPC message.
pub struct DispatchResult {
    /// Outbound messages to write to stdout.
    pub outputs: Vec<Json>,
    /// Whether the server should exit its loop.
    pub should_exit: bool,
}

impl Server {
    /// Create an empty server.
    pub fn new() -> Self {
        Self::default()
    }

    /// Process a decoded JSON-RPC message and produce responses.
    ///
    /// Returns `(outputs, should_exit)`.  `outputs` may contain both
    /// responses and notifications (e.g. `publishDiagnostics`).
    pub fn dispatch(&mut self, message: &Json) -> DispatchResult {
        let method = match message.get_str("method") {
            Some(method) => method,
            None => {
                // A response to one of our (never sent) requests; ignore.
                return DispatchResult {
                    outputs: Vec::new(),
                    should_exit: false,
                };
            }
        };

        match method {
            "initialize" => self.handle_initialize(message),
            "initialized" => {
                self.initialized = true;
                DispatchResult {
                    outputs: Vec::new(),
                    should_exit: false,
                }
            }
            "shutdown" => {
                self.shutdown_requested = true;
                DispatchResult {
                    outputs: vec![json_response(message, Json::Null)],
                    should_exit: false,
                }
            }
            "exit" => DispatchResult {
                outputs: Vec::new(),
                should_exit: true,
            },
            "$/cancelRequest" => DispatchResult {
                outputs: Vec::new(),
                should_exit: false,
            },
            "textDocument/didOpen" => self.handle_did_open(message),
            "textDocument/didChange" => self.handle_did_change(message),
            "textDocument/didSave" => self.handle_did_save(message),
            "textDocument/didClose" => self.handle_did_close(message),
            "textDocument/completion" => self.handle_completion(message),
            "textDocument/hover" => self.handle_hover(message),
            "textDocument/documentSymbol" => self.handle_document_symbol(message),
            _ => DispatchResult {
                outputs: Vec::new(),
                should_exit: false,
            },
        }
    }

    // ---------------------------------------------------------------------
    // Lifecycle
    // ---------------------------------------------------------------------

    fn handle_initialize(&mut self, message: &Json) -> DispatchResult {
        let result = Json::obj(vec![
            (
                "capabilities",
                Json::obj(vec![
                    (
                        "textDocumentSync",
                        Json::num(1.0), // Full sync
                    ),
                    (
                        "completionProvider",
                        Json::obj(vec![(
                            "triggerCharacters",
                            Json::Array(vec![Json::str("."), Json::str(":")]),
                        )]),
                    ),
                    ("hoverProvider", Json::Bool(true)),
                    ("documentSymbolProvider", Json::Bool(true)),
                ]),
            ),
            (
                "serverInfo",
                Json::obj(vec![("name", Json::str("poly-lsp"))]),
            ),
        ]);
        DispatchResult {
            outputs: vec![json_response(message, result)],
            should_exit: false,
        }
    }

    // ---------------------------------------------------------------------
    // Text synchronization
    // ---------------------------------------------------------------------

    fn handle_did_open(&mut self, message: &Json) -> DispatchResult {
        let params = message.get("params").cloned().unwrap_or(Json::Null);
        let document = params.get("textDocument").cloned().unwrap_or(Json::Null);
        let uri = document.get_str("uri");
        let text = document.get_str("text");
        if let (Some(uri), Some(text)) = (uri, text) {
            self.documents.insert(uri.to_string(), text.to_string());
            return self.publish_diagnostics_for(uri);
        }
        DispatchResult {
            outputs: Vec::new(),
            should_exit: false,
        }
    }

    fn handle_did_change(&mut self, message: &Json) -> DispatchResult {
        let params = message.get("params").cloned().unwrap_or(Json::Null);
        let document = params.get("textDocument").cloned().unwrap_or(Json::Null);
        let uri = document.get_str("uri");
        // Full sync: the latest content change replaces the document.
        let text = params
            .get("contentChanges")
            .and_then(|changes| match changes {
                Json::Array(items) => items.last().and_then(|change| change.get_str("text")),
                _ => None,
            });
        if let (Some(uri), Some(text)) = (uri, text) {
            self.documents.insert(uri.to_string(), text.to_string());
            return self.publish_diagnostics_for(uri);
        }
        DispatchResult {
            outputs: Vec::new(),
            should_exit: false,
        }
    }

    fn handle_did_save(&mut self, message: &Json) -> DispatchResult {
        let params = message.get("params").cloned().unwrap_or(Json::Null);
        let document = params.get("textDocument").cloned().unwrap_or(Json::Null);
        if let Some(uri) = document.get_str("uri") {
            return self.publish_diagnostics_for(uri);
        }
        DispatchResult {
            outputs: Vec::new(),
            should_exit: false,
        }
    }

    fn handle_did_close(&mut self, message: &Json) -> DispatchResult {
        let params = message.get("params").cloned().unwrap_or(Json::Null);
        let document = params.get("textDocument").cloned().unwrap_or(Json::Null);
        if let Some(uri) = document.get_str("uri") {
            self.documents.remove(uri);
        }
        DispatchResult {
            outputs: Vec::new(),
            should_exit: false,
        }
    }

    // ---------------------------------------------------------------------
    // Diagnostics
    // ---------------------------------------------------------------------

    fn publish_diagnostics_for(&self, uri: &str) -> DispatchResult {
        let diagnostics = match self.documents.get(uri) {
            Some(source) => collect_diagnostics(source),
            None => Vec::new(),
        };
        let notification = Json::obj(vec![
            ("jsonrpc", Json::str("2.0")),
            ("method", Json::str("textDocument/publishDiagnostics")),
            (
                "params",
                Json::obj(vec![
                    ("uri", Json::str(uri)),
                    ("diagnostics", Json::Array(diagnostics)),
                ]),
            ),
        ]);
        DispatchResult {
            outputs: vec![notification],
            should_exit: false,
        }
    }

    // ---------------------------------------------------------------------
    // Completion
    // ---------------------------------------------------------------------

    fn handle_completion(&mut self, message: &Json) -> DispatchResult {
        let params = message.get("params").cloned().unwrap_or(Json::Null);
        let uri = params
            .get("textDocument")
            .and_then(|doc| doc.get_str("uri"));
        let position = params.get("position").cloned().unwrap_or(Json::Null);
        let line = position.get_f64("line").unwrap_or(0.0) as usize;
        let character = position.get_f64("character").unwrap_or(0.0) as usize;

        let mut items: Vec<Json> = COMPLETION_KEYWORDS
            .iter()
            .map(|keyword| {
                Json::obj(vec![
                    ("label", Json::str(*keyword)),
                    ("kind", Json::num(14.0)), // Keyword
                ])
            })
            .collect();

        if let Some(uri) = uri {
            if let Some(source) = self.documents.get(uri) {
                let line_text = source.lines().nth(line).unwrap_or("");
                // LSP `character` is measured in UTF-16 code units; walk the
                // line counting units so non-ASCII text does not shift the
                // completion prefix.
                let mut prefix_chars = String::new();
                let mut units = 0usize;
                for ch in line_text.chars() {
                    if units >= character {
                        break;
                    }
                    prefix_chars.push(ch);
                    units += ch.len_utf16();
                }
                let prefix = prefix_chars
                    .split(|c: char| !c.is_alphanumeric() && c != '_')
                    .next_back()
                    .unwrap_or("")
                    .to_string();
                for symbol in document_symbols(source) {
                    if symbol
                        .name
                        .to_lowercase()
                        .starts_with(&prefix.to_lowercase())
                    {
                        items.push(Json::obj(vec![
                            ("label", Json::str(symbol.name)),
                            ("kind", Json::num(symbol.kind as f64)),
                            ("detail", Json::str(symbol.detail)),
                        ]));
                    }
                }
            }
        }

        let response = Json::obj(vec![
            ("jsonrpc", Json::str("2.0")),
            ("id", message.get("id").cloned().unwrap_or(Json::Null)),
            (
                "result",
                Json::obj(vec![
                    ("isIncomplete", Json::Bool(false)),
                    ("items", Json::Array(items)),
                ]),
            ),
        ]);
        DispatchResult {
            outputs: vec![response],
            should_exit: false,
        }
    }

    // ---------------------------------------------------------------------
    // Hover
    // ---------------------------------------------------------------------

    fn handle_hover(&mut self, message: &Json) -> DispatchResult {
        let params = message.get("params").cloned().unwrap_or(Json::Null);
        let uri = params
            .get("textDocument")
            .and_then(|doc| doc.get_str("uri"));
        let position = params.get("position").cloned().unwrap_or(Json::Null);
        let line = position.get_f64("line").unwrap_or(0.0) as usize;
        let character = position.get_f64("character").unwrap_or(0.0) as usize;

        let result = match uri {
            Some(uri) => self
                .documents
                .get(uri)
                .and_then(|source| hover_at(source, uri, line, character)),
            None => None,
        };

        let response = Json::obj(vec![
            ("jsonrpc", Json::str("2.0")),
            ("id", message.get("id").cloned().unwrap_or(Json::Null)),
            ("result", result.unwrap_or(Json::Null)),
        ]);
        DispatchResult {
            outputs: vec![response],
            should_exit: false,
        }
    }

    // ---------------------------------------------------------------------
    // Document symbols
    // ---------------------------------------------------------------------

    fn handle_document_symbol(&mut self, message: &Json) -> DispatchResult {
        let params = message.get("params").cloned().unwrap_or(Json::Null);
        let uri = params
            .get("textDocument")
            .and_then(|doc| doc.get_str("uri"));
        let symbols = uri
            .and_then(|uri| self.documents.get(uri))
            .map(|source| {
                document_symbols(source)
                    .into_iter()
                    .map(|symbol| symbol.to_json())
                    .collect()
            })
            .unwrap_or_default();

        let response = Json::obj(vec![
            ("jsonrpc", Json::str("2.0")),
            ("id", message.get("id").cloned().unwrap_or(Json::Null)),
            ("result", Json::Array(symbols)),
        ]);
        DispatchResult {
            outputs: vec![response],
            should_exit: false,
        }
    }
}

// -------------------------------------------------------------------------
// Diagnostics collection
// -------------------------------------------------------------------------

/// A diagnostic with an LSP 0-based range.
#[derive(Debug)]
struct Diagnostic {
    /// 0-based line of the start of the range.
    start_line: usize,
    /// 0-based character of the start of the range.
    start_character: usize,
    /// 0-based line of the end of the range.
    end_line: usize,
    /// 0-based character of the end of the range.
    end_character: usize,
    message: String,
    severity: u8, // 1 = Error
}

impl Diagnostic {
    fn to_json(&self) -> Json {
        Json::obj(vec![
            (
                "range",
                Json::obj(vec![
                    (
                        "start",
                        Json::obj(vec![
                            ("line", Json::num(self.start_line as f64)),
                            ("character", Json::num(self.start_character as f64)),
                        ]),
                    ),
                    (
                        "end",
                        Json::obj(vec![
                            ("line", Json::num(self.end_line as f64)),
                            ("character", Json::num(self.end_character as f64)),
                        ]),
                    ),
                ]),
            ),
            ("severity", Json::num(self.severity)),
            ("message", Json::str(&self.message)),
        ])
    }
}

fn collect_diagnostics(source: &str) -> Vec<Json> {
    let mut diagnostics = Vec::new();

    // Phase 1: lexer errors carry precise byte spans.
    let (tokens, lexer_errors) = Lexer::lex(source);
    for error in &lexer_errors {
        let (start_line, start_character) = byte_to_position(source, error.span.start);
        let (end_line, end_character) = byte_to_position(source, error.span.end);
        diagnostics.push(
            Diagnostic {
                start_line,
                start_character,
                end_line,
                end_character,
                message: error.to_string(),
                severity: 1,
            }
            .to_json(),
        );
    }

    // Phase 2: parser errors with recovery so several syntax issues surface.
    let mut parser = Parser::new(&tokens);
    let (program, parse_errors) = parser.parse_with_recovery();
    for error in &parse_errors {
        let (start_line, start_character) = byte_to_position(source, error.span.start);
        let (end_line, end_character) = byte_to_position(source, error.span.end);
        diagnostics.push(
            Diagnostic {
                start_line,
                start_character,
                end_line,
                end_character,
                message: error.message.clone(),
                severity: 1,
            }
            .to_json(),
        );
    }

    // Phase 3: semantic type checking.  The checker reports a context symbol
    // rather than a span; locate the declaring statement's span (now retained
    // in the AST) for a precise range, falling back to a text scan.
    if let Err(type_errors) = poly_transpiler::check_program(&program) {
        for error in &type_errors {
            let context = error.context.clone().unwrap_or_default();
            let (start_line, start_character, end_character) = if context.is_empty() {
                (0, 0, 1)
            } else {
                statement_symbol_range(source, &program, &context).unwrap_or_else(|| {
                    let (line, character) =
                        find_symbol_position(source, &context).unwrap_or((0, 0));
                    (line, character, character + 1)
                })
            };
            diagnostics.push(
                Diagnostic {
                    start_line,
                    start_character,
                    end_line: start_line,
                    end_character,
                    message: error.to_string(),
                    severity: 1,
                }
                .to_json(),
            );
        }
    }

    diagnostics
}

/// Convert a byte offset into an LSP 0-based (line, character) position.
///
/// The LSP spec defines `character` in UTF-16 code units, so non-ASCII
/// characters count as one unit for BMP code points and two for astral ones.
fn byte_to_position(source: &str, offset: usize) -> (usize, usize) {
    let mut line = 0usize;
    let mut character = 0usize;
    for (index, character_at) in source.char_indices() {
        if index >= offset {
            break;
        }
        if character_at == '\n' {
            line += 1;
            character = 0;
        } else {
            character += character_at.len_utf16();
        }
    }
    (line, character)
}

/// Find the 0-based line/character of the first occurrence of `symbol`.
fn find_symbol_position(source: &str, symbol: &str) -> Option<(usize, usize)> {
    let symbol_index = source.find(symbol)?;
    Some(byte_to_position(source, symbol_index))
}

/// Locate the declaration range of `symbol` using the statement spans the
/// parser retained, returning (line, start_character, end_character).
///
/// Falls back to `None` so callers can use a text scan instead.
fn statement_symbol_range(
    source: &str,
    program: &poly_parser::Program,
    symbol: &str,
) -> Option<(usize, usize, usize)> {
    use poly_parser::ast::Statement;

    for spanned in &program.statements {
        let name = match &spanned.node {
            Statement::FunctionDeclaration(function) => Some(&function.name),
            Statement::StructDeclaration(decl) => Some(&decl.name),
            Statement::EnumDeclaration(decl) => Some(&decl.name),
            Statement::TraitDeclaration(decl) => Some(&decl.name),
            Statement::VarDeclaration { name, .. }
            | Statement::LetDeclaration { name, .. }
            | Statement::ConstDeclaration { name, .. } => Some(name),
            _ => None,
        };
        if name.map(String::as_str) != Some(symbol) {
            continue;
        }
        let (line, character) = byte_to_position(source, spanned.span.start);
        // The symbol begins at the statement's first token, which for a
        // declaration is the name itself; end the range after the name in
        // UTF-16 units.
        let end_character = character + symbol.encode_utf16().count();
        return Some((line, character, end_character));
    }
    None
}

// -------------------------------------------------------------------------
// Symbols (completion + document symbols + hover)
// -------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Symbol {
    name: String,
    kind: u8, // LSP SymbolKind
    detail: String,
    start_line: usize,
    start_character: usize,
    end_line: usize,
    end_character: usize,
}

impl Symbol {
    fn to_json(&self) -> Json {
        Json::obj(vec![
            ("name", Json::str(&self.name)),
            ("kind", Json::num(self.kind)),
            ("detail", Json::str(&self.detail)),
            (
                "range",
                Json::obj(vec![
                    (
                        "start",
                        Json::obj(vec![
                            ("line", Json::num(self.start_line as f64)),
                            ("character", Json::num(self.start_character as f64)),
                        ]),
                    ),
                    (
                        "end",
                        Json::obj(vec![
                            ("line", Json::num(self.end_line as f64)),
                            ("character", Json::num(self.end_character as f64)),
                        ]),
                    ),
                ]),
            ),
            (
                "selectionRange",
                Json::obj(vec![
                    (
                        "start",
                        Json::obj(vec![
                            ("line", Json::num(self.start_line as f64)),
                            ("character", Json::num(self.start_character as f64)),
                        ]),
                    ),
                    (
                        "end",
                        Json::obj(vec![
                            ("line", Json::num(self.end_line as f64)),
                            ("character", Json::num(self.end_character as f64)),
                        ]),
                    ),
                ]),
            ),
        ])
    }
}

/// Collect top-level declarations by scanning the source text directly.
///
/// The AST does not yet retain spans, so the server locates declarations by
/// scanning lines for the current `fn`/`struct`/`enum`/`trait`/`const`/`type`
/// forms.  This is robust for the common one-declaration-per-line style.
fn document_symbols(source: &str) -> Vec<Symbol> {
    let mut symbols = Vec::new();
    for (index, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        let (kind, name) = if let Some(rest) = trimmed.strip_prefix("fn ") {
            (12u8, rest.split(['(', '<']).next().unwrap_or("").trim())
        } else if let Some(rest) = trimmed.strip_prefix("async fn ") {
            (12u8, rest.split(['(', '<']).next().unwrap_or("").trim())
        } else if let Some(rest) = trimmed.strip_prefix("struct ") {
            (
                23u8,
                rest.split(['(', '<', '{']).next().unwrap_or("").trim(),
            )
        } else if let Some(rest) = trimmed.strip_prefix("enum ") {
            (
                10u8,
                rest.split(['(', '<', '{']).next().unwrap_or("").trim(),
            )
        } else if let Some(rest) = trimmed.strip_prefix("trait ") {
            (
                24u8,
                rest.split(['(', '<', '{']).next().unwrap_or("").trim(),
            )
        } else if let Some(rest) = trimmed.strip_prefix("const ") {
            (13u8, rest.split_whitespace().next().unwrap_or("").trim())
        } else if let Some(rest) = trimmed.strip_prefix("type ") {
            (25u8, rest.split_whitespace().next().unwrap_or("").trim())
        } else if let Some(rest) = trimmed.strip_prefix("impl ") {
            (26u8, rest.trim())
        } else {
            continue;
        };
        if name.is_empty() {
            continue;
        }
        // LSP ranges are UTF-16 code units, not bytes: convert both the
        // column of the declaration and its length so non-ASCII identifiers
        // or trailing comments do not shift the reported range.
        let start = line
            .find(trimmed)
            .map(|byte_index| line[..byte_index].encode_utf16().count())
            .unwrap_or(0);
        symbols.push(Symbol {
            name: name.to_string(),
            kind,
            detail: trimmed.to_string(),
            start_line: index,
            start_character: start,
            end_line: index,
            end_character: start + trimmed.encode_utf16().count(),
        });
    }
    symbols
}

/// Build a hover payload for a position, if the position touches a symbol.
fn hover_at(source: &str, _uri: &str, line: usize, character: usize) -> Option<Json> {
    let line_text = source.lines().nth(line)?;
    let (word, word_start, word_end) = word_at(line_text, character)?;
    let symbol = document_symbols(source)
        .into_iter()
        .find(|symbol| symbol.name == word)
        .or_else(|| {
            // Fall back to matching the word as a keyword.
            COMPLETION_KEYWORDS
                .contains(&word.as_str())
                .then(|| Symbol {
                    name: word.clone(),
                    kind: 14,
                    detail: format!("keyword `{word}`"),
                    start_line: 0,
                    start_character: 0,
                    end_line: 0,
                    end_character: 0,
                })
        })?;
    let markdown = format!("```poly\n{}\n```\n\n**{}**", symbol.detail, symbol.name);
    // `word_at` reports char indexes; the LSP range must use UTF-16 units.
    let start_units = char_index_to_utf16(line_text, word_start);
    let end_units = char_index_to_utf16(line_text, word_end);
    Some(Json::obj(vec![
        (
            "range",
            Json::obj(vec![
                (
                    "start",
                    Json::obj(vec![
                        ("line", Json::num(line as f64)),
                        ("character", Json::num(start_units as f64)),
                    ]),
                ),
                (
                    "end",
                    Json::obj(vec![
                        ("line", Json::num(line as f64)),
                        ("character", Json::num(end_units as f64)),
                    ]),
                ),
            ]),
        ),
        (
            "contents",
            Json::obj(vec![
                ("kind", Json::str("markdown")),
                ("value", Json::str(markdown)),
            ]),
        ),
    ]))
}

/// Convert a `char` index within `line_text` into an LSP UTF-16 position.
fn char_index_to_utf16(line_text: &str, char_index: usize) -> usize {
    line_text
        .chars()
        .take(char_index)
        .map(|ch| ch.len_utf16())
        .sum()
}

/// Convert an LSP UTF-16 `character` into a `char` index within `line_text`.
///
/// A position that falls inside a multi-unit character (e.g. the second half
/// of an astral pair) still resolves to that character.
fn utf16_to_char_index(line_text: &str, character: usize) -> usize {
    let mut units = 0usize;
    for (index, ch) in line_text.chars().enumerate() {
        if units >= character {
            return index;
        }
        units += ch.len_utf16();
        // A multi-unit character (é, astral pairs) can contain the cursor;
        // single-unit characters land exactly on the boundary and must let
        // the next character own that position.
        if units > character {
            return index;
        }
    }
    line_text.chars().count()
}

/// Return the identifier (or keyword) containing `character` in `line_text`,
/// with its character range.
fn word_at(line_text: &str, character: usize) -> Option<(String, usize, usize)> {
    let characters: Vec<char> = line_text.chars().collect();
    if characters.is_empty() {
        return None;
    }
    let char_index = utf16_to_char_index(line_text, character);
    let character = char_index.min(characters.len().saturating_sub(1));
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    if !is_word(characters[character]) {
        return None;
    }
    let mut start = character;
    while start > 0 && is_word(characters[start - 1]) {
        start -= 1;
    }
    let mut end = character;
    while end + 1 < characters.len() && is_word(characters[end + 1]) {
        end += 1;
    }
    Some((characters[start..=end].iter().collect(), start, end + 1))
}

// -------------------------------------------------------------------------
// JSON-RPC plumbing
// -------------------------------------------------------------------------

/// Build a response to a request carrying `id`, with the given `result`.
fn json_response(request: &Json, result: Json) -> Json {
    Json::obj(vec![
        ("jsonrpc", Json::str("2.0")),
        ("id", request.get("id").cloned().unwrap_or(Json::Null)),
        ("result", result),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn initialize_message() -> Json {
        Json::obj(vec![
            ("jsonrpc", Json::str("2.0")),
            ("id", Json::num(1)),
            ("method", Json::str("initialize")),
            ("params", Json::empty_object()),
        ])
    }

    fn did_open_message(uri: &str, text: &str) -> Json {
        Json::obj(vec![
            ("jsonrpc", Json::str("2.0")),
            ("method", Json::str("textDocument/didOpen")),
            (
                "params",
                Json::obj(vec![(
                    "textDocument",
                    Json::obj(vec![("uri", Json::str(uri)), ("text", Json::str(text))]),
                )]),
            ),
        ])
    }

    #[test]
    fn initialize_responds_with_capabilities() {
        let mut server = Server::new();
        let result = server.dispatch(&initialize_message());
        assert!(!result.should_exit);
        let response = &result.outputs[0];
        assert_eq!(response.get_str("jsonrpc"), Some("2.0"));
        assert_eq!(response.get_f64("id"), Some(1.0));
        let capabilities = response.get("result").and_then(|r| r.get("capabilities"));
        assert!(capabilities.is_some());
    }

    #[test]
    fn diagnostics_are_published_on_open() {
        let mut server = Server::new();
        server.dispatch(&initialize_message());
        let result = server.dispatch(&did_open_message(
            "file:///test.poly",
            "var x i32 := 42\nvar y := @bad\n",
        ));
        assert_eq!(result.outputs.len(), 1);
        let notification = &result.outputs[0];
        assert_eq!(
            notification.get_str("method"),
            Some("textDocument/publishDiagnostics")
        );
        let diagnostics = notification
            .get("params")
            .and_then(|params| params.get("diagnostics"))
            .and_then(|items| match items {
                Json::Array(items) => Some(items),
                _ => None,
            });
        assert!(!diagnostics.unwrap().is_empty());
    }

    #[test]
    fn valid_document_publishes_no_diagnostics() {
        let mut server = Server::new();
        server.dispatch(&initialize_message());
        let result = server.dispatch(&did_open_message(
            "file:///test.poly",
            "var x i32 := 42\nput x\n",
        ));
        let diagnostics = result.outputs[0]
            .get("params")
            .and_then(|params| params.get("diagnostics"))
            .and_then(|items| match items {
                Json::Array(items) => Some(items),
                _ => None,
            });
        assert!(diagnostics.unwrap().is_empty());
    }

    #[test]
    fn completion_lists_keywords_and_symbols() {
        let mut server = Server::new();
        server.dispatch(&initialize_message());
        server.dispatch(&did_open_message(
            "file:///test.poly",
            "fn helper()\n    return 1\nend fn\nvar h := hel\n",
        ));
        let request = Json::obj(vec![
            ("jsonrpc", Json::str("2.0")),
            ("id", Json::num(2)),
            ("method", Json::str("textDocument/completion")),
            (
                "params",
                Json::obj(vec![
                    (
                        "textDocument",
                        Json::obj(vec![("uri", Json::str("file:///test.poly"))]),
                    ),
                    (
                        "position",
                        Json::obj(vec![
                            ("line", Json::num(3.0)),
                            ("character", Json::num(10.0)),
                        ]),
                    ),
                ]),
            ),
        ]);
        let result = server.dispatch(&request);
        let items = result.outputs[0]
            .get("result")
            .and_then(|r| r.get("items"))
            .and_then(|items| match items {
                Json::Array(items) => Some(items),
                _ => None,
            })
            .unwrap();
        assert!(items.len() >= COMPLETION_KEYWORDS.len());
        assert!(items
            .iter()
            .any(|item| item.get_str("label") == Some("helper")));
    }

    #[test]
    fn hover_reports_function_detail() {
        let mut server = Server::new();
        server.dispatch(&initialize_message());
        server.dispatch(&did_open_message(
            "file:///test.poly",
            "fn greet(name: ustring): ustring\n    return name\nend fn\n",
        ));
        let request = Json::obj(vec![
            ("jsonrpc", Json::str("2.0")),
            ("id", Json::num(3)),
            ("method", Json::str("textDocument/hover")),
            (
                "params",
                Json::obj(vec![
                    (
                        "textDocument",
                        Json::obj(vec![("uri", Json::str("file:///test.poly"))]),
                    ),
                    (
                        "position",
                        Json::obj(vec![
                            ("line", Json::num(0.0)),
                            ("character", Json::num(4.0)),
                        ]),
                    ),
                ]),
            ),
        ]);
        let result = server.dispatch(&request);
        let contents = result.outputs[0]
            .get("result")
            .and_then(|r| r.get("contents"))
            .and_then(|c| c.get_str("value"));
        assert!(contents.unwrap().contains("fn greet"));
    }

    #[test]
    fn document_symbols_find_declarations() {
        let mut server = Server::new();
        server.dispatch(&initialize_message());
        server.dispatch(&did_open_message(
            "file:///test.poly",
            "struct Point\n    var x: f32\nend struct\n\nfn main()\n    put 1\nend fn\n",
        ));
        let request = Json::obj(vec![
            ("jsonrpc", Json::str("2.0")),
            ("id", Json::num(4)),
            ("method", Json::str("textDocument/documentSymbol")),
            (
                "params",
                Json::obj(vec![(
                    "textDocument",
                    Json::obj(vec![("uri", Json::str("file:///test.poly"))]),
                )]),
            ),
        ]);
        let result = server.dispatch(&request);
        let symbols = result.outputs[0]
            .get("result")
            .and_then(|r| match r {
                Json::Array(items) => Some(items),
                _ => None,
            })
            .unwrap();
        assert_eq!(symbols.len(), 2);
        assert_eq!(symbols[0].get_str("name"), Some("Point"));
        assert_eq!(symbols[1].get_str("name"), Some("main"));
    }

    #[test]
    fn shutdown_then_exit_terminates() {
        let mut server = Server::new();
        server.dispatch(&initialize_message());
        let shutdown = Json::obj(vec![
            ("jsonrpc", Json::str("2.0")),
            ("id", Json::num(5)),
            ("method", Json::str("shutdown")),
            ("params", Json::empty_object()),
        ]);
        let result = server.dispatch(&shutdown);
        assert!(!result.should_exit);
        let exit = Json::obj(vec![
            ("jsonrpc", Json::str("2.0")),
            ("method", Json::str("exit")),
            ("params", Json::empty_object()),
        ]);
        assert!(server.dispatch(&exit).should_exit);
    }

    #[test]
    fn word_at_finds_identifier_bounds() {
        assert_eq!(
            word_at("fn helper()", 4),
            Some(("helper".to_string(), 3, 9))
        );
        assert_eq!(word_at("var x := 42", 1), Some(("var".to_string(), 0, 3)));
        assert_eq!(word_at("var x := 42", 4), Some(("x".to_string(), 4, 5)));
        assert_eq!(word_at("put 42", 0), Some(("put".to_string(), 0, 3)));
        assert_eq!(word_at("put 42", 3), None);
        assert_eq!(word_at("  put 42", 2), Some(("put".to_string(), 2, 5)));
    }

    #[test]
    fn document_symbols_report_utf16_ranges() {
        // é is one UTF-16 unit but two bytes; a byte-based end column would
        // overshoot by one for every such character in the declaration.
        let symbols = document_symbols("struct café\n");
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].start_character, 0);
        assert_eq!(
            symbols[0].end_character,
            "struct café".encode_utf16().count()
        );
    }

    #[test]
    fn word_at_counts_utf16_units_not_chars() {
        // "caf😀 helper" — 😀 (U+1F600) is one char but two UTF-16 units, so
        // the cursor for the `h` of `helper` sits at character 6, not 5.
        assert_eq!(
            word_at("caf😀 helper", 6),
            Some(("helper".to_string(), 5, 11))
        );
        // An astral char (😀 = 2 UTF-16 units) shifts positions the same way.
        assert_eq!(word_at("x😀helper", 4), Some(("helper".to_string(), 2, 8)));
        // A character index that is not a UTF-16 boundary still lands on the
        // correct word (cursor inside the second unit of an astral letter).
        assert_eq!(word_at("a𐐀bc xyz", 2), Some(("a𐐀bc".to_string(), 0, 4)));
    }

    #[test]
    fn json_rpc_response_round_trips() {
        let message = Json::obj(vec![
            ("jsonrpc", Json::str("2.0")),
            ("id", Json::num(7)),
            ("method", Json::str("shutdown")),
            ("params", Json::empty_object()),
        ]);
        let response = json_response(&message, Json::Null);
        let text = response.serialize();
        let parsed = crate::json::parse(&text).unwrap();
        assert_eq!(parsed.get_f64("id"), Some(7.0));
        assert_eq!(parsed.get("result"), Some(&Json::Null));
    }
}
