mod ast;
mod generator;
mod lexer;
mod migration;
mod parser;
mod validator;

use std::{env, fs, path::Path, process};

fn main() {
    if let Err(error) = run() {
        eprintln!("Error: {error}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();

    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_help();
        return Ok(());
    }

    if args.iter().any(|arg| arg == "--version" || arg == "-V") {
        println!("RustORM-CLI {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    let (command, schema_path, output_path) = match args.first().map(String::as_str) {
        Some("validate") => (
            "validate",
            args.get(1).map(String::as_str).unwrap_or("rust.schema"),
            None,
        ),
        Some("generate") => (
            "generate",
            args.get(1).map(String::as_str).unwrap_or("rust.schema"),
            None,
        ),
        Some("migrate") => (
            "migrate",
            args.get(1).map(String::as_str).unwrap_or("rust.schema"),
            Some(
                args.get(2)
                    .map(String::as_str)
                    .unwrap_or("migrations/0001_initial.sql"),
            ),
        ),
        Some(other) if other.starts_with('-') => {
            return Err(format!("Unknown option '{other}'. Use --help."));
        }
        Some(_) => (
            "generate",
            args.first().map(String::as_str).unwrap_or("rust.schema"),
            None,
        ),
        None => ("generate", "rust.schema", None),
    };

    let source = fs::read_to_string(schema_path)
        .map_err(|error| format!("Failed to read '{schema_path}': {error}"))?;

    let tokens = lexer::tokenize(&source).map_err(|error| format!("Lexer error: {error:?}"))?;

    let schema = parser::Parser::new(tokens)
        .parse()
        .map_err(|error| format!("Parser error: {error:?}"))?;

    validator::validate(&schema).map_err(|error| format!("Validation error: {error:?}"))?;

    match command {
        "validate" => {
            println!("Schema is valid: {schema_path}");
        }
        "generate" => {
            generator::write_entities(&schema, "generated/entities.rs")
                .map_err(|error| format!("Failed to write generated entities: {error}"))?;

            println!("Schema is valid.");
            println!("Generated entities: generated/entities.rs");
        }
        "migrate" => {
            let output_path = output_path.expect("migrate always has an output path");
            let sql = migration::generate_migration(&schema)?;

            if let Some(parent) = Path::new(output_path).parent()
                && !parent.as_os_str().is_empty()
            {
                fs::create_dir_all(parent)
                    .map_err(|error| format!("Failed to create migration directory: {error}"))?;
            }

            fs::write(output_path, sql)
                .map_err(|error| format!("Failed to write migration '{output_path}': {error}"))?;

            println!("Migration generated: {output_path}");
        }
        _ => unreachable!(),
    }

    Ok(())
}

fn print_help() {
    println!(
        "RustORM-CLI {}\n\
         \n\
         Usage:\n\
           RustORM-CLI [SCHEMA]\n\
           RustORM-CLI validate [SCHEMA]\n\
           RustORM-CLI generate [SCHEMA]\n\
           RustORM-CLI migrate [SCHEMA] [OUTPUT]\n\
           RustORM-CLI --help\n\
           RustORM-CLI --version\n\
         \n\
         Commands:\n\
           validate  Validate a schema\n\
           generate  Generate Rust entities (default)\n\
           migrate   Generate initial PostgreSQL migration SQL\n",
        env!("CARGO_PKG_VERSION")
    );
}
