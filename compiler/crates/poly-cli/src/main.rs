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

/// Run the interactive REPL
fn run_repl() {
    use std::io::{self, Write};

    println!("Poly Language REPL v{}", env!("CARGO_PKG_VERSION"));
    println!("Type Poly code and press Enter to transpile to Rust.");
    println!("Commands: :help, :tokens, :ast, :quit");
    println!();

    let mut buffer = String::new();
    let mut line_number = 0;

    loop {
        // Show prompt
        if buffer.is_empty() {
            print!("poly> ");
        } else {
            print!("  ... ");
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
                    println!("Goodbye!");
                    break;
                }

                if input == ":help" {
                    println!("REPL Commands:");
                    println!("  :help     Show this help message");
                    println!("  :tokens   Show tokens for buffered input");
                    println!("  :ast      Show AST for buffered input");
                    println!("  :clear    Clear the buffer");
                    println!("  :quit     Exit the REPL");
                    println!();
                    println!("Poly Syntax:");
                    println!("  var x: i32 = 42");
                    println!("  put \"Hello, World!\"");
                    println!("  fn add(a: i32, b: i32): i32");
                    println!("      return a + b");
                    println!("  end fn");
                    continue;
                }

                if input == ":clear" {
                    buffer.clear();
                    line_number = 0;
                    println!("Buffer cleared.");
                    continue;
                }

                if input == ":tokens" {
                    if buffer.is_empty() {
                        println!("No input buffered.");
                        continue;
                    }
                    let (tokens, errors) = poly_lexer::Lexer::lex(&buffer);
                    if !errors.is_empty() {
                        println!("Lexer errors:");
                        for error in &errors {
                            println!("  {}", error);
                        }
                    } else {
                        println!("Tokens:");
                        for token in &tokens {
                            println!("  {:?}", token);
                        }
                    }
                    continue;
                }

                if input == ":ast" {
                    if buffer.is_empty() {
                        println!("No input buffered.");
                        continue;
                    }
                    let (tokens, errors) = poly_lexer::Lexer::lex(&buffer);
                    if !errors.is_empty() {
                        println!("Lexer errors:");
                        for error in &errors {
                            println!("  {}", error);
                        }
                    } else {
                        let mut parser = poly_parser::Parser::new(&tokens);
                        match parser.parse() {
                            Ok(program) => {
                                println!("{:#?}", program);
                            }
                            Err(e) => {
                                println!("Parse error: {}", e);
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
                // Simple heuristic: check if it ends with a keyword that starts a new statement
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
                            println!("// Generated Rust code:");
                            println!("{}", rust_code);
                        }
                        Err(e) => {
                            println!("Error: {}", e);
                        }
                    }
                    buffer.clear();
                    line_number = 0;
                    println!();
                }
            }
            Err(e) => {
                eprintln!("Error reading input: {}", e);
                break;
            }
        }
    }
}
