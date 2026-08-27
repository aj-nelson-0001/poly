//! Interactive REPL for the Poly compiler.
//!
//! The REPL keeps a *session*: every accepted statement is appended to the
//! accumulated program, and the whole session is re-transpiled on each
//! submission.  Variables therefore persist across lines (`var x := 5`
//! followed by `put x` works), and type errors are reported against the full
//! session rather than a single isolated line.
//!
//! On Linux and macOS terminals the REPL runs a small raw-mode line editor
//! with Tab completion and arrow-key history navigation.  Elsewhere (pipes,
//! Windows, non-TTY input) it falls back to plain line reading so scripting
//! and integration tests keep working.

use std::io::{self, IsTerminal, Read, Write};

use poly_lexer::Lexer;
use poly_parser::Parser;
use poly_transpiler::Transpiler;

const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const RED: &str = "\x1b[31m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const BLUE: &str = "\x1b[34m";
const CYAN: &str = "\x1b[36m";
const MAGENTA: &str = "\x1b[35m";

/// Poly keywords for tab completion.
const POLY_KEYWORDS: &[&str] = &[
    "var", "let", "const", "fn", "return", "if", "else", "end", "while", "for", "in", "loop",
    "struct", "enum", "match", "trait", "impl", "async", "await", "unsafe", "pub", "module", "use",
    "type", "as", "try", "spawn", "move", "break", "continue", // Types
    "i8", "i16", "i32", "i64", "i128", "u8", "u16", "u32", "u64", "u128", "f32", "f64", "bool",
    "char", "string", "usize", "isize", "byte", "bytes", // Builtins
    "put", "get", "error", "warn", "info",
];

/// The accumulated REPL session: every accepted statement, in order.
struct Session {
    /// Source lines of the accepted program (top-level statements).
    program_lines: Vec<String>,
    /// The transpiled Rust from the last accepted submission.
    last_rust: String,
}

impl Session {
    fn new() -> Self {
        Self {
            program_lines: Vec::new(),
            last_rust: String::new(),
        }
    }

    fn source(&self) -> String {
        self.program_lines.join("\n")
    }
}

/// Run the interactive REPL.
pub fn run() {
    let mut session = Session::new();
    let history_path = dirs_and_history_path();
    let history = load_history(&history_path);

    println!(
        "{}{}Poly Language REPL{} v{}",
        BOLD,
        CYAN,
        RESET,
        env!("CARGO_PKG_VERSION")
    );
    println!(
        "{}Type Poly code and press Enter. Variables persist across lines.{}\n{}Commands: :help, :tokens, :ast, :vars, :history, :reset, :quit{}",
        DIM, RESET, DIM, RESET
    );
    println!();

    let interactive = io::stdin().is_terminal();

    // The raw-mode editor is only attempted on Unix terminals; every other
    // environment uses plain line reading.
    let mut raw_editor = RawEditor::new(interactive);

    let mut buffer = String::new();
    let mut line_number = 0usize;

    loop {
        // Show prompt
        if buffer.is_empty() {
            print!("{}poly{}>{} ", GREEN, BOLD, RESET);
        } else {
            print!("  {}...{} ", YELLOW, RESET);
        }
        io::stdout().flush().unwrap();

        let line = if let Some(editor) = raw_editor.as_mut() {
            let (_line, action) = editor.read_line_with_completion(&session);
            match action {
                EditorAction::Quit => {
                    save_history(&history_path, &history);
                    println!("{}Goodbye!{}", GREEN, RESET);
                    return;
                }
                EditorAction::Submit(line) => line,
                EditorAction::Clear => {
                    buffer.clear();
                    line_number = 0;
                    println!("{}Buffer cleared.{}", GREEN, RESET);
                    continue;
                }
            }
        } else {
            let mut input = String::new();
            match io::stdin().read_line(&mut input) {
                Ok(0) => break, // EOF
                Ok(_) => input,
                Err(e) => {
                    eprintln!("{}Error reading input: {}{}", RED, e, RESET);
                    break;
                }
            }
        };

        let line = line.trim_end_matches(['\n', '\r']).to_string();
        // A trailing backslash continues the statement on the next line. The
        // marker itself is stripped so the lexer never sees the `\`.
        let continues = line.trim_end().ends_with('\\');
        let trimmed = line.trim().trim_end_matches('\\').trim_end().to_string();
        line_number += 1;

        // Handle REPL commands.
        if let Some(action) =
            handle_command(&trimmed, &mut buffer, &mut session, &history_path, &history)
        {
            match action {
                CommandAction::Quit => {
                    save_history(&history_path, &history);
                    println!("{}Goodbye!{}", GREEN, RESET);
                    return;
                }
                CommandAction::Continue => continue,
            }
        }

        // Add to history (plain mode already echoes; raw mode tracked it).
        if !trimmed.is_empty() && history.last().map(String::as_str) != Some(trimmed.as_str()) {
            save_history(&history_path, &append_history_line(&history, &trimmed));
        }

        // Accumulate multi-line input.
        if !buffer.is_empty() {
            buffer.push('\n');
        }
        buffer.push_str(&trimmed);

        if !continues && input_is_complete(&buffer, line_number) {
            submit_statement(&mut session, &buffer);
            buffer.clear();
            line_number = 0;
            println!();
        }
    }

    save_history(&history_path, &history);
    println!("{}Goodbye!{}", GREEN, RESET);
}

/// Build a history list with `line` appended (deduplicated against the tail).
fn append_history_line(history: &[String], line: &str) -> Vec<String> {
    let mut updated = history.to_vec();
    if updated.last().map(String::as_str) != Some(line) {
        updated.push(line.to_string());
        if updated.len() > 1000 {
            updated.drain(0..500);
        }
    }
    updated
}

/// Handle a `:command` line; returns `None` if the line is a statement.
fn handle_command(
    line: &str,
    buffer: &mut String,
    session: &mut Session,
    history_path: &std::path::Path,
    history: &[String],
) -> Option<CommandAction> {
    match line {
        ":quit" | ":q" | ":exit" => Some(CommandAction::Quit),
        ":help" => {
            print_help();
            Some(CommandAction::Continue)
        }
        ":clear" => {
            buffer.clear();
            println!("{}Buffer cleared.{}", GREEN, RESET);
            Some(CommandAction::Continue)
        }
        ":reset" => {
            session.program_lines.clear();
            session.last_rust.clear();
            buffer.clear();
            println!(
                "{}Session reset. Variables no longer persist.{}\n",
                YELLOW, RESET
            );
            Some(CommandAction::Continue)
        }
        ":history" => {
            println!("{}Command History:{}", BOLD, RESET);
            for (index, command) in history.iter().enumerate() {
                println!("  {}{}: {}{}", DIM, index + 1, command, RESET);
            }
            if history.is_empty() {
                println!("  {}(empty){}", DIM, RESET);
            }
            Some(CommandAction::Continue)
        }
        ":clear-history" => {
            let _ = std::fs::write(history_path, "");
            println!("{}History cleared.{}", GREEN, RESET);
            Some(CommandAction::Continue)
        }
        ":vars" => {
            print_session_vars(session);
            Some(CommandAction::Continue)
        }
        ":tokens" => {
            let source = if buffer.is_empty() {
                session.source()
            } else {
                buffer.clone()
            };
            if source.trim().is_empty() {
                println!("{}No input to tokenize.{}", YELLOW, RESET);
            } else {
                let (tokens, errors) = Lexer::lex(&source);
                if !errors.is_empty() {
                    println!("{}Lexer errors:{}", RED, RESET);
                    for error in &errors {
                        let rendered = poly_lexer::diagnostics::render_span_error(
                            &source,
                            error.span,
                            "<repl>",
                            &error.message,
                        );
                        println!("{}{}{}", RED, rendered, RESET);
                    }
                } else {
                    println!("{}Tokens:{}", BOLD, RESET);
                    for token in &tokens {
                        println!("  {}", highlight_token(&format!("{token:?}")));
                    }
                }
            }
            Some(CommandAction::Continue)
        }
        ":ast" => {
            let source = if buffer.is_empty() {
                session.source()
            } else {
                buffer.clone()
            };
            if source.trim().is_empty() {
                println!("{}No input to parse.{}", YELLOW, RESET);
            } else {
                let (tokens, errors) = Lexer::lex(&source);
                if !errors.is_empty() {
                    println!("{}Lexer errors:{}", RED, RESET);
                    for error in &errors {
                        println!("{}{}{}", RED, error, RESET);
                    }
                } else {
                    let mut parser = Parser::new(&tokens);
                    match parser.parse() {
                        Ok(program) => {
                            println!("{:#?}", program);
                        }
                        Err(error) => {
                            let rendered = poly_lexer::diagnostics::render_span_error(
                                &source,
                                error.span,
                                "<repl>",
                                &error.message,
                            );
                            println!("{}{}{}", RED, rendered, RESET);
                        }
                    }
                }
            }
            Some(CommandAction::Continue)
        }
        _ => None,
    }
}

enum CommandAction {
    Quit,
    Continue,
}

/// Print the current session statements and inferred variable names.
fn print_session_vars(session: &Session) {
    println!("{}Session:{}", BOLD, RESET);
    if session.program_lines.is_empty() {
        println!("  {}(empty — no statements accepted yet){}", DIM, RESET);
        return;
    }
    for (index, line) in session.program_lines.iter().enumerate() {
        println!("  {}{:>3}{}  {}", DIM, index + 1, RESET, line);
    }
    // Best-effort variable listing from the parsed program.
    let (tokens, _) = Lexer::lex(&session.source());
    let mut parser = Parser::new(&tokens);
    if let Ok(program) = parser.parse() {
        let mut names: Vec<String> = Vec::new();
        for spanned in &program.statements {
            match &spanned.node {
                poly_parser::ast::Statement::VarDeclaration { name, .. }
                | poly_parser::ast::Statement::LetDeclaration { name, .. } => {
                    if !names.iter().any(|n| n == name) {
                        names.push(name.clone());
                    }
                }
                poly_parser::ast::Statement::FunctionDeclaration(function) => {
                    names.push(function.name.clone());
                }
                _ => {}
            }
        }
        if !names.is_empty() {
            println!("{}Bindings:{} {}", GREEN, RESET, names.join(", "));
        }
    }
}

/// Submit a completed statement to the session and show the new Rust.
fn submit_statement(session: &mut Session, source: &str) {
    session.program_lines.push(source.trim().to_string());
    let full_source = session.source();

    let transpiler = Transpiler::new();
    match transpiler.transpile_checked(&full_source) {
        Ok(rust_code) => {
            println!("{}// Generated Rust:{}", DIM, RESET);
            let added = added_lines(&session.last_rust, &rust_code);
            if added.is_empty() {
                println!("  {}<no new output>{}", DIM, RESET);
            } else {
                for line in added {
                    println!("  {}", highlight_rust(&line));
                }
            }
            session.last_rust = rust_code;
        }
        Err(error) => {
            // Roll back the rejected statement so the session stays valid.
            session.program_lines.pop();
            println!("{}Error: {}{}", RED, error, RESET);
            show_error_suggestions(&error);
        }
    }
}

/// Return the lines in `new` that are not continuations of `old`.
///
/// Uses the longest common prefix and suffix so appending statements (the
/// common REPL case) shows exactly the newly generated lines, while insertions
/// (e.g. a new function before `main`) still surface their new lines.
fn added_lines(old: &str, new: &str) -> Vec<String> {
    let old_lines: Vec<&str> = old.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();

    let mut prefix = 0;
    while prefix < old_lines.len()
        && prefix < new_lines.len()
        && old_lines[prefix] == new_lines[prefix]
    {
        prefix += 1;
    }

    let mut suffix = 0;
    while suffix < old_lines.len().saturating_sub(prefix)
        && suffix < new_lines.len().saturating_sub(prefix)
        && old_lines[old_lines.len() - 1 - suffix] == new_lines[new_lines.len() - 1 - suffix]
    {
        suffix += 1;
    }

    let middle_end = new_lines.len().saturating_sub(suffix);
    new_lines[prefix..middle_end]
        .iter()
        .map(|s| s.to_string())
        .collect()
}

/// Whether the buffered input forms a complete statement.
///
/// Single-line statements are complete immediately. Statements that open a
/// block (`fn`, `if`, `while`, `for`, `loop`, `match`, `struct`, `enum`,
/// `trait`, `impl`, `unsafe`, `module`) stay buffered until the matching
/// `end <keyword>` arrives, so multi-line definitions work naturally.
fn input_is_complete(buffer: &str, _line_number: usize) -> bool {
    const BLOCK_KEYWORDS: &[&str] = &[
        "fn", "struct", "enum", "trait", "impl", "module", "if", "while", "for", "loop", "match",
        "unsafe",
    ];
    let trimmed = buffer.trim();
    if trimmed.is_empty() {
        return true;
    }
    let starts_block = BLOCK_KEYWORDS.iter().any(|keyword| {
        trimmed.starts_with(keyword)
            && trimmed[keyword.len()..]
                .chars()
                .next()
                .map(|character| !character.is_alphanumeric() && character != '_')
                .unwrap_or(true)
    });
    if !starts_block {
        return true;
    }
    BLOCK_KEYWORDS
        .iter()
        .any(|keyword| trimmed.ends_with(&format!("end {keyword}")))
}

/// Complete a partial input with keyword suggestions.
pub(crate) fn complete_input(partial: &str) -> Vec<String> {
    let partial_lower = partial.to_lowercase();
    POLY_KEYWORDS
        .iter()
        .filter(|keyword| keyword.starts_with(&partial_lower) && *keyword != &partial_lower)
        .map(|s| s.to_string())
        .collect()
}

/// Print the REPL help text.
fn print_help() {
    println!();
    println!("{}REPL Commands:{}", BOLD, RESET);
    println!("  {}:{}       Show this help message", CYAN, RESET);
    println!("  {}:{}     Tokenize the session or buffer", CYAN, RESET);
    println!("  {}:{}      Parse the session or buffer", CYAN, RESET);
    println!(
        "  {}:{}      List session statements and bindings",
        CYAN, RESET
    );
    println!("  {}:{}     Show command history", CYAN, RESET);
    println!("  {}:{}    Clear the pending buffer", CYAN, RESET);
    println!(
        "  {}:{}    Reset the session (drop all variables)",
        CYAN, RESET
    );
    println!("  {}:{}      Clear history", CYAN, RESET);
    println!("  {}:{}    Exit the REPL", CYAN, RESET);
    println!();
    println!("{}Features:{}", BOLD, RESET);
    println!(
        "{}  - Variables persist across lines: enter `var x := 5` then `put x`{}",
        DIM, RESET
    );
    println!(
        "{}  - Tab completes keywords and symbols (on Linux/macOS terminals){}",
        DIM, RESET
    );
    println!(
        "{}  - ↑/↓ navigate history, ←/→ move the cursor{}",
        DIM, RESET
    );
    println!(
        "{}  - Multi-line input: continue on the next line for blocks{}",
        DIM, RESET
    );
    println!(
        "{}  - :tokens / :ast / :vars inspect the current session{}",
        DIM, RESET
    );
    println!();
}

/// Show helpful suggestions based on error message.
fn show_error_suggestions(error: &str) {
    if error.contains("Expected") && error.contains("end") {
        println!(
            "{}  💡 Tip: Blocks must end with 'end <keyword>' (e.g., end fn, end if){}",
            YELLOW, RESET
        );
    } else if error.contains("Unexpected token") {
        println!(
            "{}  💡 Tip: Check for missing semicolons or keywords{}\n{}     Poly uses 'put' for output and 'get' for input{}",
            YELLOW, RESET, DIM, RESET
        );
    } else if error.contains("type") {
        println!(
            "{}  💡 Tip: Valid types: i32, f64, string, bool, char, u8, etc.{}",
            YELLOW, RESET
        );
    } else if error.contains("not found") || error.contains("unknown") {
        println!(
            "{}  💡 Tip: Use `:vars` to see bindings available in the session{}",
            YELLOW, RESET
        );
    }
}

/// Highlight a token for REPL output.
fn highlight_token(token_str: &str) -> String {
    if token_str.contains("StringLiteral") || token_str.contains("UnicodeStringLiteral") {
        format!("{YELLOW}{token_str}{RESET}")
    } else if token_str.contains("IntLiteral") || token_str.contains("FloatLiteral") {
        format!("{BLUE}{token_str}{RESET}")
    } else if token_str.contains("Fn")
        || token_str.contains("Let")
        || token_str.contains("Var")
        || token_str.contains("Return")
        || token_str.contains("If")
        || token_str.contains("Else")
        || token_str.contains("While")
        || token_str.contains("For")
        || token_str.contains("Match")
        || token_str.contains("End")
        || token_str.contains("Struct")
        || token_str.contains("Enum")
        || token_str.contains("Break")
        || token_str.contains("Continue")
    {
        format!("{MAGENTA}{token_str}{RESET}")
    } else if token_str.contains("Identifier") {
        format!("{CYAN}{token_str}{RESET}")
    } else {
        format!("{GREEN}{token_str}{RESET}")
    }
}

/// Highlight Rust code for REPL output.
fn highlight_rust(line: &str) -> String {
    let trimmed = line.trim();
    if trimmed.starts_with("//") {
        return format!("{DIM}{line}{RESET}");
    }
    let mut result = line.to_string();
    let keywords = [
        "fn", "let", "mut", "return", "if", "else", "while", "for", "loop", "struct", "enum",
        "impl", "use", "pub", "const", "break", "continue", "match", "self", "true", "false",
    ];
    for keyword in keywords {
        result = result.replace(keyword, &format!("{MAGENTA}{keyword}{RESET}"));
    }
    if result.contains('"') {
        let parts: Vec<&str> = result.split('"').collect();
        if parts.len() >= 3 {
            let mut highlighted = String::new();
            for (index, part) in parts.iter().enumerate() {
                if index % 2 == 0 {
                    highlighted.push_str(part);
                } else {
                    highlighted.push_str(&format!("{YELLOW}\"{part}\"{RESET}"));
                }
            }
            result = highlighted;
        }
    }
    result
}

// ---------------------------------------------------------------------------
// History persistence
// ---------------------------------------------------------------------------

fn dirs_and_history_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    std::path::PathBuf::from(home).join(".poly_repl_history")
}

fn load_history(path: &std::path::Path) -> Vec<String> {
    std::fs::read_to_string(path)
        .map(|content| content.lines().map(String::from).collect())
        .unwrap_or_default()
}

fn save_history(path: &std::path::Path, history: &[String]) {
    let _ = std::fs::write(path, history.join("\n"));
}

// ---------------------------------------------------------------------------
// Raw-mode line editor (Unix terminals)
// ---------------------------------------------------------------------------

enum EditorAction {
    Submit(String),
    Quit,
    Clear,
}

/// A minimal raw-mode line editor with history and Tab completion.
///
/// Implemented with `termios` FFI directly (no external dependencies).  Falls
/// back to plain line reading when raw mode cannot be enabled.
struct RawEditor {
    line: Vec<char>,
    cursor: usize,
    history: Vec<String>,
    history_index: Option<usize>,
    _guard: Option<TermiosGuard>,
}

enum TermiosGuard {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    Restore { fd: i32, original: RawTermios },
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    Unsupported,
}

impl RawEditor {
    fn new(interactive: bool) -> Option<Self> {
        if !interactive {
            return None;
        }
        let guard = enable_raw_mode()?;
        Some(Self {
            line: Vec::new(),
            cursor: 0,
            history: Vec::new(),
            history_index: None,
            _guard: Some(guard),
        })
    }

    /// Read one line, handling editing keys.  Returns the submitted line and
    /// whether it should replace history handling.
    fn read_line_with_completion(&mut self, session: &Session) -> (String, EditorAction) {
        let stdin = io::stdin();
        let mut reader = stdin.lock();
        let mut pending: Vec<u8> = Vec::new();

        loop {
            if pending.is_empty() {
                let mut byte = [0u8; 1];
                if reader.read(&mut byte).unwrap_or(0) == 0 {
                    return (String::new(), EditorAction::Quit);
                }
                pending.push(byte[0]);
            }

            let byte = pending.remove(0);
            match byte {
                b'\r' | b'\n' => {
                    println!();
                    let submitted: String = self.line.iter().collect();
                    let trimmed = submitted.trim().to_string();
                    if !trimmed.is_empty() {
                        self.history.push(trimmed.clone());
                    }
                    self.history_index = None;
                    self.line.clear();
                    self.cursor = 0;
                    return (submitted.clone(), EditorAction::Submit(submitted));
                }
                0x03 => {
                    // Ctrl+C: clear the current line.
                    self.line.clear();
                    self.cursor = 0;
                    println!("^C");
                    return (String::new(), EditorAction::Clear);
                }
                0x04 => {
                    // Ctrl+D: quit if the line is empty.
                    if self.line.is_empty() {
                        return (String::new(), EditorAction::Quit);
                    }
                }
                0x7F | 0x08 => self.backspace(),
                b'\t' => self.complete(session),
                0x1B => {
                    // Escape sequence: arrows, home, end, delete.
                    if let Some(sequence) = self.read_escape_sequence(&mut reader) {
                        self.handle_escape(&sequence);
                    }
                }
                byte if byte < 0x20 => { /* ignore other control bytes */ }
                byte => {
                    // Assemble a full UTF-8 character.  Each continuation byte
                    // is read exactly once (reading in the loop condition would
                    // consume and discard it).
                    let mut bytes = vec![byte];
                    let expected = utf8_sequence_length(byte);
                    while bytes.len() < expected {
                        let mut next = [0u8; 1];
                        if reader.read(&mut next).unwrap_or(0) == 0 {
                            break;
                        }
                        bytes.push(next[0]);
                    }
                    if let Ok(text) = std::str::from_utf8(&bytes) {
                        for character in text.chars() {
                            self.line.insert(self.cursor, character);
                            self.cursor += 1;
                        }
                        self.redraw();
                    }
                }
            }
        }
    }

    fn redraw(&self) {
        let text: String = self.line.iter().collect();
        print!("\r{}{}poly>{} {}", GREEN, BOLD, RESET, text);
        // Move cursor back to the edit position.
        let trailing = text.chars().count().saturating_sub(self.cursor);
        if trailing > 0 {
            print!("\x1b[{}D", trailing);
        }
        io::stdout().flush().unwrap();
    }

    fn backspace(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.line.remove(self.cursor);
            self.redraw();
        }
    }

    /// Read the rest of an escape sequence starting with ESC.
    fn read_escape_sequence<R: Read>(&self, reader: &mut R) -> Option<Vec<u8>> {
        let mut sequence = vec![0x1B];
        // CSI sequences start with '['.
        let mut first = [0u8; 1];
        reader.read_exact(&mut first).ok()?;
        sequence.push(first[0]);
        if first[0] == b'[' {
            loop {
                let mut byte = [0u8; 1];
                if reader.read(&mut byte).ok()? == 0 {
                    break;
                }
                sequence.push(byte[0]);
                if byte[0].is_ascii_alphabetic() || byte[0] == b'~' {
                    break;
                }
            }
        }
        Some(sequence)
    }

    fn handle_escape(&mut self, sequence: &[u8]) {
        match sequence {
            [0x1B, b'[', b'A'] => self.history_up(),
            [0x1B, b'[', b'B'] => self.history_down(),
            [0x1B, b'[', b'C'] => {
                if self.cursor < self.line.len() {
                    self.cursor += 1;
                    print!("\x1b[1C");
                    io::stdout().flush().unwrap();
                }
            }
            [0x1B, b'[', b'D'] => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                    print!("\x1b[1D");
                    io::stdout().flush().unwrap();
                }
            }
            [0x1B, b'[', b'H'] | [0x1B, b'[', b'1', b'~'] => {
                self.cursor = 0;
                self.redraw();
            }
            [0x1B, b'[', b'F'] | [0x1B, b'[', b'4', b'~'] => {
                self.cursor = self.line.len();
                self.redraw();
            }
            [0x1B, b'[', b'3', b'~'] if self.cursor < self.line.len() => {
                self.line.remove(self.cursor);
                self.redraw();
            }
            _ => {}
        }
    }

    fn history_up(&mut self) {
        if self.history.is_empty() {
            return;
        }
        let current = self.history_index.unwrap_or(self.history.len());
        if current > 0 {
            self.history_index = Some(current - 1);
            self.line = self.history[current - 1].chars().collect();
            self.cursor = self.line.len();
            self.redraw();
        }
    }

    fn history_down(&mut self) {
        if let Some(index) = self.history_index {
            let next = index + 1;
            if next < self.history.len() {
                self.history_index = Some(next);
                self.line = self.history[next].chars().collect();
            } else {
                self.history_index = None;
                self.line.clear();
            }
            self.cursor = self.line.len();
            self.redraw();
        }
    }

    /// Tab completion: replace the current word with its unique completion,
    /// or cycle through candidates when several match.
    fn complete(&mut self, session: &Session) {
        let text: String = self.line.iter().collect();
        let chars: Vec<char> = text.chars().take(self.cursor).collect();
        let word_start = chars
            .iter()
            .rposition(|c| !c.is_alphanumeric() && *c != '_')
            .map(|index| index + 1)
            .unwrap_or(0);
        let word: String = text
            .chars()
            .skip(word_start)
            .take(self.cursor - word_start)
            .collect();

        let mut candidates = complete_input(&word);
        for symbol in session_symbols(session) {
            if symbol.starts_with(&word) && symbol != word {
                candidates.push(symbol);
            }
        }
        candidates.sort();
        candidates.dedup();

        if candidates.len() == 1 {
            let replacement: String = text
                .chars()
                .take(word_start)
                .chain(candidates[0].chars())
                .collect();
            self.line = replacement.chars().collect();
            self.cursor = self.line.len();
            self.redraw();
        } else if candidates.len() > 1 {
            // Show the options on a fresh line.
            println!();
            println!("{}  {}{}", DIM, candidates.join("  "), RESET);
            self.redraw();
        }
    }
}

/// Names of top-level bindings and functions in the session.
fn session_symbols(session: &Session) -> Vec<String> {
    let mut symbols = Vec::new();
    let (tokens, _) = Lexer::lex(&session.source());
    let mut parser = Parser::new(&tokens);
    if let Ok(program) = parser.parse() {
        for spanned in &program.statements {
            match &spanned.node {
                poly_parser::ast::Statement::VarDeclaration { name, .. }
                | poly_parser::ast::Statement::LetDeclaration { name, .. } => {
                    symbols.push(name.clone());
                }
                poly_parser::ast::Statement::FunctionDeclaration(function) => {
                    symbols.push(function.name.clone());
                }
                _ => {}
            }
        }
    }
    symbols
}

/// Number of UTF-8 bytes that make up a character starting with `first`.
fn utf8_sequence_length(first: u8) -> usize {
    if first < 0x80 {
        1
    } else if first >= 0xF0 {
        4
    } else if first >= 0xE0 {
        3
    } else if first >= 0xC0 {
        2
    } else {
        1
    }
}

/// Enable raw mode on the terminal, returning a guard that restores it.
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn enable_raw_mode() -> Option<TermiosGuard> {
    use std::os::fd::AsRawFd;

    let fd = io::stdin().as_raw_fd();
    let mut termios = RawTermios::default();
    if unsafe { raw_tcgetattr(fd, &mut termios) } != 0 {
        return None;
    }
    let original = termios;
    termios.c_lflag &= !(ICANON | ECHO | ISIG);
    termios.c_iflag &= !(ICRNL | IXON);
    if unsafe { raw_tcsetattr(fd, TCSANOW, &termios) } != 0 {
        return None;
    }
    Some(TermiosGuard::Restore { fd, original })
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn enable_raw_mode() -> Option<TermiosGuard> {
    None
}

impl Drop for TermiosGuard {
    fn drop(&mut self) {
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        match self {
            TermiosGuard::Restore { fd, original } => {
                let _ = unsafe { raw_tcsetattr(*fd, TCSANOW, original) };
            }
            #[cfg(not(any(target_os = "linux", target_os = "macos")))]
            TermiosGuard::Unsupported => {}
        }
    }
}

// ---------------------------------------------------------------------------
// termios FFI (Linux and macOS)
// ---------------------------------------------------------------------------

#[cfg(any(target_os = "linux", target_os = "macos"))]
const TCSANOW: i32 = 0;
#[cfg(target_os = "linux")]
const ICANON: u32 = 0x0002;
#[cfg(target_os = "linux")]
const ECHO: u32 = 0x0008;
#[cfg(target_os = "linux")]
const ISIG: u32 = 0x0001;
#[cfg(target_os = "linux")]
const ICRNL: u32 = 0x0100;
#[cfg(target_os = "linux")]
const IXON: u32 = 0x0400;
#[cfg(target_os = "macos")]
const ICANON: u64 = 0x0000_0100;
#[cfg(target_os = "macos")]
const ECHO: u64 = 0x0000_0008;
#[cfg(target_os = "macos")]
const ISIG: u64 = 0x0000_0080;
#[cfg(target_os = "macos")]
const ICRNL: u64 = 0x0000_0200;
#[cfg(target_os = "macos")]
const IXON: u64 = 0x0000_0020;

/// Raw termios layout.  The field widths match glibc (Linux) and macOS.
#[cfg(target_os = "linux")]
#[repr(C)]
#[derive(Clone, Copy)]
struct RawTermios {
    c_iflag: u32,
    c_oflag: u32,
    c_cflag: u32,
    c_lflag: u32,
    c_line: u8,
    c_cc: [u8; 32],
    c_ispeed: u32,
    c_ospeed: u32,
}

#[cfg(target_os = "macos")]
#[repr(C)]
#[derive(Clone, Copy)]
struct RawTermios {
    c_iflag: u64,
    c_oflag: u64,
    c_cflag: u64,
    c_lflag: u64,
    c_cc: [u8; 20],
    c_ispeed: u64,
    c_ospeed: u64,
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
#[derive(Clone, Copy)]
struct RawTermios;

impl Default for RawTermios {
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
extern "C" {
    fn tcgetattr(fd: i32, termios_p: *mut RawTermios) -> i32;
    fn tcsetattr(fd: i32, optional_actions: i32, termios_p: *const RawTermios) -> i32;
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
unsafe fn raw_tcgetattr(fd: i32, termios: *mut RawTermios) -> i32 {
    tcgetattr(fd, termios)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
unsafe fn raw_tcsetattr(fd: i32, actions: i32, termios: *const RawTermios) -> i32 {
    tcsetattr(fd, actions, termios)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_line_statements_are_complete() {
        assert!(input_is_complete("var x := 42", 1));
        assert!(input_is_complete("put \"hi\"", 1));
        assert!(input_is_complete("if x > 0, put x end if", 1));
    }

    #[test]
    fn block_statements_buffer_until_terminator() {
        assert!(!input_is_complete("fn add(a: i32): i32", 1));
        assert!(!input_is_complete("fn add(a: i32): i32\n    return a", 2));
        assert!(input_is_complete(
            "fn add(a: i32): i32\n    return a\nend fn",
            3
        ));
        assert!(!input_is_complete("if x > 0,", 1));
        assert!(!input_is_complete("if x > 0,\n    put x", 2));
        assert!(input_is_complete("if x > 0,\n    put x\nend if", 3));
        assert!(!input_is_complete("loop i 0..5", 1));
        assert!(input_is_complete("loop i 0..5\n    put i\nend loop", 3));
    }

    #[test]
    fn non_block_keywords_do_not_buffer() {
        assert!(input_is_complete("loop_the_loop()", 1));
        assert!(input_is_complete("fnsnack := 3", 1));
        assert!(input_is_complete("var x := \"if only\"", 1));
    }
}
