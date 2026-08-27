//! Poly Language CLI
//!
//! Command-line interface for the Poly compiler/transpiler.

mod repl;

use std::path::{Path, PathBuf};
use std::process;

use anyhow::{bail, Context, Result};

/// Supported compilation targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Target {
    Rust,
    C,
}

impl Target {
    fn from_str(s: &str) -> Result<Self> {
        match s {
            "rust" | "rs" => Ok(Target::Rust),
            "c" => Ok(Target::C),
            other => bail!("Unknown target '{}'. Supported targets: rust, c", other),
        }
    }

    fn extension(&self) -> &str {
        match self {
            Target::Rust => "rs",
            Target::C => "c",
        }
    }

    fn language_name(&self) -> &str {
        match self {
            Target::Rust => "rust",
            Target::C => "c",
        }
    }
}

/// Extract and remove `--target <lang>` from the argument list.
/// The target is handled before subcommand dispatch so every CLI mode, including
/// `--check`, `--emit-*`, and project generation, selects the same backend.
fn extract_target(args: &[String]) -> Result<(Target, Vec<String>)> {
    let mut target = Target::Rust;
    let mut filtered = Vec::new();
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--target" {
            i += 1;
            if i >= args.len() {
                bail!("--target requires a language argument (e.g. --target rust)");
            }
            target = Target::from_str(&args[i])?;
        } else {
            filtered.push(args[i].clone());
        }
        i += 1;
    }
    Ok((target, filtered))
}

fn main() -> Result<()> {
    // Keep argument parsing dependency-free: the CLI is also used as a small
    // standalone binary in generated-project and integration-test workflows.
    let raw_args: Vec<String> = std::env::args().collect();
    let (target, args) = extract_target(&raw_args)?;

    if args.len() < 2 {
        eprintln!("Poly Language Compiler v{}", env!("CARGO_PKG_VERSION"));
        eprintln!();
        eprintln!("Usage: poly [options] <file.poly>");
        eprintln!();
        eprintln!("Options:");
        eprintln!("  --target <lang>   Target language (default: rust; options: rust, c)");
        eprintln!("  --tokens          Print tokens and exit");
        eprintln!("  --ast             Print AST and exit");
        eprintln!("  --check           Validate code and verify compilation");
        eprintln!("  --emit-rust       Print transpiled Rust instead of compiling");
        eprintln!("  --emit-c          Print transpiled C instead of compiling");
        eprintln!("  --intermediate-representation  Print the IR pipeline output");
        eprintln!("  --ir                           Alias for --intermediate-representation");
        eprintln!("  --source-map      Print the generated source map");
        eprintln!("  --format          Format output with rustfmt");
        eprintln!("  --diff            Show diff between unformatted and formatted");
        eprintln!("  --watch           Watch file and re-transpile on changes");
        eprintln!(
            "  --project         Generate a Cargo project (usage: --project <dir> <file.poly>)"
        );
        eprintln!("  --repl            Start interactive REPL");
        eprintln!("  --help            Show this help message");
        eprintln!("  --version         Show version information");
        process::exit(1);
    }

    match args[1].as_str() {
        "--help" | "-h" => {
            println!("Poly Language Compiler v{}", env!("CARGO_PKG_VERSION"));
            println!();
            println!("Usage: poly [options] <file.poly>");
            println!();
            println!("Options:");
            println!("  --target <lang>   Target language (default: rust; options: rust, c)");
            println!("  --tokens          Print tokens and exit");
            println!("  --ast             Print AST and exit");
            println!("  --check           Validate code and verify compilation");
            println!("  --emit-rust       Print transpiled Rust instead of compiling");
            println!("  --emit-c          Print transpiled C instead of compiling");
            println!("  --intermediate-representation  Print the IR pipeline output");
            println!("  --ir                           Alias for --intermediate-representation");
            println!("  --source-map      Print the generated source map");
            println!("  --format          Format output with rustfmt");
            println!(
                "  --project         Generate a Cargo project (usage: --project <dir> <file.poly>)"
            );
            println!("  --repl            Start interactive REPL");
            println!("  --help            Show this help message");
            println!("  --version         Show version information");
            println!();
            println!("Targets:");
            println!("  rust              Transpile to Rust (default)");
            println!("  c                 Transpile to C (C11 subset)");
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
                let label = path.display().to_string();
                for error in &errors {
                    eprintln!(
                        "{}",
                        poly_lexer::render_error(&source, error.span, &label, &error.message)
                    );
                    eprintln!();
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
            let label = path.display().to_string();

            let (tokens, errors) = poly_lexer::Lexer::lex(&source);

            if !errors.is_empty() {
                eprintln!("Lexer errors:");
                for error in &errors {
                    eprintln!(
                        "{}",
                        poly_lexer::render_error(&source, error.span, &label, &error.message)
                    );
                    eprintln!();
                }
                process::exit(1);
            }

            let mut parser = poly_parser::Parser::new(&tokens);
            match parser.parse() {
                Ok(program) => {
                    println!("{:#?}", program);
                }
                Err(e) => {
                    let rendered = poly_lexer::render_error(&source, e.span, &label, &e.message);
                    eprintln!("{}", rendered);
                    if let Some(suggestion) = &e.suggestion {
                        eprintln!("  note: {}", suggestion);
                    }
                    process::exit(1);
                }
            }
            Ok(())
        }
        "--check" => {
            // `--check` runs every compiler phase and the selected native
            // compiler, so success means the target output is buildable.
            if args.len() < 3 {
                eprintln!("Error: --check requires a file argument");
                process::exit(1);
            }
            let path = PathBuf::from(&args[2]);
            let source = std::fs::read_to_string(&path)
                .with_context(|| format!("Failed to read file: {}", path.display()))?;
            let label = path.display().to_string();

            // The CLI keeps phase boundaries visible in its diagnostics: lexer
            // errors stop meaningful parsing, while parser recovery can expose
            // several independent syntax errors in one invocation.
            let (tokens, lexer_errors) = poly_lexer::Lexer::lex(&source);
            let mut has_errors = false;

            if !lexer_errors.is_empty() {
                eprintln!("Lexer errors:");
                for error in &lexer_errors {
                    eprintln!(
                        "{}",
                        poly_lexer::render_error(&source, error.span, &label, &error.message)
                    );
                    eprintln!();
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
                        eprintln!(
                            "{}",
                            poly_lexer::render_error(&source, error.span, &label, &error.message)
                        );
                        if let Some(suggestion) = &error.suggestion {
                            eprintln!("  note: {}", suggestion);
                        }
                        eprintln!();
                    }
                    has_errors = true;
                }

                let selected_program = match poly_transpiler::Transpiler::parse_target(
                    &source,
                    target.language_name(),
                ) {
                    Ok(program) => program,
                    Err(error) => {
                        eprintln!("Target parse error: {}", error);
                        has_errors = true;
                        program.clone()
                    }
                };

                // Run semantic type checking before transpilation.
                if !has_errors {
                    let (check_result, warnings) =
                        poly_transpiler::check_program_with_warnings(&selected_program);
                    for warning in warnings {
                        eprintln!("Warning: {}", warning);
                    }
                    if let Err(type_errors) = check_result {
                        eprintln!("Type errors:");
                        for error in type_errors {
                            eprintln!("  {}", error);
                        }
                        has_errors = true;
                    }
                }

                // Run transpiler to check for transpilation errors
                if !has_errors {
                    let transpiler = poly_transpiler::Transpiler::new();
                    match transpiler.transpile_target(&source, target.language_name()) {
                        Ok(code) => {
                            let compile_result = match target {
                                Target::Rust => verify_rust_compiles(&code),
                                Target::C => verify_c_compiles(&code),
                            };
                            match compile_result {
                                Ok(_) => println!(
                                    "OK: {} statements parsed, {} code compiles",
                                    selected_program.statements.len(),
                                    target.language_name()
                                ),
                                Err(e) => {
                                    eprintln!(
                                        "{} compilation error: {}",
                                        target.language_name(),
                                        e
                                    );
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
        "--emit-rust" | "--emit-c" => {
            if args.len() < 3 {
                eprintln!("Error: emit option requires a file argument");
                process::exit(1);
            }
            let path = PathBuf::from(&args[2]);
            let source = std::fs::read_to_string(&path)
                .with_context(|| format!("Failed to read file: {}", path.display()))?;

            let transpiler = poly_transpiler::Transpiler::new();
            let requested_target = if args[1] == "--emit-c" { "c" } else { "rust" };
            let code = transpiler
                .transpile_target(&source, requested_target)
                .map_err(|e| anyhow::anyhow!(e))?;
            if requested_target == "rust" {
                let formatted = format_with_rustfmt(&code).unwrap_or(code);
                println!("{}", formatted);
            } else {
                println!("{}", code);
            }
            Ok(())
        }
        "--intermediate-representation" | "--ir" => {
            if args.len() < 3 {
                eprintln!("Error: --intermediate-representation requires a file argument");
                process::exit(1);
            }
            let path = PathBuf::from(&args[2]);
            let source = std::fs::read_to_string(&path)
                .with_context(|| format!("Failed to read file: {}", path.display()))?;

            let transpiler = poly_transpiler::Transpiler::new();
            let rust_code = transpiler
                .transpile_with_intermediate_representation(&source)
                .map_err(|e| anyhow::anyhow!(e))?;
            // The optimizer can rewrite programs in ways the checker never
            // sees, so verify the optimized output actually compiles instead
            // of printing invalid Rust silently.
            if let Err(e) = verify_rust_compiles(&rust_code) {
                eprintln!("Optimized Rust compilation error: {}", e);
                process::exit(1);
            }
            let formatted = format_with_rustfmt(&rust_code).unwrap_or(rust_code);
            println!("{}", formatted);
            Ok(())
        }
        "--source-map" => {
            if args.len() < 3 {
                eprintln!("Error: --source-map requires a file argument");
                process::exit(1);
            }
            let path = PathBuf::from(&args[2]);
            let source = std::fs::read_to_string(&path)
                .with_context(|| format!("Failed to read file: {}", path.display()))?;

            let mut transpiler = poly_transpiler::Transpiler::with_source_map();
            let (_rust_code, source_map) = transpiler
                .transpile_with_source_map(&source)
                .map_err(|e| anyhow::anyhow!(e))?;
            println!("{}", source_map.summary());
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
        "--project" => {
            if args.len() < 4 {
                eprintln!("Error: --project requires an output directory and a .poly file");
                eprintln!("Usage: poly --project <dir> <file.poly>");
                process::exit(1);
            }

            let output_dir = PathBuf::from(&args[2]);
            let source_path = PathBuf::from(&args[3]);
            match target {
                Target::Rust => {
                    generate_cargo_project(&source_path, &output_dir)?;
                    println!(
                        "Generated Cargo project at {} (source: {})",
                        output_dir.display(),
                        source_path.display()
                    );
                }
                Target::C => {
                    generate_c_project(&source_path, &output_dir)?;
                    println!(
                        "Generated C project at {} (source: {})",
                        output_dir.display(),
                        source_path.display()
                    );
                }
            }
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
            repl::run();
            Ok(())
        }
        file if file.ends_with(".poly") => {
            let path = PathBuf::from(file);
            match target {
                Target::Rust => {
                    let output_dir = default_cargo_output_dir(&path);
                    generate_cargo_project_inner(&path, &output_dir, false)?;
                    build_cargo_project(&output_dir)?;
                    let binary_path = cargo_binary_path(&output_dir, &path);
                    println!(
                        "Generated and built {} -> {}",
                        path.display(),
                        binary_path.display()
                    );
                }
                Target::C => {
                    let output_path = default_c_output_path(&path);
                    let source = std::fs::read_to_string(&path)
                        .with_context(|| format!("Failed to read file: {}", path.display()))?;
                    let code = poly_transpiler::Transpiler::new()
                        .transpile_c_checked(&source)
                        .map_err(|e| anyhow::anyhow!(e))?;
                    std::fs::write(&output_path, &code)
                        .with_context(|| format!("Failed to write {}", output_path.display()))?;
                    compile_c_binary(&output_path, &c_binary_path(&path))?;
                    println!(
                        "Generated and built {} -> {}",
                        path.display(),
                        c_binary_path(&path).display()
                    );
                }
            }
            Ok(())
        }
        other => {
            eprintln!("Error: Unknown option '{}'", other);
            eprintln!("Run 'poly --help' for usage information.");
            process::exit(1);
        }
    }
}

/// Generate a standalone C project from a Poly source file.
fn generate_c_project(source_path: &Path, output_dir: &Path) -> Result<()> {
    if source_path
        .extension()
        .and_then(|extension| extension.to_str())
        != Some("poly")
    {
        bail!(
            "Source file must have a .poly extension: {}",
            source_path.display()
        );
    }
    let source = std::fs::read_to_string(source_path)
        .with_context(|| format!("Failed to read source file: {}", source_path.display()))?;
    let code = poly_transpiler::Transpiler::new()
        .transpile_c_checked(&source)
        .map_err(|error| anyhow::anyhow!(error))?;
    if output_dir.exists() && output_dir.read_dir()?.next().is_some() {
        bail!(
            "Refusing to overwrite existing files in {}",
            output_dir.display()
        );
    }
    std::fs::create_dir_all(output_dir).with_context(|| {
        format!(
            "Failed to create project directory: {}",
            output_dir.display()
        )
    })?;
    let source_path_out = output_dir.join("main.c");
    let binary_path = output_dir.join(format!("main{}", std::env::consts::EXE_SUFFIX));
    std::fs::write(&source_path_out, &code)
        .with_context(|| format!("Failed to write {}", source_path_out.display()))?;
    compile_c_binary(&source_path_out, &binary_path)?;
    Ok(())
}

/// Generate a standalone Cargo project from a Poly source file.
fn generate_cargo_project(source_path: &Path, output_dir: &Path) -> Result<()> {
    generate_cargo_project_inner(source_path, output_dir, true)
}

/// Generate a Cargo project, optionally replacing files generated by Poly before.
fn generate_cargo_project_inner(
    source_path: &Path,
    output_dir: &Path,
    refuse_overwrite: bool,
) -> Result<()> {
    // Validate the source suffix before reading or creating anything so a typo
    // cannot accidentally turn an arbitrary file into a generated project.
    if source_path
        .extension()
        .and_then(|extension| extension.to_str())
        != Some("poly")
    {
        bail!(
            "Source file must have a .poly extension: {}",
            source_path.display()
        );
    }

    let source = std::fs::read_to_string(source_path)
        .with_context(|| format!("Failed to read file: {}", source_path.display()))?;

    let transpiler = poly_transpiler::Transpiler::new();
    let rust_code = transpiler
        .transpile_checked(&source)
        .map_err(|e| anyhow::anyhow!(e))?;
    let rust_code = format_with_rustfmt(&rust_code).unwrap_or(rust_code);

    let src_dir = output_dir.join("src");
    let manifest_path = output_dir.join("Cargo.toml");
    let main_rs_path = src_dir.join("main.rs");
    let generated_marker = output_dir.join(".poly-generated");
    // Explicit `--project` is conservative, while the default command may
    // refresh only directories that it previously marked as Poly-generated.
    if refuse_overwrite && (manifest_path.exists() || main_rs_path.exists()) {
        bail!(
            "Refusing to overwrite existing generated files in {}",
            output_dir.display()
        );
    }
    let source_identity = source_path
        .canonicalize()
        .unwrap_or_else(|_| source_path.to_path_buf());
    let marker_contents = format!("source={}\n", source_identity.display());
    if !refuse_overwrite && (manifest_path.exists() || main_rs_path.exists()) {
        if !generated_marker.is_file() {
            bail!(
                "Refusing to overwrite a non-Poly Cargo project in {}",
                output_dir.display()
            );
        }
        let existing_marker = std::fs::read_to_string(&generated_marker).with_context(|| {
            format!(
                "Failed to read generated-project marker: {}",
                generated_marker.display()
            )
        })?;
        if existing_marker != marker_contents {
            bail!(
                "Generated output directory {} belongs to a different source file",
                output_dir.display()
            );
        }
    }

    std::fs::create_dir_all(&src_dir)
        .with_context(|| format!("Failed to create project directory: {}", src_dir.display()))?;

    let package_name = cargo_package_name(output_dir, source_path);
    let manifest = cargo_manifest(
        &package_name,
        rust_code.contains("#[tokio::main]") || rust_code.contains("tokio::"),
        rust_code.contains("rusqlite::"),
    );

    std::fs::write(&manifest_path, manifest)
        .with_context(|| format!("Failed to write {}", manifest_path.display()))?;
    std::fs::write(&main_rs_path, rust_code)
        .with_context(|| format!("Failed to write {}", main_rs_path.display()))?;
    std::fs::write(&generated_marker, marker_contents)
        .with_context(|| format!("Failed to write {}", generated_marker.display()))?;

    Ok(())
}

/// Return the Cargo project directory used by the default compiler command.
fn default_cargo_output_dir(source_path: &Path) -> PathBuf {
    let stem = source_path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("poly-program");
    PathBuf::from("rust_output").join(sanitize_output_directory_name(stem))
}

/// Make a source stem safe and readable as a generated directory name.
fn sanitize_output_directory_name(stem: &str) -> String {
    let name: String = stem
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '_' || character == '-' {
                character
            } else {
                '-'
            }
        })
        .collect();

    if name.is_empty() {
        "poly-program".to_string()
    } else {
        name
    }
}

/// Build a generated Cargo project with its own dependency and target directory.
fn build_cargo_project(output_dir: &Path) -> Result<()> {
    use std::process::Command;

    // Build in the generated project itself so Cargo owns dependency and target
    // isolation; the user's workspace is never used as the build directory.

    let output = Command::new("cargo")
        .arg("build")
        .current_dir(output_dir)
        .output()
        .with_context(|| {
            format!(
                "Failed to run cargo in {}. Is Cargo installed?",
                output_dir.display()
            )
        })?;

    if output.status.success() {
        return Ok(());
    }

    let message = if output.stderr.is_empty() {
        String::from_utf8_lossy(&output.stdout).to_string()
    } else {
        String::from_utf8_lossy(&output.stderr).to_string()
    };
    bail!(
        "Cargo build failed in {}:\n{}",
        output_dir.display(),
        message
    )
}

/// Return the debug binary path produced by Cargo for a generated project.
fn cargo_binary_path(output_dir: &Path, source_path: &Path) -> PathBuf {
    let package_name = cargo_package_name(output_dir, source_path);
    output_dir.join("target").join("debug").join(format!(
        "{}{}",
        package_name,
        std::env::consts::EXE_SUFFIX
    ))
}

/// Derive a safe Cargo package name from the output directory.
fn cargo_package_name(output_dir: &Path, source_path: &Path) -> String {
    let candidate = output_dir
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty() && *name != "." && *name != "..")
        .or_else(|| source_path.file_stem().and_then(|name| name.to_str()))
        .unwrap_or("poly-project");

    let mut name: String = candidate
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '_' || character == '-' {
                character
            } else {
                '-'
            }
        })
        .collect();

    if name.is_empty()
        || !name
            .chars()
            .any(|character| character.is_ascii_alphanumeric())
    {
        name = "poly-project".to_string();
    }
    if name.starts_with(|character: char| character.is_ascii_digit()) {
        name.insert_str(0, "poly-");
    }

    name
}

/// Build the manifest for a generated Cargo project.
fn cargo_manifest(package_name: &str, needs_tokio: bool, needs_rusqlite: bool) -> String {
    let mut dependencies = String::new();
    if needs_tokio {
        // "time" enables `tokio::time::sleep` (used by `delay`); "net" and
        // "io-util" back the `http_get`/`tcp_connect`/`spawn` builtins.
        dependencies.push_str(
            "tokio = { version = \"1\", features = [\"macros\", \"rt-multi-thread\", \"time\", \"net\", \"io-util\"] }\n",
        );
    }
    if needs_rusqlite {
        // "bundled" compiles SQLite from source so no system library is needed.
        dependencies.push_str("rusqlite = { version = \"0.31\", features = [\"bundled\"] }\n");
    }

    format!(
        "[package]\nname = \"{}\"\nversion = \"{}\"\nedition = \"2021\"\n\n[dependencies]\n{}\n[workspace]\n",
        package_name,
        env!("CARGO_PKG_VERSION"),
        dependencies
    )
}

/// Run the interactive REPL with syntax highlighting
/// Format Rust code with rustfmt
fn format_with_rustfmt(code: &str) -> Option<String> {
    // Formatting is best-effort: all callers retain valid unformatted output
    // when rustfmt is unavailable or rejects incomplete generated code.
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

/// Return the executable path produced for a Poly source file.
#[allow(dead_code)]
fn executable_path(source_path: &Path) -> PathBuf {
    let mut output_path = source_path.to_path_buf();
    output_path.set_extension(std::env::consts::EXE_EXTENSION);
    output_path
}

/// Compile generated Rust into an executable with rustc.
#[allow(dead_code)]
fn compile_rust_binary(code: &str, output_path: &Path) -> Result<()> {
    use std::process::Command;

    let unique_id = format!(
        "{}-{}",
        process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    );
    let temp_source = std::env::temp_dir().join(format!("poly-{}.rs", unique_id));
    let output_name = output_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("poly-output");
    let temp_output = output_path.with_file_name(format!(".{}.tmp-{}", output_name, unique_id));

    std::fs::write(&temp_source, code).context("Failed to write temporary Rust source")?;

    let rustc_result = Command::new("rustc")
        .arg("--edition")
        .arg("2021")
        .arg("--crate-name")
        .arg("poly_program")
        .arg(&temp_source)
        .arg("-o")
        .arg(&temp_output)
        .output();

    let _ = std::fs::remove_file(&temp_source);
    let output = match rustc_result {
        Ok(output) => output,
        Err(error) => {
            let _ = std::fs::remove_file(&temp_output);
            return Err(anyhow::anyhow!(
                "Failed to run rustc. Is Rust installed? ({})",
                error
            ));
        }
    };

    if !output.status.success() {
        let _ = std::fs::remove_file(&temp_output);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let message = if stderr.trim().is_empty() {
            String::from_utf8_lossy(&output.stdout).to_string()
        } else {
            stderr.to_string()
        };
        return Err(anyhow::anyhow!(message));
    }

    if let Err(error) = install_compiled_binary(&temp_output, output_path) {
        let _ = std::fs::remove_file(&temp_output);
        return Err(error);
    }
    Ok(())
}

/// Move a successfully compiled temporary binary into its final location.
#[allow(dead_code)]
fn install_compiled_binary(temp_output: &Path, output_path: &Path) -> Result<()> {
    if std::env::consts::FAMILY != "windows" || !output_path.exists() {
        return std::fs::rename(temp_output, output_path).with_context(|| {
            format!(
                "Failed to install compiled executable at {}",
                output_path.display()
            )
        });
    }

    // Windows cannot rename over an existing executable. Keep a backup until
    // the new binary has been installed so an installation error is recoverable.
    let unique_id = format!(
        "{}-{}",
        process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    );
    let output_name = output_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("poly-output");
    let backup_path = output_path.with_file_name(format!(".{}.backup-{}", output_name, unique_id));

    std::fs::rename(output_path, &backup_path).with_context(|| {
        format!(
            "Failed to move existing executable {}",
            output_path.display()
        )
    })?;

    match std::fs::rename(temp_output, output_path) {
        Ok(()) => {
            let _ = std::fs::remove_file(backup_path);
            Ok(())
        }
        Err(error) => {
            let _ = std::fs::rename(&backup_path, output_path);
            Err(anyhow::anyhow!(
                "Failed to install compiled executable at {}: {}",
                output_path.display(),
                error
            ))
        }
    }
}

fn default_c_output_path(source_path: &Path) -> PathBuf {
    let mut output = source_path.to_path_buf();
    output.set_extension(Target::C.extension());
    output
}

fn c_binary_path(source_path: &Path) -> PathBuf {
    let mut output = source_path.to_path_buf();
    output.set_extension(std::env::consts::EXE_EXTENSION);
    output
}

fn verify_c_compiles(code: &str) -> Result<()> {
    let unique_id = format!(
        "{}-{}",
        process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    );
    let temp_dir = std::env::temp_dir();
    let source_path = temp_dir.join(format!("poly_check_{unique_id}.c"));
    let output_path = temp_dir.join(format!("poly_check_{unique_id}"));
    std::fs::write(&source_path, code).context("Failed to write temporary C source")?;
    let mut compiler = c_compiler_command()?;
    let output = compiler
        .arg("-std=c11")
        .arg("-Wall")
        .arg("-Werror")
        .arg("-fsyntax-only")
        .arg(&source_path)
        .output()
        .context("Failed to run the configured C compiler");
    let _ = std::fs::remove_file(&source_path);
    let _ = std::fs::remove_file(&output_path);
    let output = output?;
    if output.status.success() {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            String::from_utf8_lossy(&output.stderr).to_string()
        ))
    }
}

fn c_compiler_command() -> Result<std::process::Command> {
    use std::process::Command;

    if let Ok(configured) = std::env::var("POLY_CC") {
        if configured.trim().is_empty() {
            bail!("POLY_CC is set but empty; configure it to a C11 compiler executable");
        }
        return Ok(Command::new(configured));
    }

    let candidates = if cfg!(windows) {
        ["gcc", "clang", "cc"]
    } else {
        ["cc", "clang", "gcc"]
    };
    for candidate in candidates {
        if Command::new(candidate).arg("--version").output().is_ok() {
            return Ok(Command::new(candidate));
        }
    }

    bail!("No C compiler found; install C11 cc/clang/gcc or set POLY_CC to its executable")
}

fn compile_c_binary(source_path: &Path, output_path: &Path) -> Result<()> {
    let mut compiler = c_compiler_command()?;
    let output = compiler
        .arg("-std=c11")
        .arg(source_path)
        .arg("-o")
        .arg(output_path)
        .output()
        .with_context(|| {
            format!(
                "Failed to run the configured C compiler for {}",
                source_path.display()
            )
        })?;
    if output.status.success() {
        Ok(())
    } else {
        bail!(
            "C compilation failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

/// Verify generated Rust without running LLVM code generation.
///
/// Metadata emission preserves the compiler checks needed by `--check` while
/// avoiding an unnecessary object file. The unique source and output names
/// also make concurrent CLI checks safe.
///
/// Async programs emit `#[tokio::main]`, which a bare `rustc` invocation
/// cannot resolve, so those are verified in a temporary Cargo project that
/// declares tokio as a dependency.
fn verify_rust_compiles(code: &str) -> Result<()> {
    use std::process::Command;
    use std::sync::{Mutex, OnceLock};
    use std::time::{SystemTime, UNIX_EPOCH};

    static RUSTC_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    let _rustc_guard = RUSTC_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| anyhow::anyhow!("rustc verification lock was poisoned"))?;

    let unique_id = format!(
        "{}-{}",
        process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    );

    // A program needs the Cargo project when it emits `#[tokio::main]` or
    // references tokio directly (e.g. `delay` lowers to `tokio::time::sleep`
    // inside async functions even when main stays sync), or when `db_execute`
    // references the rusqlite crate.
    if code.contains("#[tokio::main]") || code.contains("tokio::") || code.contains("rusqlite::") {
        return verify_async_rust_compiles(code, &unique_id);
    }

    let temp_dir = std::env::temp_dir();
    let stem = format!("poly_check_{unique_id}").replace('-', "_");
    let temp_file = temp_dir.join(format!("{stem}.rs"));
    let output_file = temp_dir.join(format!("lib{stem}.rmeta"));
    std::fs::write(&temp_file, code).context("Failed to write temp file")?;

    let output = Command::new("rustc")
        .arg("--edition")
        .arg("2021")
        .arg("--crate-type")
        .arg("lib")
        .arg("--crate-name")
        .arg(&stem)
        .arg("--emit=metadata")
        .arg("--out-dir")
        .arg(&temp_dir)
        .arg(&temp_file)
        .output()
        .context("Failed to run rustc. Is Rust installed?");

    let _ = std::fs::remove_file(&temp_file);
    let _ = std::fs::remove_file(output_file);
    let output = output?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(anyhow::anyhow!(stderr.to_string()))
    }
}

/// Verify async generated Rust by running `cargo check` in a temporary Cargo
/// project that provides the tokio dependency `#[tokio::main]` requires.
fn verify_async_rust_compiles(code: &str, unique_id: &str) -> Result<()> {
    use std::process::Command;

    let temp_dir = std::env::temp_dir();
    let project_dir = temp_dir.join(format!("poly_check_async_{unique_id}"));
    let src_dir = project_dir.join("src");
    std::fs::create_dir_all(&src_dir).context("Failed to create temp Cargo project")?;
    let rusqlite = if code.contains("rusqlite::") {
        // "bundled" compiles SQLite from source so no system library is needed.
        "rusqlite = { version = \"0.31\", features = [\"bundled\"] }\n"
    } else {
        ""
    };
    let manifest = format!(
        "[package]\nname = \"poly_check_async\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n[dependencies]\ntokio = {{ version = \"1\", features = [\"macros\", \"rt-multi-thread\", \"time\", \"net\", \"io-util\"] }}\n{rusqlite}[workspace]\n"
    );
    std::fs::write(project_dir.join("Cargo.toml"), manifest)
        .context("Failed to write temp Cargo.toml")?;
    std::fs::write(src_dir.join("main.rs"), code).context("Failed to write temp main.rs")?;

    let output = Command::new("cargo")
        .arg("check")
        .arg("--quiet")
        .current_dir(&project_dir)
        .output()
        .context("Failed to run cargo. Is Cargo installed?");

    let _ = std::fs::remove_dir_all(&project_dir);
    let output = output?;

    if output.status.success() {
        Ok(())
    } else {
        let message = if output.stderr.is_empty() {
            String::from_utf8_lossy(&output.stdout).to_string()
        } else {
            String::from_utf8_lossy(&output.stderr).to_string()
        };
        Err(anyhow::anyhow!(message))
    }
}

/// Show diff between original and formatted code
fn show_diff(original: &str, formatted: &str) {
    // This is a small line-oriented diff intended for readable CLI output, not
    // a replacement for a patch algorithm or machine-readable diff format.
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

    // Polling keeps watch mode portable and avoids adding a filesystem-notify
    // dependency to the compiler's small CLI binary.

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_directory() -> PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("poly-cli-test-{}-{}", process::id(), timestamp))
    }

    #[test]
    fn target_parsing_supports_rust_and_c() {
        assert_eq!(Target::from_str("rust").unwrap(), Target::Rust);
        assert_eq!(Target::from_str("c").unwrap(), Target::C);
        assert!(Target::from_str("cpp").is_err());
        assert_eq!(Target::C.extension(), "c");
    }

    #[test]
    fn cargo_manifest_adds_tokio_only_for_async_programs() {
        let synchronous = cargo_manifest("demo", false, false);
        assert!(synchronous.contains("name = \"demo\""));
        assert!(!synchronous.contains("tokio"));
        assert!(!synchronous.contains("rusqlite"));
        assert!(synchronous.contains("[workspace]"));

        let asynchronous = cargo_manifest("demo", true, false);
        assert!(asynchronous.contains("tokio = { version = \"1\""));
        assert!(asynchronous.contains("\"time\""));
        assert!(!asynchronous.contains("rusqlite"));

        let with_db = cargo_manifest("demo", false, true);
        assert!(with_db.contains("rusqlite = { version = \"0.31\""));
    }

    #[test]
    fn cargo_package_name_is_safe_for_cargo() {
        assert_eq!(
            cargo_package_name(Path::new("my demo"), Path::new("source.poly")),
            "my-demo"
        );
        assert_eq!(
            cargo_package_name(Path::new("123"), Path::new("source.poly")),
            "poly-123"
        );
        assert_eq!(
            cargo_package_name(Path::new("---"), Path::new("source.poly")),
            "poly-project"
        );
    }

    #[test]
    fn default_output_dir_is_per_program() {
        assert_eq!(
            default_cargo_output_dir(Path::new("examples/prime_numbers.poly")),
            PathBuf::from("rust_output/prime_numbers")
        );
        assert_eq!(
            default_cargo_output_dir(Path::new("examples/my program.poly")),
            PathBuf::from("rust_output/my-program")
        );
    }

    #[test]
    fn project_generation_writes_manifest_and_main_rs() {
        let root = test_directory();
        let source_path = root.join("hello.poly");
        let output_dir = root.join("generated");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(&source_path, "put \"Hello\"").unwrap();

        generate_cargo_project(&source_path, &output_dir).unwrap();

        let manifest = std::fs::read_to_string(output_dir.join("Cargo.toml")).unwrap();
        let rust_code = std::fs::read_to_string(output_dir.join("src/main.rs")).unwrap();
        assert!(manifest.contains("[package]"));
        assert!(manifest.contains("name = \"generated\""));
        assert!(rust_code.contains("println!"));

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn project_generation_rejects_non_poly_sources() {
        let root = test_directory();
        std::fs::create_dir_all(&root).unwrap();
        let source_path = root.join("hello.txt");

        std::fs::write(&source_path, "put \"Hello\"").unwrap();
        assert!(generate_cargo_project(&source_path, &root.join("generated")).is_err());

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn executable_path_replaces_poly_extension() {
        let expected = if cfg!(windows) {
            PathBuf::from("examples/hello.exe")
        } else {
            PathBuf::from("examples/hello")
        };
        assert_eq!(executable_path(Path::new("examples/hello.poly")), expected);
    }

    #[test]
    fn compile_rust_binary_writes_executable() {
        let root = test_directory();
        std::fs::create_dir_all(&root).unwrap();
        let output_path = root.join("hello");

        compile_rust_binary("fn main() { println!(\"hello\"); }", &output_path).unwrap();
        assert!(output_path.is_file());

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_compilation_preserves_existing_executable() {
        let root = test_directory();
        std::fs::create_dir_all(&root).unwrap();
        let output_path = root.join("hello");
        std::fs::write(&output_path, "existing binary placeholder").unwrap();

        assert!(compile_rust_binary("fn main( {", &output_path).is_err());
        assert_eq!(
            std::fs::read_to_string(&output_path).unwrap(),
            "existing binary placeholder"
        );

        std::fs::remove_dir_all(root).unwrap();
    }
}
