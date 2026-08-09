//! Poly Language CLI
//!
//! Command-line interface for the Poly compiler/transpiler.

use std::path::PathBuf;
use std::process;

use anyhow::{Context, Result};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        eprintln!("Poly Language Compiler v{}", env!("CARGO_PKG_VERSION"));
        eprintln!();
        eprintln!("Usage: poly <file.poly> [options]");
        eprintln!();
        eprintln!("Options:");
        eprintln!("  --tokens    Print tokens and exit");
        eprintln!("  --ast       Print AST and exit");
        eprintln!("  --check     Validate code and verify Rust compilation");
        eprintln!("  --format    Format output with rustfmt");
        eprintln!("  --diff      Show diff between unformatted and formatted");
        eprintln!("  --watch     Watch file and re-transpile on changes");
        eprintln!("  --repl      Start interactive REPL");
        eprintln!("  --help      Show this help message");
        eprintln!("  --version   Show version information");
        process::exit(1);
    }

    match args[1].as_str() {
        "--help" | "-h" => {
            println!("Poly Language Compiler v{}", env!("CARGO_PKG_VERSION"));
            println!();
            println!("Usage: poly <file.poly> [options]");
            println!();
            println!("Options:");
            println!("  --tokens    Print tokens and exit");
            println!("  --ast       Print AST and exit");
            println!("  --check     Validate code and verify Rust compilation");
            println!("  --format    Format output with rustfmt");
            println!("  --repl      Start interactive REPL");
            println!("  --help      Show this help message");
            println!("  --version   Show version information");
            Ok(())
        }
        "--version" | "-v" => {
            println!("poly {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        "--tokens" => {
            if args.len() < 3 {
                eprintln!("Error: --tokens requires a file argument");
                process::exit(1);
            }
            let path = PathBuf::from(&args[2]);
            let source = std::fs::read_to_string(&path)
                .with_context(|| format!("Failed to read file: {}", path.display()))?;

            let (tokens, errors) = poly_lexer::Lexer::lex(&source);

            if !errors.is_empty() {
                eprintln!("Lexer errors:");
                for error in &errors {
                    eprintln!("  {}", error);
                }
                process::exit(1);
            }

            for token in &tokens {
                println!("{:?}", token);
            }
            Ok(())
        }
        "--ast" => {
            if args.len() < 3 {
                eprintln!("Error: --ast requires a file argument");
                process::exit(1);
            }
            let path = PathBuf::from(&args[2]);
            let source = std::fs::read_to_string(&path)
                .with_context(|| format!("Failed to read file: {}", path.display()))?;

            let (tokens, errors) = poly_lexer::Lexer::lex(&source);

            if !errors.is_empty() {
                eprintln!("Lexer errors:");
                for error in &errors {
                    eprintln!("  {}", error);
                }
                process::exit(1);
            }

            let mut parser = poly_parser::Parser::new(&tokens);
            match parser.parse() {
                Ok(program) => {
                    println!("{:#?}", program);
                }
                Err(e) => {
                    eprintln!("Parse error: {}", e);
                    process::exit(1);
                }
            }
            Ok(())
        }
        "--check" => {
            if args.len() < 3 {
                eprintln!("Error: --check requires a file argument");
                process::exit(1);
            }
            let path = PathBuf::from(&args[2]);
            let source = std::fs::read_to_string(&path)
                .with_context(|| format!("Failed to read file: {}", path.display()))?;

            // Run lexer
            let (tokens, lexer_errors) = poly_lexer::Lexer::lex(&source);
            let mut has_errors = false;

            if !lexer_errors.is_empty() {
                eprintln!("Lexer errors:");
                for error in &lexer_errors {
                    eprintln!("  {}", error);
                }
                has_errors = true;
            }

            // Run parser with error recovery
            if !has_errors || lexer_errors.is_empty() {
                let mut parser = poly_parser::Parser::new(&tokens);
                let (program, parse_errors) = parser.parse_with_recovery();

                if !parse_errors.is_empty() {
                    eprintln!("Parse errors:");
                    for error in &parse_errors {
                        eprintln!("  {}", error);
                    }
                    has_errors = true;
                }

                // Run transpiler to check for transpilation errors
                if !has_errors {
                    let transpiler = poly_transpiler::Transpiler::new();
                    match transpiler.transpile(&source) {
                        Ok(rust_code) => {
                            // Verify generated Rust code compiles
                            match verify_rust_compiles(&rust_code) {
                                Ok(_) => {
                                    println!(
                                        "OK: {} statements parsed, Rust code compiles",
                                        program.statements.len()
                                    );
                                }
                                Err(e) => {
                                    eprintln!("Rust compilation error: {}", e);
                                    has_errors = true;
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("Transpile error: {}", e);
                            has_errors = true;
                        }
                    }
                }
            }

            if has_errors {
                process::exit(1);
            }

            Ok(())
        }
        "--watch" => {
            if args.len() < 3 {
                eprintln!("Error: --watch requires a file argument");
                process::exit(1);
            }
            let path = PathBuf::from(&args[2]);
            watch_file(&path)?;
            Ok(())
        }
        "--diff" => {
            if args.len() < 3 {
                eprintln!("Error: --diff requires a file argument");
                process::exit(1);
            }
            let path = PathBuf::from(&args[2]);
            let source = std::fs::read_to_string(&path)
                .with_context(|| format!("Failed to read file: {}", path.display()))?;

            let transpiler = poly_transpiler::Transpiler::new();
            let rust_code = transpiler
                .transpile(&source)
                .map_err(|e| anyhow::anyhow!(e))?;

            // Get formatted version
            match format_with_rustfmt(&rust_code) {
                Some(formatted) => {
                    println!("--- Original");
                    println!("+++ Formatted");
                    show_diff(&rust_code, &formatted);
                }
                None => {
                    eprintln!("Warning: rustfmt not available");
                    println!("{}", rust_code);
                }
            }
            Ok(())
        }
        "--format" => {
            if args.len() < 3 {
                eprintln!("Error: --format requires a file argument");
                process::exit(1);
            }
            let path = PathBuf::from(&args[2]);
            let source = std::fs::read_to_string(&path)
                .with_context(|| format!("Failed to read file: {}", path.display()))?;

            let transpiler = poly_transpiler::Transpiler::new();
            let rust_code = transpiler
                .transpile(&source)
                .map_err(|e| anyhow::anyhow!(e))?;

            // Format with rustfmt
            match format_with_rustfmt(&rust_code) {
                Some(formatted) => println!("{}", formatted),
                None => {
                    eprintln!("Warning: rustfmt not available, outputting unformatted code");
                    println!("{}", rust_code);
                }
            }
            Ok(())
        }
        "--repl" => {
            run_repl();
            Ok(())
        }
        file if file.ends_with(".poly") => {
            let path = PathBuf::from(file);
            let source = std::fs::read_to_string(&path)
                .with_context(|| format!("Failed to read file: {}", path.display()))?;

            let transpiler = poly_transpiler::Transpiler::new();
            let rust_code = transpiler
                .transpile(&source)
                .map_err(|e| anyhow::anyhow!(e))?;

            // Try to format with rustfmt
            let formatted = format_with_rustfmt(&rust_code).unwrap_or(rust_code);
            println!("{}", formatted);
            Ok(())
        }
        other => {
            eprintln!("Error: Unknown option '{}'", other);
            eprintln!("Run 'poly --help' for usage information.");
            process::exit(1);
        }
    }
}

/// Run the interactive REPL with syntax highlighting
fn run_repl() {
    use std::io::{self, Write};

    // Color codes for syntax highlighting
    const RESET: &str = "\x1b[0m";
    const BOLD: &str = "\x1b[1m";
    const DIM: &str = "\x1b[2m";
    const RED: &str = "\x1b[31m";
    const GREEN: &str = "\x1b[32m";
    const YELLOW: &str = "\x1b[33m";
    const BLUE: &str = "\x1b[34m";
    const MAGENTA: &str = "\x1b[35m";
    const CYAN: &str = "\x1b[36m";

    println!(
        "{}{}Poly Language REPL{} v{}",
        BOLD,
        CYAN,
        RESET,
        env!("CARGO_PKG_VERSION")
    );
    println!(
        "{}Type Poly code and press Enter to transpile to Rust.{}",
        DIM, RESET
    );
    println!(
        "{}Commands: :help, :tokens, :ast, :history, :clear, :quit{}",
        DIM, RESET
    );
    println!();

    // Load command history
    let history_path = dirs_and_history_path();
    let mut history: Vec<String> = load_history(&history_path);
    let mut history_index: Option<usize> = None;
    let mut buffer = String::new();
    let mut line_number = 0;

    loop {
        // Show prompt
        if buffer.is_empty() {
            print!("{}poly{}>{} ", GREEN, BOLD, RESET);
        } else {
            print!("  {}...{} ", YELLOW, RESET);
        }
        io::stdout().flush().unwrap();

        // Read input
        let mut input = String::new();
        match io::stdin().read_line(&mut input) {
            Ok(0) => break, // EOF
            Ok(_) => {
                let input = input.trim();
                line_number += 1;

                // Handle commands
                if input == ":quit" || input == ":q" || input == ":exit" {
                    save_history(&history_path, &history);
                    println!("{}Goodbye!{}", GREEN, RESET);
                    break;
                }

                if input == ":help" {
                    println!();
                    println!("{}REPL Commands:{}", BOLD, RESET);
                    println!("  {}:{}    Show this help message", CYAN, RESET);
                    println!("  {}:{}    Show tokens for buffered input", CYAN, RESET);
                    println!("  {}:{}     Show AST for buffered input", CYAN, RESET);
                    println!("  {}:{}     Show command history", CYAN, RESET);
                    println!("  {}:{}    Clear the buffer", CYAN, RESET);
                    println!("  {}:{}      Clear history", CYAN, RESET);
                    println!("  {}:{}    Exit the REPL", CYAN, RESET);
                    println!();
                    println!("{}Poly Syntax:{}", BOLD, RESET);
                    println!(
                        "  {}var{} x: {}i32{} = {}",
                        MAGENTA, RESET, BLUE, RESET, GREEN
                    );
                    println!(
                        "  {}put{} \"{}Hello, World!{}\"",
                        MAGENTA, RESET, YELLOW, RESET
                    );
                    println!(
                        "  {}fn{} {}add{}(a: {}i32{}, b: {}i32{}): {}i32{}",
                        MAGENTA, RESET, CYAN, RESET, BLUE, RESET, BLUE, RESET, BLUE, RESET
                    );
                    println!("      {}return{} a {}+{} b", MAGENTA, RESET, RED, RESET);
                    println!("  {}end{} {}fn{}", MAGENTA, RESET, MAGENTA, RESET);
                    println!();
                    println!("{}Tips:{}", BOLD, RESET);
                    println!("{}  - Use ↑/↓ arrows to navigate history{}", DIM, RESET);
                    println!(
                        "{}  - Multi-line: continue on next line for blocks{}",
                        DIM, RESET
                    );
                    println!(
                        "{}  - Use :tokens or :ast to inspect buffered input{}",
                        DIM, RESET
                    );
                    println!();
                    continue;
                }

                if input == ":clear" {
                    buffer.clear();
                    line_number = 0;
                    println!("{}Buffer cleared.{}", GREEN, RESET);
                    continue;
                }

                if input == ":history" {
                    println!("{}Command History:{}", BOLD, RESET);
                    for (i, cmd) in history.iter().enumerate() {
                        println!("  {}{}: {}{}", DIM, i + 1, cmd, RESET);
                    }
                    if history.is_empty() {
                        println!("  {}(empty){}", DIM, RESET);
                    }
                    continue;
                }

                if input == ":clear-history" {
                    history.clear();
                    save_history(&history_path, &history);
                    println!("{}History cleared.{}", GREEN, RESET);
                    continue;
                }

                if input == ":tokens" {
                    if buffer.is_empty() {
                        println!("{}No input buffered.{}", YELLOW, RESET);
                        continue;
                    }
                    let (tokens, errors) = poly_lexer::Lexer::lex(&buffer);
                    if !errors.is_empty() {
                        println!("{}Lexer errors:{}", RED, RESET);
                        for error in &errors {
                            println!("  {}{}{}", RED, error, RESET);
                        }
                    } else {
                        println!("{}Tokens:{}", BOLD, RESET);
                        for token in &tokens {
                            // Syntax highlight tokens
                            let token_str = format!("{:?}", token);
                            let highlighted = highlight_token(&token_str);
                            println!("  {}", highlighted);
                        }
                    }
                    continue;
                }

                if input == ":ast" {
                    if buffer.is_empty() {
                        println!("{}No input buffered.{}", YELLOW, RESET);
                        continue;
                    }
                    let (tokens, errors) = poly_lexer::Lexer::lex(&buffer);
                    if !errors.is_empty() {
                        println!("{}Lexer errors:{}", RED, RESET);
                        for error in &errors {
                            println!("  {}{}{}", RED, error, RESET);
                        }
                    } else {
                        let mut parser = poly_parser::Parser::new(&tokens);
                        match parser.parse() {
                            Ok(program) => {
                                println!("{:#?}", program);
                            }
                            Err(e) => {
                                println!("{}Parse error: {}{}", RED, e, RESET);
                            }
                        }
                    }
                    continue;
                }

                // Add to history if not empty
                if !input.is_empty() {
                    // Avoid adding duplicates consecutively
                    if history.last().is_none_or(|last| last != input) {
                        history.push(input.to_string());
                        // Keep history reasonable size
                        if history.len() > 1000 {
                            history.drain(0..500);
                        }
                    }
                }
                history_index = None;

                // Accumulate input (multi-line support)
                if !buffer.is_empty() {
                    buffer.push('\n');
                }
                buffer.push_str(input);

                // Check if we have a complete statement
                let trimmed = buffer.trim();
                let is_complete = trimmed.ends_with("end fn")
                    || trimmed.ends_with("end if")
                    || trimmed.ends_with("end while")
                    || trimmed.ends_with("end struct")
                    || trimmed.ends_with("end enum")
                    || trimmed.ends_with("end match")
                    || trimmed.ends_with("end loop")
                    || trimmed.ends_with("end trait")
                    || trimmed.ends_with("end impl")
                    || trimmed.ends_with("end unsafe")
                    || (line_number > 0 && !input.is_empty() && !input.trim().ends_with('\\'));

                if is_complete {
                    // Transpile
                    let transpiler = poly_transpiler::Transpiler::new();
                    match transpiler.transpile(&buffer) {
                        Ok(rust_code) => {
                            println!();
                            println!("{}// Generated Rust code:{}", DIM, RESET);
                            // Syntax highlight the Rust output
                            for line in rust_code.lines() {
                                println!("  {}", highlight_rust(line));
                            }
                        }
                        Err(e) => {
                            println!("{}Error: {}{}", RED, e, RESET);
                            // Show helpful suggestions based on error
                            show_error_suggestions(&e);
                        }
                    }
                    buffer.clear();
                    line_number = 0;
                    println!();
                }
            }
            Err(e) => {
                eprintln!("{}Error reading input: {}{}", RED, e, RESET);
                break;
            }
        }
    }
}

/// Get the path to the history file
fn dirs_and_history_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    std::path::PathBuf::from(home).join(".poly_repl_history")
}

/// Load command history from file
fn load_history(path: &std::path::Path) -> Vec<String> {
    std::fs::read_to_string(path)
        .map(|content| content.lines().map(String::from).collect())
        .unwrap_or_default()
}

/// Save command history to file
fn save_history(path: &std::path::Path, history: &[String]) {
    let _ = std::fs::write(path, history.join("\n"));
}

/// Show helpful suggestions based on error message
fn show_error_suggestions(error: &str) {
    const DIM: &str = "\x1b[2m";
    const CYAN: &str = "\x1b[36m";
    const RESET: &str = "\x1b[0m";
    const YELLOW: &str = "\x1b[33m";

    if error.contains("Expected") && error.contains("end") {
        println!(
            "{}  💡 Tip: Blocks must end with 'end <keyword>' (e.g., end fn, end if){}",
            YELLOW, RESET
        );
    } else if error.contains("Unexpected token") {
        println!(
            "{}  💡 Tip: Check for missing semicolons or keywords{}
{}     Poly uses 'put' for output and 'get' for input{}
{}     Function calls use: fn_name(args){}
{}     Strings use double quotes: \"hello\"{}",
            YELLOW, RESET, DIM, RESET, DIM, RESET, DIM, RESET
        );
    } else if error.contains("type") {
        println!(
            "{}  💡 Tip: Valid types: i32, f64, string, bool, char, u8, etc.{}
{}     Or use custom types: MyStruct, MyEnum{}
{}     Containers: Vec<T>, Option<T>, Result<T, E>{}",
            YELLOW, RESET, DIM, RESET, DIM, RESET
        );
    } else if error.contains("assignment") || error.contains("= ") {
        println!(
            "{}  💡 Tip: Use '=' for assignment, '==' for comparison{}
{}     var x = 42  # declaration{}
{}     x = 10     # reassignment{}",
            YELLOW, RESET, DIM, RESET, DIM, RESET
        );
    }
}

/// Poly keywords for tab completion
const POLY_KEYWORDS: &[&str] = &[
    "var", "let", "const", "fn", "return", "if", "else", "end", "while", "for", "in", "loop",
    "struct", "enum", "match", "trait", "impl", "async", "await", "unsafe", "pub", "module", "use",
    "type", "as", "try", "spawn", "move", "break", "continue", // Types
    "i8", "i16", "i32", "i64", "i128", "u8", "u16", "u32", "u64", "u128", "f32", "f64", "bool",
    "char", "string", "usize", "isize", "byte", "bytes", // Builtins
    "put", "get", "error", "warn", "info",
];

/// Complete a partial input with keyword suggestions
fn complete_input(partial: &str) -> Vec<String> {
    let partial_lower = partial.to_lowercase();
    POLY_KEYWORDS
        .iter()
        .filter(|kw| kw.starts_with(&partial_lower))
        .map(|s| s.to_string())
        .collect()
}

/// Highlight a Poly token for REPL output
fn highlight_token(token_str: &str) -> String {
    const RESET: &str = "\x1b[0m";
    const GREEN: &str = "\x1b[32m";
    const YELLOW: &str = "\x1b[33m";
    const BLUE: &str = "\x1b[34m";
    const MAGENTA: &str = "\x1b[35m";
    const CYAN: &str = "\x1b[36m";

    if token_str.contains("StringLiteral") || token_str.contains("UnicodeStringLiteral") {
        format!("{}{}{}", YELLOW, token_str, RESET)
    } else if token_str.contains("IntLiteral") || token_str.contains("FloatLiteral") {
        format!("{}{}{}", BLUE, token_str, RESET)
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
        format!("{}{}{}", MAGENTA, token_str, RESET)
    } else if token_str.contains("Identifier") {
        format!("{}{}{}", CYAN, token_str, RESET)
    } else {
        format!("{}{}{}", GREEN, token_str, RESET)
    }
}

/// Format Rust code with rustfmt
fn format_with_rustfmt(code: &str) -> Option<String> {
    use std::io::Write;
    use std::process::Command;

    let mut child = Command::new("rustfmt")
        .arg("--edition")
        .arg("2021")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .ok()?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(code.as_bytes()).ok()?;
    }

    let output = child.wait_with_output().ok()?;
    if output.status.success() {
        String::from_utf8(output.stdout).ok()
    } else {
        None // Fall back to unformatted code
    }
}

/// Verify that Rust code compiles by running rustc --edition 2021 --crate-type lib
fn verify_rust_compiles(code: &str) -> Result<()> {
    use std::process::Command;

    // Write code to a temporary file
    let temp_dir = std::env::temp_dir();
    let temp_file = temp_dir.join("poly_check.rs");
    std::fs::write(&temp_file, code).context("Failed to write temp file")?;

    // Run rustc to check compilation
    let output = Command::new("rustc")
        .arg("--edition")
        .arg("2021")
        .arg("--crate-type")
        .arg("lib")
        .arg("--out-dir")
        .arg(&temp_dir)
        .arg(&temp_file)
        .output()
        .context("Failed to run rustc. Is Rust installed?")?;

    // Clean up
    let _ = std::fs::remove_file(&temp_file);
    let _ = std::fs::remove_file(temp_dir.join("libpoly_check.rlib"));

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(anyhow::anyhow!(stderr.to_string()))
    }
}

/// Highlight Rust code for REPL output
fn highlight_rust(line: &str) -> String {
    const RESET: &str = "\x1b[0m";
    const DIM: &str = "\x1b[2m";
    const YELLOW: &str = "\x1b[33m";
    const MAGENTA: &str = "\x1b[35m";

    // Simple Rust syntax highlighting
    let trimmed = line.trim();

    // Comments
    if trimmed.starts_with("//") {
        return format!("{}{}{}", DIM, line, RESET);
    }

    // Keywords
    let mut result = line.to_string();

    // Apply highlighting to common patterns
    let keywords = [
        "fn", "let", "mut", "return", "if", "else", "while", "for", "loop", "struct", "enum",
        "impl", "use", "pub", "const", "break", "continue", "match", "self", "Self", "true",
        "false",
    ];

    for keyword in keywords {
        // Color keywords
        result = result.replace(keyword, &format!("{}{}{}", MAGENTA, keyword, RESET));
    }

    // Highlight strings
    if result.contains('"') {
        let parts: Vec<&str> = result.split('"').collect();
        if parts.len() >= 3 {
            let mut highlighted = String::new();
            for (i, part) in parts.iter().enumerate() {
                if i % 2 == 0 {
                    highlighted.push_str(part);
                } else {
                    highlighted.push_str(&format!("{}\"{}\"{}", YELLOW, part, RESET));
                }
            }
            result = highlighted;
        }
    }

    // Highlight numbers
    for c in result.chars() {
        if c.is_numeric() {
            // This is a simplified approach - in production, use a proper tokenizer
        }
    }

    result
}

/// Show diff between original and formatted code
fn show_diff(original: &str, formatted: &str) {
    let original_lines: Vec<&str> = original.lines().collect();
    let formatted_lines: Vec<&str> = formatted.lines().collect();

    let mut i = 0;
    let mut j = 0;

    while i < original_lines.len() || j < formatted_lines.len() {
        if i < original_lines.len() && j < formatted_lines.len() {
            if original_lines[i] == formatted_lines[j] {
                // Lines match, show with space prefix
                println!(" {}", original_lines[i]);
                i += 1;
                j += 1;
            } else {
                // Lines differ, show removal and addition
                println!("-{}", original_lines[i]);
                println!("+{}", formatted_lines[j]);
                i += 1;
                j += 1;
            }
        } else if i < original_lines.len() {
            println!("-{}", original_lines[i]);
            i += 1;
        } else {
            println!("+{}", formatted_lines[j]);
            j += 1;
        }
    }
}

/// Watch a file for changes and re-transpile
fn watch_file(path: &std::path::Path) -> anyhow::Result<()> {
    use std::time::Duration;

    println!(
        "Watching {} for changes... (Ctrl+C to stop)",
        path.display()
    );

    let mut last_modified = std::fs::metadata(path).and_then(|m| m.modified()).ok();

    loop {
        std::thread::sleep(Duration::from_millis(500));

        let current_modified = std::fs::metadata(path).and_then(|m| m.modified()).ok();

        if current_modified != last_modified {
            last_modified = current_modified;

            // Read and transpile
            match std::fs::read_to_string(path) {
                Ok(source) => {
                    let transpiler = poly_transpiler::Transpiler::new();
                    match transpiler.transpile(&source) {
                        Ok(rust_code) => match format_with_rustfmt(&rust_code) {
                            Some(formatted) => {
                                println!("\n--- Transpiled at {} ---", chrono_free_timestamp());
                                println!("{}", formatted);
                            }
                            None => {
                                println!("\n--- Transpiled at {} ---", chrono_free_timestamp());
                                println!("{}", rust_code);
                            }
                        },
                        Err(e) => {
                            eprintln!("Transpile error: {}", e);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Read error: {}", e);
                }
            }
        }
    }
}

/// Get a simple timestamp without chrono
fn chrono_free_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{}", secs)
}
