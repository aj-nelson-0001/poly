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
        eprintln!("  --check     Validate code without transpiling");
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
            println!("  --check     Validate code without transpiling");
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
                        Ok(_) => {
                            println!("OK: {} statements parsed", program.statements.len());
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
        "--repl" => {
            run_repl();
            Ok(())
        }
        file if file.ends_with(".poly") => {
            let path = PathBuf::from(file);
            let source = std::fs::read_to_string(&path)
                .with_context(|| format!("Failed to read file: {}", path.display()))?;

            let transpiler = poly_transpiler::Transpiler::new();
            let rust_code = transpiler.transpile(&source)
                .map_err(|e| anyhow::anyhow!(e))?;

            println!("{}", rust_code);
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

    println!("{}{}Poly Language REPL{} v{}", BOLD, CYAN, RESET, env!("CARGO_PKG_VERSION"));
    println!("{}Type Poly code and press Enter to transpile to Rust.{}", DIM, RESET);
    println!("{}Commands: :help, :tokens, :ast, :quit{}", DIM, RESET);
    println!();

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
                    println!("{}Goodbye!{}", GREEN, RESET);
                    break;
                }

                if input == ":help" {
                    println!();
                    println!("{}REPL Commands:{}", BOLD, RESET);
                    println!("  {}:{}    Show this help message", CYAN, RESET);
                    println!("  {}:{}    Show tokens for buffered input", CYAN, RESET);
                    println!("  {}:{}     Show AST for buffered input", CYAN, RESET);
                    println!("  {}:{}    Clear the buffer", CYAN, RESET);
                    println!("  {}:{}    Exit the REPL", CYAN, RESET);
                    println!();
                    println!("{}Poly Syntax:{}", BOLD, RESET);
                    println!("  {}var{} x: {}i32{} = {}", MAGENTA, RESET, BLUE, RESET, GREEN);
                    println!("  {}put{} \"{}Hello, World!{}\"", MAGENTA, RESET, YELLOW, RESET);
                    println!("  {}fn{} {}add{}(a: {}i32{}, b: {}i32{}): {}i32{}", MAGENTA, RESET, CYAN, RESET, BLUE, RESET, BLUE, RESET, BLUE, RESET);
                    println!("      {}return{} a {}+{} b", MAGENTA, RESET, RED, RESET);
                    println!("  {}end{} {}fn{}", MAGENTA, RESET, MAGENTA, RESET);
                    println!();
                    continue;
                }

                if input == ":clear" {
                    buffer.clear();
                    line_number = 0;
                    println!("{}Buffer cleared.{}", GREEN, RESET);
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
    } else if token_str.contains("Fn") || token_str.contains("Let") || token_str.contains("Var")
        || token_str.contains("Return") || token_str.contains("If") || token_str.contains("Else")
        || token_str.contains("While") || token_str.contains("For") || token_str.contains("Match")
        || token_str.contains("End") || token_str.contains("Struct") || token_str.contains("Enum")
        || token_str.contains("Break") || token_str.contains("Continue") {
        format!("{}{}{}", MAGENTA, token_str, RESET)
    } else if token_str.contains("Identifier") {
        format!("{}{}{}", CYAN, token_str, RESET)
    } else {
        format!("{}{}{}", GREEN, token_str, RESET)
    }
}

/// Highlight Rust code for REPL output
fn highlight_rust(line: &str) -> String {
    const RESET: &str = "\x1b[0m";
    const DIM: &str = "\x1b[2m";
    const GREEN: &str = "\x1b[32m";
    const YELLOW: &str = "\x1b[33m";
    const BLUE: &str = "\x1b[34m";
    const MAGENTA: &str = "\x1b[35m";
    const CYAN: &str = "\x1b[36m";

    // Simple Rust syntax highlighting
    let trimmed = line.trim();

    // Comments
    if trimmed.starts_with("//") {
        return format!("{}{}{}", DIM, line, RESET);
    }

    // Keywords
    let mut result = line.to_string();

    // Apply highlighting to common patterns
    let keywords = ["fn", "let", "mut", "return", "if", "else", "while", "for", "loop",
                     "struct", "enum", "impl", "use", "pub", "const", "break", "continue",
                     "match", "self", "Self", "true", "false"];

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
