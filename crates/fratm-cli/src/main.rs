//! FratmScript CLI - JavaScript, but the way it should be 🤌

use clap::{Parser as ClapParser, Subcommand};
use colored::*;
use fratm_core::{compile, errors, CompileOptions};
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::process::Command;

#[derive(ClapParser)]
#[command(name = "fratm")]
#[command(author = "Federico")]
#[command(version = fratm_core::version())]
#[command(about = "🤌 FratmScript - JavaScript, but the way it should be", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Compile and launch a .fratm file
    #[command(name = "lancia", alias = "run")]
    Lancia {
        file: PathBuf,
        #[arg(long)]
        sourcemap: bool,
    },
    /// Compile a .fratm file to JavaScript
    Build {
        file: PathBuf,
        #[arg(short, long)]
        output: Option<PathBuf>,
        #[arg(long)]
        sourcemap: bool,
    },
    /// Check .fratm files for errors without writing any output
    Check {
        #[arg(required = true)]
        files: Vec<PathBuf>,
    },
    /// Format .fratm files in place
    Fmt {
        #[arg(required = true)]
        files: Vec<PathBuf>,
        /// Only report files that are not formatted (non-zero exit if any)
        #[arg(long)]
        check: bool,
    },
    /// Interactive REPL
    Repl,
    /// Show tokens (debug)
    Tokens { file: PathBuf },
    /// Show AST (debug)
    Ast { file: PathBuf },
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Lancia { file, sourcemap } => run_file(&file, sourcemap),
        Commands::Build {
            file,
            output,
            sourcemap,
        } => build_file(&file, output, sourcemap),
        Commands::Check { files } => check_files(&files),
        Commands::Fmt { files, check } => format_files(&files, check),
        Commands::Repl => run_repl(),
        Commands::Tokens { file } => show_tokens(&file),
        Commands::Ast { file } => show_ast(&file),
    }
}

/// Loaded with `node -r` before the program: reports an uncaught error as
/// JSON in the file named by `FRATM_ERROR_FILE`, so `fratm lancia` can print
/// it in Neapolitan at the `.fratm` line (the stack is source-mapped).
const ERROR_HOOK: &str = r#"const fs = require('fs')
function report(e) {
  const isError = e !== null && typeof e === 'object'
  fs.writeFileSync(process.env.FRATM_ERROR_FILE, JSON.stringify({
    name: isError && e.name ? String(e.name) : 'Error',
    message: isError && 'message' in e ? String(e.message) : String(e),
    stack: isError && e.stack ? String(e.stack) : '',
  }))
  process.exit(1)
}
process.on('uncaughtException', report)
process.on('unhandledRejection', report)
"#;

/// Source map name of the program, as it appears in source-mapped stacks.
const SOURCE_NAME: &str = "input.fratm";

/// `--sourcemap` is kept for compatibility: `lancia` always maps runtime
/// errors back to the `.fratm` source now.
fn run_file(path: &PathBuf, _sourcemap: bool) {
    let source = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{} {}", "Error: file not found:".red().bold(), e);
            std::process::exit(1);
        }
    };

    let options = CompileOptions {
        source_map: true,
        filename: Some(path.display().to_string()),
        minify: false,
    };

    let result = match compile(&source, options) {
        Ok(result) => result,
        Err(e) => {
            print_error(&source, &e);
            std::process::exit(1);
        }
    };

    let script_dir = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    let id = std::process::id();
    let temp_path = script_dir.join(format!(".fratm_temp_{}.js", id));
    let hook_path = std::env::temp_dir().join(format!("fratm_hook_{}.cjs", id));
    let error_path = std::env::temp_dir().join(format!("fratm_error_{}.json", id));
    let mut output = result.code;
    if let Some(sm) = &result.source_map {
        output.push('\n');
        output.push_str(&sm.to_data_url());
    }
    if let Err(e) = fs::write(&temp_path, &output).and_then(|_| fs::write(&hook_path, ERROR_HOOK)) {
        eprintln!("{} {}", "Error: cannot write file:".red().bold(), e);
        std::process::exit(1);
    }
    let _ = fs::remove_file(&error_path);
    let cmd_status = Command::new("node")
        .arg("--enable-source-maps")
        .arg("--require")
        .arg(&hook_path)
        .arg(&temp_path)
        .env("FRATM_ERROR_FILE", &error_path)
        .status();
    let _ = fs::remove_file(&temp_path);
    let _ = fs::remove_file(&hook_path);
    let report = fs::read_to_string(&error_path).ok();
    let _ = fs::remove_file(&error_path);
    match cmd_status {
        Ok(status) => {
            if let Some(report) = report {
                print_runtime_error(&source, &report);
                std::process::exit(1);
            }
            if !status.success() {
                std::process::exit(status.code().unwrap_or(1));
            }
        }
        Err(e) => {
            eprintln!("{} {}", "Error: Node.js failed:".red().bold(), e);
            std::process::exit(1);
        }
    }
}

/// Prints an uncaught runtime error reported by [`ERROR_HOOK`].
fn print_runtime_error(source: &str, report: &str) {
    let value: serde_json::Value = serde_json::from_str(report).unwrap_or_default();
    let field = |key: &str| value[key].as_str().unwrap_or_default().to_string();
    let message =
        fratm_core::runtime::napoletanize_runtime_error(&field("name"), &field("message"));
    match runtime_position(&field("stack")) {
        Some((line, column)) => {
            eprintln!(
                "\n{} Riga {}, colonna {}: {}",
                "✗ Errore a runtime:".red().bold(),
                line,
                column,
                message
            );
            print_source_line(source, line, Some(column));
        }
        None => eprintln!("\n{} {}", "✗ Errore a runtime:".red().bold(), message),
    }
    eprintln!("\n{}", errors::random_encouragement().dimmed());
}

/// Line and column of the innermost stack frame inside the `.fratm` program.
fn runtime_position(stack: &str) -> Option<(usize, usize)> {
    stack.lines().find_map(|frame| {
        let at = frame.find(SOURCE_NAME)? + SOURCE_NAME.len();
        let mut parts = frame[at..].trim_start_matches(':').split(':');
        let line = parts.next()?.parse().ok()?;
        let column = parts.next()?.trim_end_matches(')').parse().ok()?;
        Some((line, column))
    })
}

fn build_file(path: &PathBuf, output: Option<PathBuf>, sourcemap: bool) {
    let source = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{} {}", "Error: file not found:".red().bold(), e);
            std::process::exit(1);
        }
    };

    let options = CompileOptions {
        source_map: sourcemap,
        filename: Some(path.display().to_string()),
        minify: false,
    };

    match compile(&source, options) {
        Ok(result) => {
            let out_path = output.unwrap_or_else(|| {
                let mut p = path.clone();
                p.set_extension("js");
                p
            });
            let mut output_content = result.code;
            if sourcemap {
                if let Some(sm) = &result.source_map {
                    let map_path = out_path.with_extension("js.map");
                    if let Err(e) = fs::write(&map_path, sm.to_json_pretty()) {
                        eprintln!("{} {}", "Warning: cannot write source map:".yellow(), e);
                    } else {
                        output_content.push_str(&format!(
                            "\n//# sourceMappingURL={}",
                            map_path.file_name().unwrap().to_string_lossy()
                        ));
                        println!("  {} {}", "Source map:".dimmed(), map_path.display());
                    }
                }
            }
            if let Err(e) = fs::write(&out_path, &output_content) {
                eprintln!("{} {}", "Error: cannot write file:".red().bold(), e);
                std::process::exit(1);
            }
            println!(
                "{} {} → {}",
                errors::success_message().green().bold(),
                path.display(),
                out_path.display()
            );
        }
        Err(e) => {
            print_error(&source, &e);
            std::process::exit(1);
        }
    }
}

fn check_files(paths: &[PathBuf]) {
    let mut failed = 0;
    for path in paths {
        let Some(source) = read_source(path) else {
            failed += 1;
            continue;
        };
        if let Err(e) = compile(&source, Default::default()) {
            eprintln!("{}", path.display().to_string().bold());
            print_error(&source, &e);
            failed += 1;
        }
    }
    if failed > 0 {
        eprintln!(
            "{}",
            format!("✗ {} of {} files have errors", failed, paths.len())
                .red()
                .bold()
        );
        std::process::exit(1);
    }
    println!(
        "{} {} file(s) checked",
        errors::success_message().green().bold(),
        paths.len()
    );
}

fn format_files(paths: &[PathBuf], check: bool) {
    let mut failed = 0;
    let mut unformatted = 0;
    for path in paths {
        let Some(source) = read_source(path) else {
            failed += 1;
            continue;
        };
        let formatted = match fratm_core::format(&source) {
            Ok(formatted) => formatted,
            Err(e) => {
                eprintln!("{}", path.display().to_string().bold());
                print_error(&source, &e);
                failed += 1;
                continue;
            }
        };
        if formatted == source {
            continue;
        }
        unformatted += 1;
        if check {
            println!("{} {}", "Not formatted:".yellow(), path.display());
        } else if let Err(e) = fs::write(path, formatted) {
            eprintln!("{} {}", "Error: cannot write file:".red().bold(), e);
            failed += 1;
        } else {
            println!("{} {}", "Formatted:".green(), path.display());
        }
    }
    if failed > 0 || (check && unformatted > 0) {
        std::process::exit(1);
    }
    if unformatted == 0 {
        println!(
            "{} {} file(s) already formatted",
            errors::success_message().green().bold(),
            paths.len()
        );
    }
}

fn read_source(path: &PathBuf) -> Option<String> {
    match fs::read_to_string(path) {
        Ok(s) => Some(s),
        Err(e) => {
            eprintln!(
                "{} {}: {}",
                "Error: cannot read file".red().bold(),
                path.display(),
                e
            );
            None
        }
    }
}

fn run_repl() {
    println!(
        "{}",
        "🤌 FratmScript REPL - Write JavaScript the way it should be"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        format!(
            "   Version {} - Type 'exit' to quit\n",
            fratm_core::version()
        )
        .dimmed()
    );
    let stdin = io::stdin();
    let mut accumulated = String::new();
    loop {
        let prompt = if accumulated.is_empty() {
            "fratm> "
        } else {
            "  ...> "
        };
        print!("{}", prompt.green());
        io::stdout().flush().unwrap();
        let mut line = String::new();
        if stdin.lock().read_line(&mut line).is_err() {
            break;
        }
        let trimmed = line.trim();
        if trimmed == "esci" || trimmed == "exit" {
            println!("{}", "Goodbye! 👋".cyan());
            break;
        }
        if trimmed.is_empty() {
            continue;
        }
        accumulated.push_str(&line);
        match compile(&accumulated, Default::default()) {
            Ok(result) => {
                println!("{}", "─".repeat(40).dimmed());
                println!("{}", result.code.trim().blue());
                println!("{}", "─".repeat(40).dimmed());
                let temp_path = std::env::temp_dir().join("fratm_repl.js");
                if fs::write(&temp_path, &result.code).is_ok() {
                    if let Ok(output) = Command::new("node").arg(&temp_path).output() {
                        let stdout = String::from_utf8_lossy(&output.stdout);
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        if !stdout.is_empty() {
                            print!("{}", stdout);
                        }
                        if !stderr.is_empty() {
                            eprint!("{}", stderr.red());
                        }
                    }
                }
                accumulated.clear();
            }
            Err(e) => {
                let msg = format!("{}", e);
                if !msg.contains("'}'") && !msg.contains("')'") {
                    println!("{} {}", "✗".red().bold(), msg.red());
                    accumulated.clear();
                }
            }
        }
    }
}

fn show_tokens(path: &PathBuf) {
    let source = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{} {}", "Error: file not found:".red().bold(), e);
            std::process::exit(1);
        }
    };
    let mut lexer = fratm_core::lexer::Lexer::new(&source);
    let tokens = lexer.tokenize();
    println!("{}", "Tokens:".cyan().bold());
    for token in tokens {
        println!(
            "  {:20} @ {}:{}",
            format!("{:?}", token.kind).yellow(),
            token.span.line.to_string().dimmed(),
            token.span.column.to_string().dimmed()
        );
    }
}

fn show_ast(path: &PathBuf) {
    let source = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{} {}", "Error: file not found:".red().bold(), e);
            std::process::exit(1);
        }
    };
    let mut lexer = fratm_core::lexer::Lexer::new(&source);
    let tokens = lexer.tokenize();
    let mut parser = fratm_core::parser::Parser::new(tokens);
    match parser.parse() {
        Ok(program) => {
            println!("{}", "AST:".cyan().bold());
            println!(
                "{}",
                serde_json::to_string_pretty(&program).unwrap_or_default()
            );
        }
        Err(errors) => {
            for e in errors {
                println!("{} {}", "✗".red().bold(), e.message.red());
            }
        }
    }
}

fn print_error(source: &str, error: &fratm_core::errors::CompileError) {
    eprintln!("\n{} {}", "✗ Error:".red().bold(), error);
    if let Some(line_num) = error.line() {
        print_source_line(source, line_num, error.column());
    }
    if let Some(suggestion) = errors::suggest(error, source) {
        eprintln!("\n{}", suggestion.yellow());
    }
    eprintln!("\n{}", errors::random_encouragement().dimmed());
}

/// Prints line `line_num` of `source` with a caret under `column`.
fn print_source_line(source: &str, line_num: usize, column: Option<usize>) {
    let lines: Vec<&str> = source.lines().collect();
    if line_num == 0 || line_num > lines.len() {
        return;
    }
    eprintln!(
        "  {} │ {}",
        line_num.to_string().dimmed(),
        lines[line_num - 1]
    );
    if let Some(col) = column {
        let pointer = " ".repeat(col.saturating_sub(1)) + "^";
        eprintln!(
            "  {} │ {}",
            " ".repeat(line_num.to_string().len()),
            pointer.red()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_fratm_frame_in_a_mapped_stack() {
        let stack = "TypeError: x\n    at f (/tmp/input.fratm:4:20)\n    at Object.<anonymous> (/tmp/input.fratm:6:1)";
        assert_eq!(runtime_position(stack), Some((4, 20)));
        let top_level =
            "ReferenceError: y\n    at /home/a b/input.fratm:2:12\n    at node:internal";
        assert_eq!(runtime_position(top_level), Some((2, 12)));
        assert_eq!(
            runtime_position("Error: z\n    at node:internal/x:1:1"),
            None
        );
    }
}
