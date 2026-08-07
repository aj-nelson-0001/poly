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
        file if file.ends_with(".poly") => {
            let path = PathBuf::from(file);
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
